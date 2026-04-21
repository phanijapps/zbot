"""REST surface the UI consumes from the mock gateway.

Wire-shape quirk preserved: LogSession.session_id = execution id,
LogSession.conversation_id = sess-* id.
"""
import json
from pathlib import Path

from fastapi import FastAPI, HTTPException
from fastapi.responses import JSONResponse

from e2e.fixtures.types import SessionFixture


def _load_session_fixture(fixture_dir: Path) -> SessionFixture:
    raw = json.loads((fixture_dir / "session.json").read_text())
    return SessionFixture(**raw)


def _session_to_log_row(fixture: SessionFixture) -> list[dict]:
    rows = []
    for e in fixture.executions:
        rows.append({
            "session_id": e.execution_id,
            "conversation_id": fixture.session_id,
            "agent_id": e.agent_id,
            "agent_name": e.agent_id,
            "title": fixture.title,
            "started_at": "2026-04-20T00:00:00+00:00",
            "ended_at": "2026-04-20T00:00:10+00:00",
            "status": "completed",
            "token_count": 0,
            "tool_call_count": 0,
            "error_count": 0,
            "duration_ms": e.ended_at_offset_ms - e.started_at_offset_ms,
            "parent_session_id": e.parent_execution_id,
            "child_session_ids": [
                c.execution_id for c in fixture.executions
                if c.parent_execution_id == e.execution_id
            ],
        })
    return rows


def _load_llm_responses(fixture_dir: Path) -> list[dict]:
    path = fixture_dir / "llm-responses.jsonl"
    if not path.exists():
        return []
    records = []
    for line in path.read_text().splitlines():
        if line.strip():
            records.append(json.loads(line))
    return records


def _extract_tool_calls(llm_record: dict) -> list[dict]:
    """Pull the assistant tool_calls out of a recorded OpenAI response."""
    response = llm_record.get("response", {})
    choices = response.get("choices", [])
    if not choices:
        return []
    message = choices[0].get("message", {})
    raw_tool_calls = message.get("tool_calls") or []
    out = []
    for i, tc in enumerate(raw_tool_calls):
        fn = tc.get("function", {})
        name = fn.get("name", "")
        try:
            args = json.loads(fn.get("arguments", "{}"))
        except json.JSONDecodeError:
            args = {}
        out.append({
            "tool_name": name,
            "args": args,
            "tool_id": tc.get("id") or f"call_{i}",
        })
    return out


def _derive_messages(fixture: SessionFixture, fixture_dir: Path) -> list[dict]:
    """Synthesise the REST messages shape from the fixture's recorded
    llm-responses.jsonl, so the UI renders the same tool-calls that the
    WS stream would produce on a fresh live run.
    """
    llm_records = _load_llm_responses(fixture_dir)
    out: list[dict] = [
        {
            "id": "msg-user-0",
            "execution_id": fixture.executions[0].execution_id,
            "role": "user",
            "content": "synthetic prompt",
            "created_at": "2026-04-20T00:00:00+00:00",
        }
    ]
    records_by_exec: dict[str, list[dict]] = {}
    for rec in llm_records:
        records_by_exec.setdefault(rec.get("execution_id", ""), []).append(rec)
    for e in fixture.executions:
        recs = records_by_exec.get(e.execution_id, [])
        tool_calls = _extract_tool_calls(recs[-1]) if recs else []
        out.append({
            "id": f"msg-assistant-{e.execution_id}",
            "execution_id": e.execution_id,
            "role": "assistant",
            "content": "[tool calls]" if tool_calls else "",
            "created_at": "2026-04-20T00:00:05+00:00",
            "toolCalls": json.dumps(tool_calls),
        })
    return out


def create_rest_app(fixture_dir: Path) -> FastAPI:
    app = FastAPI()
    fixture = _load_session_fixture(fixture_dir)

    @app.get("/api/health")
    def health() -> dict:
        return {"ok": True}

    @app.get("/api/logs/sessions")
    def logs_sessions(limit: int = 50) -> list[dict]:
        return _session_to_log_row(fixture)[:limit]

    @app.get("/api/sessions/{sid}/state")
    def session_state(sid: str):
        raise HTTPException(404, detail="not available")

    @app.get("/api/sessions/{sid}/messages")
    def session_messages(sid: str, scope: str = "all") -> list[dict]:
        if sid != fixture.session_id:
            raise HTTPException(404, detail="session not found")
        return _derive_messages(fixture, fixture_dir)

    @app.get("/api/executions/v2/sessions/{sid}/messages")
    def executions_v2_session_messages(sid: str, scope: str = "all") -> list[dict]:
        if sid != fixture.session_id:
            raise HTTPException(404, detail="session not found")
        return _derive_messages(fixture, fixture_dir)

    @app.get("/api/sessions/{sid}/artifacts")
    def session_artifacts(sid: str) -> list[dict]:
        if sid != fixture.session_id:
            raise HTTPException(404, detail="session not found")
        return [
            {
                "id": a.id,
                "fileName": a.file_name,
                "fileType": a.file_type,
                "fileSize": a.file_size,
            }
            for a in fixture.artifacts
        ]

    @app.post("/api/wards/{wid}/open")
    def open_ward(wid: str) -> JSONResponse:
        return JSONResponse({"path": f"/tmp/stub-ward/{wid}"}, status_code=200)

    @app.delete("/api/sessions/{sid}")
    def delete_session(sid: str) -> JSONResponse:
        return JSONResponse(status_code=204, content=None)

    return app
