"""WebSocket protocol contract tests."""
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from e2e.fixtures.seed_synthetic import (
    SIMPLE_QA_SESSION_ID,
    build_simple_qa_fixture,
)
from e2e.mock_gateway.server import create_app


@pytest.fixture
def client(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    app = create_app(fixture_dir=tmp_path / "simple-qa")
    return TestClient(app)


def test_ws_accepts_subscribe_and_streams_events(client):
    with client.websocket_connect("/ws") as ws:
        ws.send_json({
            "type": "subscribe",
            "conversation_id": SIMPLE_QA_SESSION_ID,
            "scope": "all",
        })
        ack = ws.receive_json()
        assert ack["type"] == "subscribed"
        types = []
        for _ in range(20):
            try:
                msg = ws.receive_json()
            except Exception:
                break
            types.append(msg.get("type"))
            if msg.get("type") == "agent_completed":
                break
        assert "agent_completed" in types


def test_replay_status_reflects_consumed_events(client):
    with client.websocket_connect("/ws") as ws:
        ws.send_json({
            "type": "subscribe",
            "conversation_id": SIMPLE_QA_SESSION_ID,
            "scope": "all",
        })
        ws.receive_json()
        for _ in range(20):
            try:
                msg = ws.receive_json()
            except Exception:
                break
            if msg.get("type") == "agent_completed":
                break
    r = client.get("/__replay/status")
    assert r.status_code == 200
    assert r.json()["consumed"] > 0
