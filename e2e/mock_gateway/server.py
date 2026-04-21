"""FastAPI app combining REST + WS for mock-gateway.

UI connects here exclusively in Mode UI; mock-gateway speaks the subset
of zerod's wire protocol the UI actually uses.
"""
from pathlib import Path
from typing import Optional

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware

from e2e.fixtures.types import Execution, SessionFixture
from e2e.mock_gateway.replay import Cadence, WSEventReplayer
from e2e.mock_gateway.rest_endpoints import _load_session_fixture, create_rest_app


class WSState:
    def __init__(self) -> None:
        self.consumed = 0

    def bump(self) -> None:
        self.consumed += 1


def _find_root_execution(fixture: SessionFixture) -> Optional[Execution]:
    """Return the first execution with no parent (the root)."""
    for exec_ in fixture.executions:
        if exec_.parent_execution_id is None:
            return exec_
    return None


async def _handle_subscribe(
    websocket: WebSocket,
    msg: dict,
) -> Optional[str]:
    """Process a subscribe frame and send the subscribed ack.

    Returns the conversation_id the client subscribed with, or None on error.
    """
    conv_id = msg.get("conversation_id")
    await websocket.send_json({
        "type": "subscribed",
        "conversation_id": conv_id,
        "current_sequence": 0,
        "root_execution_ids": [],
    })
    return conv_id


async def _handle_invoke(
    websocket: WebSocket,
    fixture: SessionFixture,
    root_exec: Execution,
    conv_id: str,
    fixture_dir: Path,
    cadence: Cadence,
    state: WSState,
) -> None:
    """Send invoke_accepted then stream fixture events with rewritten conversation_id."""
    await websocket.send_json({
        "type": "invoke_accepted",
        "conversation_id": conv_id,
        "session_id": fixture.session_id,
        "execution_id": root_exec.execution_id,
    })

    replayer = WSEventReplayer.from_fixture(fixture_dir, cadence=cadence)
    seq = 0
    async for ev in replayer.stream():
        # Skip the invoke_accepted recorded in JSONL — we already sent a live one.
        if ev.get("type") == "invoke_accepted":
            continue
        # Rewrite conversation_id so the UI's subscription filter matches.
        if "conversation_id" in ev:
            ev = {**ev, "conversation_id": conv_id}
        seq += 1
        await websocket.send_json({**ev, "seq": seq})
        state.bump()


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
    root_exec = _find_root_execution(fixture)
    state = WSState()
    app.state.ws = state
    app.state.fixture = fixture
    app.state.cadence = cadence

    @app.websocket("/")
    @app.websocket("/ws")
    async def ws_endpoint(websocket: WebSocket) -> None:
        await websocket.accept()
        conv_id: Optional[str] = None
        try:
            while True:
                msg = await websocket.receive_json()
                msg_type = msg.get("type")

                if msg_type == "subscribe":
                    conv_id = await _handle_subscribe(websocket, msg)

                elif msg_type == "invoke":
                    invoke_conv_id = msg.get("conversation_id", conv_id)
                    if root_exec is not None and invoke_conv_id:
                        await _handle_invoke(
                            websocket, fixture, root_exec,
                            invoke_conv_id, fixture_dir, cadence, state,
                        )

                elif msg_type == "ping":
                    await websocket.send_json({"type": "pong"})

        except WebSocketDisconnect:
            return

    @app.get("/__replay/status")
    def replay_status() -> dict:
        return {
            "consumed": state.consumed,
            "fixture": str(fixture_dir),
        }

    return app
