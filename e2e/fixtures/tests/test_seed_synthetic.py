"""Synthetic fixture generator emits schema-valid bundles."""
import json
from pathlib import Path

from e2e.fixtures.seed_synthetic import build_simple_qa_fixture
from e2e.fixtures.types import SessionFixture, WSEventRecord


def test_simple_qa_emits_four_files(tmp_path: Path):
    out = tmp_path / "simple-qa"
    build_simple_qa_fixture(out)
    assert (out / "session.json").exists()
    assert (out / "llm-responses.jsonl").exists()
    assert (out / "tool-results.jsonl").exists()
    assert (out / "ws-events.jsonl").exists()


def test_simple_qa_session_json_validates(tmp_path: Path):
    out = tmp_path / "simple-qa"
    build_simple_qa_fixture(out)
    raw = json.loads((out / "session.json").read_text())
    fixture = SessionFixture(**raw)
    assert fixture.session_id.startswith("sess-")
    assert len(fixture.executions) == 1
    assert fixture.executions[0].agent_id == "root"


def test_simple_qa_ws_events_include_invoke_and_respond(tmp_path: Path):
    out = tmp_path / "simple-qa"
    build_simple_qa_fixture(out)
    events = [
        WSEventRecord(**json.loads(line))
        for line in (out / "ws-events.jsonl").read_text().splitlines()
        if line.strip()
    ]
    types = [e.type for e in events]
    assert "invoke_accepted" in types
    assert "agent_started" in types
    assert "agent_completed" in types
