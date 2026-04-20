"""REST endpoint contract tests for mock-gateway."""
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from e2e.fixtures.seed_synthetic import (
    SIMPLE_QA_SESSION_ID,
    build_simple_qa_fixture,
)
from e2e.mock_gateway.rest_endpoints import create_rest_app


@pytest.fixture
def client(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    app = create_rest_app(fixture_dir=tmp_path / "simple-qa")
    return TestClient(app)


def test_health(client):
    assert client.get("/api/health").status_code == 200


def test_logs_sessions_returns_fixture_row(client):
    r = client.get("/api/logs/sessions?limit=10")
    assert r.status_code == 200
    rows = r.json()
    assert any(row["conversation_id"] == SIMPLE_QA_SESSION_ID for row in rows)


def test_session_state_returns_404(client):
    r = client.get(f"/api/sessions/{SIMPLE_QA_SESSION_ID}/state")
    assert r.status_code == 404


def test_session_messages_returns_derived_rows(client):
    r = client.get(f"/api/sessions/{SIMPLE_QA_SESSION_ID}/messages?scope=all")
    assert r.status_code == 200
    messages = r.json()
    assert len(messages) >= 1


def test_artifacts_returns_empty_for_simple_qa(client):
    r = client.get(f"/api/sessions/{SIMPLE_QA_SESSION_ID}/artifacts")
    assert r.status_code == 200
    assert r.json() == []
