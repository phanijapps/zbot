"""FastAPI app speaking /v1/chat/completions, replaying recorded fixtures.

The real gateway identifies which execution is making an LLM request via
a synthetic field `zbot_execution_id` in the POST body. (When the real
gateway makes the call, our adapter injects that field. For unit tests
we set it directly.)
"""
import hashlib
import json
from pathlib import Path

from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse

from e2e.mock_llm.replay import ReplayStore


class AppState:
    def __init__(self, store: ReplayStore) -> None:
        self.store = store


def _messages_hash(messages: list[dict]) -> str:
    payload = json.dumps(messages, sort_keys=True)
    return "sha256:" + hashlib.sha256(payload.encode()).hexdigest()


def create_app(
    fixture_dir: Path, *, strict_hashing: bool = False
) -> FastAPI:
    app = FastAPI()
    state = AppState(store=ReplayStore.from_fixture(
        fixture_dir, strict_hashing=strict_hashing
    ))
    app.state.replay = state

    @app.get("/health")
    def health() -> dict:
        return {"status": "ok", "fixture": str(fixture_dir)}

    @app.get("/v1/models")
    def list_models() -> dict:
        return {
            "object": "list",
            "data": [
                {"id": "gpt-4", "object": "model"},
                {"id": "gpt-4o-mini", "object": "model"},
                {"id": "glm-4-plus", "object": "model"},
            ],
        }

    @app.post("/v1/chat/completions")
    async def chat_completions(request: Request) -> JSONResponse:
        body = await request.json()
        messages = body.get("messages", [])
        exec_id = body.get("zbot_execution_id") or request.headers.get(
            "x-zbot-execution-id"
        )
        if not exec_id:
            raise HTTPException(400, detail="missing zbot_execution_id")
        result = app.state.replay.store.next_response(
            exec_id=exec_id, messages_hash=_messages_hash(messages),
        )
        if result.ok:
            return JSONResponse(result.response)
        if result.reason == "exhausted":
            return JSONResponse(
                {"error": "fixture exhausted", "exec_id": exec_id},
                status_code=410,
            )
        if result.reason == "hash_mismatch":
            return JSONResponse(
                {
                    "error": "drift",
                    "expected_hash": result.expected_hash,
                    "received_hash": result.received_hash,
                    "exec_id": exec_id,
                },
                status_code=409,
            )
        return JSONResponse(
            {"error": result.reason, "exec_id": exec_id}, status_code=500,
        )

    @app.get("/__replay/status")
    def replay_status() -> dict:
        return app.state.replay.store.status()

    return app
