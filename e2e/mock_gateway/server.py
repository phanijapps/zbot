"""FastAPI app combining REST + WS for mock-gateway.

UI connects here exclusively in Mode UI; mock-gateway speaks the subset
of zerod's wire protocol the UI actually uses.
"""
from pathlib import Path

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware

from e2e.fixtures.types import SessionFixture
from e2e.mock_gateway.replay import Cadence, WSEventReplayer
from e2e.mock_gateway.rest_endpoints import _load_session_fixture, create_rest_app


class WSState:
    def __init__(self) -> None:
        self.consumed = 0

    def bump(self) -> None:
        self.consumed += 1


def create_app(
    fixture_dir: Path, *, cadence: Cadence = Cadence.COMPRESSED,
) -> FastAPI:
    app = create_rest_app(fixture_dir)
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"], allow_credentials=True,
        allow_methods=["*"], allow_headers=["*"],
    )
    fixture: SessionFixture = _load_session_fixture(fixture_dir)
    state = WSState()
    app.state.ws = state
    app.state.fixture = fixture
    app.state.cadence = cadence

    @app.websocket("/ws")
    async def ws_endpoint(websocket: WebSocket) -> None:
        await websocket.accept()
        try:
            msg = await websocket.receive_json()
            if msg.get("type") != "subscribe":
                await websocket.close(code=1008, reason="expected subscribe")
                return
            await websocket.send_json({
                "type": "subscribed",
                "conversation_id": msg.get("conversation_id"),
                "seq": 0,
            })
            replayer = WSEventReplayer.from_fixture(
                fixture_dir, cadence=cadence,
            )
            seq = 0
            async for ev in replayer.stream():
                seq += 1
                await websocket.send_json({**ev, "seq": seq})
                state.bump()
        except WebSocketDisconnect:
            return

    @app.get("/__replay/status")
    def replay_status() -> dict:
        return {
            "consumed": state.consumed,
            "fixture": str(fixture_dir),
        }

    return app
