"""Generate schema-valid synthetic fixtures for early harness work.

Real fixtures come from record-fixture.py (Task 21). These synthetic ones
let the harness run end-to-end before any live session has been captured.
"""
import hashlib
import json
from pathlib import Path
from typing import Iterable

from pydantic import BaseModel

from e2e.fixtures.types import (
    Execution, LLMResponseRecord, SessionFixture,
    ToolResultRecord, WSEventRecord,
)

SIMPLE_QA_SESSION_ID = "sess-synthetic-simple-qa-0000"
SIMPLE_QA_EXEC_ROOT = "exec-synthetic-root-0000"
SIMPLE_QA_PROMPT = "what is 2+2? one-line answer"
SIMPLE_QA_ANSWER = "4"


def _hash(obj: object) -> str:
    return "sha256:" + hashlib.sha256(
        json.dumps(obj, sort_keys=True).encode()
    ).hexdigest()


def _write_jsonl(path: Path, records: Iterable[BaseModel | dict]) -> None:
    with path.open("w") as f:
        for r in records:
            payload = r.model_dump() if isinstance(r, BaseModel) else r
            f.write(json.dumps(payload) + "\n")


def build_simple_qa_fixture(out_dir: Path) -> None:
    """Root-only scenario: user → agent_started → respond → agent_completed."""
    out_dir.mkdir(parents=True, exist_ok=True)

    session = SessionFixture(
        session_id=SIMPLE_QA_SESSION_ID,
        title="Simple Q+A",
        executions=[
            Execution(
                execution_id=SIMPLE_QA_EXEC_ROOT,
                agent_id="root",
                parent_execution_id=None,
                started_at_offset_ms=0,
                ended_at_offset_ms=1500,
            )
        ],
        artifacts=[],
    )
    (out_dir / "session.json").write_text(session.model_dump_json(indent=2))

    respond_args = {"message": SIMPLE_QA_ANSWER}
    llm_response = {
        "id": "chatcmpl-synthetic",
        "object": "chat.completion",
        "choices": [
            {
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "respond",
                                "arguments": json.dumps(respond_args),
                            },
                        }
                    ],
                },
            }
        ],
    }
    llm_records = [
        LLMResponseRecord(
            execution_id=SIMPLE_QA_EXEC_ROOT,
            iteration=0,
            messages_hash=None,
            response=llm_response,
        )
    ]
    _write_jsonl(out_dir / "llm-responses.jsonl", llm_records)

    tool_records = [
        ToolResultRecord(
            execution_id=SIMPLE_QA_EXEC_ROOT,
            tool_index=0,
            tool_name="respond",
            args_hash=_hash(respond_args),
            result=json.dumps({"ok": True}),
        )
    ]
    _write_jsonl(out_dir / "tool-results.jsonl", tool_records)

    ws_events = [
        WSEventRecord(t_offset_ms=0, type="invoke_accepted",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "conversation_id": "conv-synth"}),
        WSEventRecord(t_offset_ms=50, type="agent_started",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "execution_id": SIMPLE_QA_EXEC_ROOT,
                               "agent_id": "root"}),
        WSEventRecord(t_offset_ms=200, type="thinking",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "execution_id": SIMPLE_QA_EXEC_ROOT,
                               "content": "simple arithmetic"}),
        WSEventRecord(t_offset_ms=400, type="tool_call",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "execution_id": SIMPLE_QA_EXEC_ROOT,
                               "tool_name": "respond",
                               "tool_id": "call_1",
                               "args": respond_args}),
        WSEventRecord(t_offset_ms=450, type="tool_result",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "execution_id": SIMPLE_QA_EXEC_ROOT,
                               "tool_id": "call_1",
                               "result": json.dumps({"ok": True})}),
        WSEventRecord(t_offset_ms=500, type="turn_complete",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "execution_id": SIMPLE_QA_EXEC_ROOT,
                               "final_message": ""}),
        WSEventRecord(t_offset_ms=510, type="agent_completed",
                      payload={"session_id": SIMPLE_QA_SESSION_ID,
                               "execution_id": SIMPLE_QA_EXEC_ROOT,
                               "agent_id": "root"}),
    ]
    _write_jsonl(out_dir / "ws-events.jsonl", ws_events)


if __name__ == "__main__":
    here = Path(__file__).parent
    build_simple_qa_fixture(here / "simple-qa")
    print(f"Wrote synthetic simple-qa fixture to {here / 'simple-qa'}")
