#!/usr/bin/env python3
"""Extract an e2e fixture bundle from a live session.

Two source modes:
  --gateway-url http://...    talk to a live daemon
  --db /path/to/conversations.db  read sqlite directly (no daemon needed)

Usage:
    # From a live daemon
    PYTHONPATH=. python3 e2e/scripts/record-fixture.py \
        --session-id sess-xxx --out e2e/fixtures/aapl-peer-valuation \
        --gateway-url http://127.0.0.1:18791

    # From a stopped daemon's sqlite DB
    PYTHONPATH=. python3 e2e/scripts/record-fixture.py \
        --session-id sess-xxx --out e2e/fixtures/aapl-peer-valuation \
        --db ~/Documents/zbot/data/conversations.db

Emits session.json, llm-responses.jsonl, tool-results.jsonl, ws-events.jsonl.

ws-events.jsonl is reconstructed from execution metadata (lossy). Real
WS fidelity requires a live-ws capture, which is Phase 2 follow-up.
"""
import argparse
import hashlib
import json
import sqlite3
from datetime import datetime
from pathlib import Path

import httpx

from e2e.fixtures.types import (
    Artifact,
    Execution,
    LLMResponseRecord,
    SessionFixture,
    ToolResultRecord,
    WSEventRecord,
)


def fetch_session_rows(gateway_url: str, session_id: str) -> list[dict]:
    r = httpx.get(f"{gateway_url}/api/logs/sessions", params={"limit": 200}, timeout=5)
    r.raise_for_status()
    payload = r.json()
    rows = payload if isinstance(payload, list) else payload.get("data", [])
    return [row for row in rows if row.get("conversation_id") == session_id]


def fetch_messages(gateway_url: str, session_id: str) -> list[dict]:
    r = httpx.get(
        f"{gateway_url}/api/sessions/{session_id}/messages",
        params={"scope": "all"},
        timeout=10,
    )
    r.raise_for_status()
    payload = r.json()
    return payload if isinstance(payload, list) else payload.get("data", [])


def fetch_artifacts(gateway_url: str, session_id: str) -> list[dict]:
    r = httpx.get(f"{gateway_url}/api/sessions/{session_id}/artifacts", timeout=5)
    r.raise_for_status()
    payload = r.json()
    return payload if isinstance(payload, list) else payload.get("data", [])


def _db_connect(db_path: Path) -> sqlite3.Connection:
    con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    con.row_factory = sqlite3.Row
    return con


def db_session_rows(con: sqlite3.Connection, session_id: str) -> list[dict]:
    """Return log-session-shaped rows for a session's executions.

    API quirk preserved: log row's `session_id` field holds the execution
    id; `conversation_id` holds the real session_id.
    """
    session = con.execute(
        "SELECT id, title FROM sessions WHERE id = ?", (session_id,),
    ).fetchone()
    if session is None:
        return []
    execs = con.execute(
        """SELECT id, agent_id, parent_execution_id, status, started_at,
                  completed_at, tokens_in, tokens_out
             FROM agent_executions
            WHERE session_id = ?
            ORDER BY started_at""",
        (session_id,),
    ).fetchall()
    rows: list[dict] = []
    for e in execs:
        rows.append({
            "session_id": e["id"],
            "conversation_id": session_id,
            "agent_id": e["agent_id"],
            "agent_name": e["agent_id"],
            "title": session["title"] or "Untitled",
            "started_at": e["started_at"],
            "ended_at": e["completed_at"] or e["started_at"],
            "status": e["status"],
            "token_count": (e["tokens_in"] or 0) + (e["tokens_out"] or 0),
            "tool_call_count": 0,
            "error_count": 0,
            "duration_ms": 0,
            "parent_session_id": e["parent_execution_id"],
            "child_session_ids": [],
        })
    return rows


def db_messages(con: sqlite3.Connection, session_id: str) -> list[dict]:
    rows = con.execute(
        """SELECT id, execution_id, role, content, created_at,
                  tool_calls, tool_results, tool_call_id
             FROM messages
            WHERE session_id = ?
            ORDER BY created_at""",
        (session_id,),
    ).fetchall()
    out: list[dict] = []
    for m in rows:
        out.append({
            "id": m["id"],
            "execution_id": m["execution_id"],
            "role": m["role"],
            "content": m["content"] or "",
            "created_at": m["created_at"],
            "toolCalls": m["tool_calls"],
            "tool_results": m["tool_results"],
            "tool_call_id": m["tool_call_id"],
        })
    return out


def db_artifacts(con: sqlite3.Connection, session_id: str) -> list[dict]:
    rows = con.execute(
        """SELECT id, file_name, file_type, file_size
             FROM artifacts WHERE session_id = ?""",
        (session_id,),
    ).fetchall()
    return [
        {
            "id": r["id"],
            "file_name": r["file_name"] or "",
            "file_type": r["file_type"] or "",
            "file_size": r["file_size"] or 0,
        }
        for r in rows
    ]


def _parse_ts(s: str) -> float:
    return datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()


def build_session_fixture(
    rows: list[dict], artifacts: list[dict], session_id: str,
) -> SessionFixture:
    root = next((r for r in rows if not r.get("parent_session_id")), None)
    if root is None:
        raise SystemExit(f"no root row for session {session_id}")
    anchor = _parse_ts(root["started_at"])
    executions = []
    for r in rows:
        started = _parse_ts(r["started_at"])
        ended = _parse_ts(r.get("ended_at") or r["started_at"])
        executions.append(Execution(
            execution_id=r["session_id"],
            agent_id=r["agent_id"],
            parent_execution_id=r.get("parent_session_id") or None,
            started_at_offset_ms=int((started - anchor) * 1000),
            ended_at_offset_ms=int((ended - anchor) * 1000),
        ))
    return SessionFixture(
        session_id=session_id,
        title=root.get("title") or "Unnamed",
        executions=executions,
        artifacts=[
            Artifact(
                id=a["id"],
                file_name=a.get("file_name", ""),
                file_type=a.get("file_type", ""),
                file_size=a.get("file_size", 0),
            ) for a in artifacts
        ],
    )


def _parse_tool_calls(raw) -> list:
    if not raw:
        return []
    if isinstance(raw, str):
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return []
    return raw


def _group_assistant_messages(messages: list[dict]) -> dict[str, list[dict]]:
    by_exec: dict[str, list[dict]] = {}
    for m in messages:
        if m.get("role") != "assistant":
            continue
        exec_id = m.get("execution_id")
        if not exec_id:
            continue
        by_exec.setdefault(exec_id, []).append(m)
    return by_exec


def _build_llm_response(exec_id: str, iteration: int, m: dict) -> dict:
    tool_calls = _parse_tool_calls(m.get("toolCalls") or m.get("tool_calls"))
    content = m.get("content")
    if content == "[tool calls]":
        content = ""
    tc_field = [
        {
            "id": tc.get("tool_id", f"call_{ix}"),
            "type": "function",
            "function": {
                "name": tc.get("tool_name", ""),
                "arguments": json.dumps(tc.get("args", {})),
            },
        } for ix, tc in enumerate(tool_calls)
    ] if tool_calls else None
    return {
        "id": f"chatcmpl-{exec_id}-{iteration}",
        "object": "chat.completion",
        "choices": [
            {
                "index": 0,
                "finish_reason": "tool_calls" if tool_calls else "stop",
                "message": {
                    "role": "assistant",
                    "content": content,
                    "tool_calls": tc_field,
                },
            }
        ],
    }


def extract_llm_responses(messages: list[dict]) -> list[LLMResponseRecord]:
    records: list[LLMResponseRecord] = []
    for exec_id, msgs in _group_assistant_messages(messages).items():
        for i, m in enumerate(msgs):
            records.append(LLMResponseRecord(
                execution_id=exec_id,
                iteration=i,
                messages_hash=None,
                response=_build_llm_response(exec_id, i, m),
            ))
    return records


def _hash_args(args: dict) -> str:
    return "sha256:" + hashlib.sha256(
        json.dumps(args, sort_keys=True).encode()
    ).hexdigest()


def extract_tool_results(messages: list[dict]) -> list[ToolResultRecord]:
    records: list[ToolResultRecord] = []
    cursor_by_exec: dict[str, int] = {}
    pending_by_exec: dict[str, list[dict]] = {}
    for m in messages:
        exec_id = m.get("execution_id")
        if not exec_id:
            continue
        role = m.get("role")
        if role == "assistant":
            tool_calls = _parse_tool_calls(m.get("toolCalls") or m.get("tool_calls"))
            if tool_calls:
                pending_by_exec.setdefault(exec_id, []).extend(tool_calls)
            continue
        if role != "tool":
            continue
        pending = pending_by_exec.get(exec_id)
        if not pending:
            continue
        tc = pending.pop(0)
        idx = cursor_by_exec.get(exec_id, 0)
        records.append(ToolResultRecord(
            execution_id=exec_id,
            tool_index=idx,
            tool_name=tc.get("tool_name", ""),
            args_hash=_hash_args(tc.get("args", {})),
            result=m.get("content", ""),
        ))
        cursor_by_exec[exec_id] = idx + 1
    return records


def reconstruct_ws_events(rows: list[dict]) -> list[WSEventRecord]:
    """Lossy stub — produces minimal agent_started/agent_completed per
    execution. Real fixtures should use --live-ws capture for accuracy.
    """
    events: list[WSEventRecord] = []
    t = 0
    for r in rows:
        payload = {
            "session_id": r["conversation_id"],
            "execution_id": r["session_id"],
            "agent_id": r["agent_id"],
        }
        events.append(WSEventRecord(
            t_offset_ms=t, type="agent_started", payload=payload,
        ))
        t += 50
        events.append(WSEventRecord(
            t_offset_ms=t, type="agent_completed", payload=payload,
        ))
        t += 50
    return events


def write_jsonl(path: Path, records: list) -> None:
    with path.open("w") as f:
        for r in records:
            f.write(r.model_dump_json() + "\n")


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--session-id", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--gateway-url", default=None,
                   help="Live daemon URL (mutually exclusive with --db)")
    p.add_argument("--db", type=Path, default=None,
                   help="Path to conversations.db (read directly, no daemon)")
    args = p.parse_args()

    if bool(args.db) == bool(args.gateway_url):
        raise SystemExit("exactly one of --db or --gateway-url is required")

    args.out.mkdir(parents=True, exist_ok=True)

    if args.db:
        con = _db_connect(args.db)
        try:
            rows = db_session_rows(con, args.session_id)
            messages = db_messages(con, args.session_id)
            artifacts = db_artifacts(con, args.session_id)
        finally:
            con.close()
    else:
        rows = fetch_session_rows(args.gateway_url, args.session_id)
        messages = fetch_messages(args.gateway_url, args.session_id)
        artifacts = fetch_artifacts(args.gateway_url, args.session_id)

    if not rows:
        raise SystemExit(f"no rows for session {args.session_id}")

    fixture = build_session_fixture(rows, artifacts, args.session_id)
    (args.out / "session.json").write_text(fixture.model_dump_json(indent=2))

    write_jsonl(args.out / "llm-responses.jsonl", extract_llm_responses(messages))
    write_jsonl(args.out / "tool-results.jsonl", extract_tool_results(messages))
    write_jsonl(args.out / "ws-events.jsonl", reconstruct_ws_events(rows))

    print(f"Wrote fixture bundle to {args.out}")


if __name__ == "__main__":
    main()
