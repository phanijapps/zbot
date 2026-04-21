"""WebSocket protocol contract tests."""
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from e2e.fixtures.seed_synthetic import (
    SIMPLE_QA_EXEC_ROOT,
    SIMPLE_QA_SESSION_ID,
    build_simple_qa_fixture,
)
from e2e.mock_gateway.server import create_app


@pytest.fixture
def client(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    app = create_app(fixture_dir=tmp_path / "simple-qa")
    return TestClient(app)


def _drain_until(ws, predicate, limit: int = 30):
    collected = []
    for _ in range(limit):
        msg = ws.receive_json()
        collected.append(msg)
        if predicate(msg):
            break
    return collected


def test_subscribe_then_invoke_flow(client):
    with client.websocket_connect("/ws") as ws:
        ws.send_json({
            "type": "subscribe",
            "conversation_id": "research-abc",
            "scope": "all",
        })
        ack = ws.receive_json()
        assert ack["type"] == "subscribed"
        assert ack["conversation_id"] == "research-abc"

        ws.send_json({
            "type": "invoke",
            "agent_id": "root",
            "conversation_id": "research-abc",
            "message": "anything",
        })
        accepted = ws.receive_json()
        assert accepted["type"] == "invoke_accepted"
        assert accepted["session_id"] == SIMPLE_QA_SESSION_ID
        assert accepted["execution_id"] == SIMPLE_QA_EXEC_ROOT
        assert accepted["conversation_id"] == "research-abc"

        events = _drain_until(ws, lambda m: m.get("type") == "agent_completed")
        types = [e["type"] for e in events]
        assert "agent_completed" in types
        # conversation_id must have been rewritten
        for e in events:
            if "conversation_id" in e:
                assert e["conversation_id"] == "research-abc"


def test_ping_pong(client):
    with client.websocket_connect("/ws") as ws:
        ws.send_json({"type": "ping"})
        assert ws.receive_json() == {"type": "pong"}


def test_replay_status_reflects_consumed_events(client):
    with client.websocket_connect("/ws") as ws:
        ws.send_json({
            "type": "subscribe",
            "conversation_id": "research-abc",
            "scope": "all",
        })
        ws.receive_json()  # subscribed
        ws.send_json({
            "type": "invoke",
            "agent_id": "root",
            "conversation_id": "research-abc",
            "message": "x",
        })
        ws.receive_json()  # invoke_accepted
        _drain_until(ws, lambda m: m.get("type") == "agent_completed")
    r = client.get("/__replay/status")
    assert r.status_code == 200
    assert r.json()["consumed"] > 0
