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


def _derive_messages(fixture: SessionFixture) -> list[dict]:
    """Synthesise the REST messages shape from the fixture.

    For now: one user message + one assistant stub per execution. Real
    recordings will populate this from the sqlite DB in record-fixture.py.
    """
    out = [
        {
            "id": "msg-user-0",
            "execution_id": fixture.executions[0].execution_id,
            "role": "user",
            "content": "synthetic prompt",
            "created_at": "2026-04-20T00:00:00+00:00",
        }
    ]
    for e in fixture.executions:
        out.append({
            "id": f"msg-assistant-{e.execution_id}",
            "execution_id": e.execution_id,
            "role": "assistant",
            "content": "[tool calls]",
            "created_at": "2026-04-20T00:00:05+00:00",
            "toolCalls": json.dumps([
                {
                    "tool_name": "respond",
                    "args": {"message": "stub answer"},
                    "tool_id": "call_1",
                }
            ]),
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
        return _derive_messages(fixture)

    @app.get("/api/sessions/{sid}/artifacts")
    def session_artifacts(sid: str) -> list[dict]:
        if sid != fixture.session_id:
            raise HTTPException(404, detail="session not found")
        return [a.model_dump() for a in fixture.artifacts]

    @app.post("/api/wards/{wid}/open")
    def open_ward(wid: str) -> JSONResponse:
        return JSONResponse({"path": f"/tmp/stub-ward/{wid}"}, status_code=200)

    @app.delete("/api/sessions/{sid}")
    def delete_session(sid: str) -> JSONResponse:
        return JSONResponse(status_code=204, content=None)

    return app
