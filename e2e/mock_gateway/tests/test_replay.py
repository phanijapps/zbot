"""Cadence tests for the WS event replayer."""
import json
import time
from pathlib import Path

import pytest

from e2e.fixtures.seed_synthetic import build_simple_qa_fixture
from e2e.fixtures.types import WSEventRecord
from e2e.mock_gateway.replay import Cadence, WSEventReplayer


@pytest.mark.asyncio
async def test_compressed_cadence_emits_all_events_quickly(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    replayer = WSEventReplayer.from_fixture(
        tmp_path / "simple-qa", cadence=Cadence.COMPRESSED,
    )
    events = []
    async for ev in replayer.stream():
        events.append(ev)
    types = [e["type"] for e in events]
    assert "invoke_accepted" in types
    assert "agent_completed" in types
    assert types.index("invoke_accepted") < types.index("agent_completed")


@pytest.mark.asyncio
async def test_compressed_cadence_completes_under_two_hundred_ms(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    replayer = WSEventReplayer.from_fixture(
        tmp_path / "simple-qa", cadence=Cadence.COMPRESSED,
    )
    start = time.monotonic()
    async for _ in replayer.stream():
        pass
    assert (time.monotonic() - start) < 0.2


@pytest.mark.asyncio
async def test_realtime_cadence_honors_t_offset_ms(tmp_path: Path):
    out = tmp_path / "pause"
    out.mkdir()
    (out / "session.json").write_text(json.dumps({
        "session_id": "sess-x", "title": "x", "executions": [], "artifacts": [],
    }))
    (out / "llm-responses.jsonl").write_text("")
    (out / "tool-results.jsonl").write_text("")
    events = [
        WSEventRecord(t_offset_ms=0, type="a", payload={}),
        WSEventRecord(t_offset_ms=150, type="b", payload={}),
    ]
    (out / "ws-events.jsonl").write_text(
        "\n".join(e.model_dump_json() for e in events)
    )
    replayer = WSEventReplayer.from_fixture(
        out, cadence=Cadence.REALTIME,
    )
    start = time.monotonic()
    collected = []
    async for ev in replayer.stream():
        collected.append(ev)
    elapsed = time.monotonic() - start
    assert len(collected) == 2
    assert 0.1 < elapsed < 0.3
