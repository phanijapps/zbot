"""Pydantic schemas for e2e fixture files.

Each fixture bundle lives in `e2e/fixtures/<scenario>/` and contains:
  - session.json          metadata (SessionFixture)
  - llm-responses.jsonl   one LLMResponseRecord per line
  - tool-results.jsonl    one ToolResultRecord per line
  - ws-events.jsonl       one WSEventRecord per line
"""
from typing import Optional
from pydantic import BaseModel, Field


class Execution(BaseModel):
    execution_id: str
    agent_id: str
    parent_execution_id: Optional[str] = None
    started_at_offset_ms: int
    ended_at_offset_ms: int


class Artifact(BaseModel):
    id: str
    file_name: str
    file_type: str
    file_size: int = 0


class SessionFixture(BaseModel):
    """Contents of session.json."""

    session_id: str
    title: str
    executions: list[Execution]
    artifacts: list[Artifact] = Field(default_factory=list)


class LLMResponseRecord(BaseModel):
    """One line in llm-responses.jsonl."""

    execution_id: str
    iteration: int
    messages_hash: Optional[str] = None
    response: dict


class ToolResultRecord(BaseModel):
    """One line in tool-results.jsonl."""

    execution_id: str
    tool_index: int
    tool_name: str
    args_hash: str
    result: str


class WSEventRecord(BaseModel):
    """One line in ws-events.jsonl (the server-to-UI event stream)."""

    t_offset_ms: int
    type: str
    payload: dict = Field(default_factory=dict)
