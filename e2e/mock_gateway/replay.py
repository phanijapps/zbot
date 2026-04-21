"""WS event replayer for mock-gateway.

Loads ws-events.jsonl and yields events at the selected cadence. Each
event is yielded as a dict with at least `type` + any `payload` fields.
"""
import asyncio
import json
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import AsyncIterator

from e2e.fixtures.types import WSEventRecord


COMPRESSED_GAP_MS = 5


class Cadence(str, Enum):
    REALTIME = "realtime"
    COMPRESSED = "compressed"
    PACED = "paced"


@dataclass
class WSEventReplayer:
    events: list[WSEventRecord]
    cadence: Cadence

    @classmethod
    def from_fixture(
        cls, fixture_dir: Path, *, cadence: Cadence = Cadence.COMPRESSED,
    ) -> "WSEventReplayer":
        path = fixture_dir / "ws-events.jsonl"
        events = []
        if path.exists():
            for line in path.read_text().splitlines():
                if line.strip():
                    events.append(WSEventRecord(**json.loads(line)))
        return cls(events=events, cadence=cadence)

    async def stream(self) -> AsyncIterator[dict]:
        prev_offset = 0
        for ev in self.events:
            if self.cadence == Cadence.REALTIME:
                gap_ms = max(0, ev.t_offset_ms - prev_offset)
            elif self.cadence == Cadence.COMPRESSED:
                gap_ms = COMPRESSED_GAP_MS if prev_offset > 0 else 0
            else:
                gap_ms = 0
            if gap_ms > 0:
                await asyncio.sleep(gap_ms / 1000)
            yield {"type": ev.type, **ev.payload}
            prev_offset = ev.t_offset_ms
