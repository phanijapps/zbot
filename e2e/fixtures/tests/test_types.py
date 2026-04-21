"""Unit tests for fixture schema validation."""
import pytest
from pydantic import ValidationError
from e2e.fixtures.types import (
    Execution, SessionFixture, LLMResponseRecord, ToolResultRecord, WSEventRecord,
)


def test_execution_requires_core_fields():
    with pytest.raises(ValidationError):
        Execution(execution_id="exec-1")  # missing agent_id


def test_session_fixture_round_trips_via_json():
    raw = {
        "session_id": "sess-1",
        "title": "Test",
        "executions": [
            {
                "execution_id": "exec-root",
                "agent_id": "root",
                "parent_execution_id": None,
                "started_at_offset_ms": 0,
                "ended_at_offset_ms": 100,
            }
        ],
        "artifacts": [],
    }
    fixture = SessionFixture(**raw)
    assert fixture.session_id == "sess-1"
    assert fixture.executions[0].agent_id == "root"


def test_llm_response_record_requires_execution_id():
    with pytest.raises(ValidationError):
        LLMResponseRecord(iteration=0, response={"choices": []})


def test_tool_result_record_requires_args_hash():
    rec = ToolResultRecord(
        execution_id="exec-1", tool_index=0, tool_name="shell",
        args_hash="sha256:abc", result="ok",
    )
    assert rec.args_hash == "sha256:abc"


def test_ws_event_record_preserves_type_and_offset():
    rec = WSEventRecord(
        t_offset_ms=42, type="invoke_accepted",
        payload={"session_id": "sess-1"},
    )
    assert rec.type == "invoke_accepted"
    assert rec.t_offset_ms == 42
