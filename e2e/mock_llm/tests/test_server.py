"""Integration tests for mock-llm HTTP server."""
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from e2e.fixtures.seed_synthetic import (
    SIMPLE_QA_EXEC_ROOT,
    build_simple_qa_fixture,
)
from e2e.mock_llm.server import create_app


@pytest.fixture
def client(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    app = create_app(fixture_dir=tmp_path / "simple-qa")
    return TestClient(app)


def test_health_ok(client):
    r = client.get("/health")
    assert r.status_code == 200


def test_chat_completions_returns_recorded_response_non_streaming(client):
    r = client.post("/v1/chat/completions", json={
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "anything"}],
        "zbot_execution_id": SIMPLE_QA_EXEC_ROOT,
    })
    assert r.status_code == 200
    body = r.json()
    assert body["choices"][0]["finish_reason"] == "tool_calls"
    assert body["choices"][0]["message"]["tool_calls"][0]["function"]["name"] == "respond"


def test_chat_completions_drift_on_exhaust(client):
    client.post("/v1/chat/completions", json={
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "x"}],
        "zbot_execution_id": SIMPLE_QA_EXEC_ROOT,
    })
    r = client.post("/v1/chat/completions", json={
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "x"}],
        "zbot_execution_id": SIMPLE_QA_EXEC_ROOT,
    })
    assert r.status_code == 410


def test_replay_status_endpoint(client):
    client.post("/v1/chat/completions", json={
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "x"}],
        "zbot_execution_id": SIMPLE_QA_EXEC_ROOT,
    })
    r = client.get("/__replay/status")
    assert r.status_code == 200
    status = r.json()
    assert status["expected_requests"] == 1
    assert status["received_requests"] == 1
