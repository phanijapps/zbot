"""Fixture replay store + drift tracking for mock-llm.

Loads llm-responses.jsonl into an in-memory map keyed by execution_id.
Each call to next_response consumes the next record for that execution_id.
Tracks drift (hash mismatch in strict mode, exhaustion, unknown exec).
"""
from dataclasses import dataclass
from pathlib import Path
from typing import Optional
import json

from e2e.fixtures.types import LLMResponseRecord


@dataclass
class MatchResult:
    ok: bool
    response: Optional[dict] = None
    reason: Optional[str] = None
    expected_hash: Optional[str] = None
    received_hash: Optional[str] = None


@dataclass
class DriftRecord:
    exec_id: str
    iteration: int
    reason: str
    expected_hash: Optional[str] = None
    received_hash: Optional[str] = None


class ReplayStore:
    def __init__(self, strict_hashing: bool = False) -> None:
        self._records: dict[str, list[LLMResponseRecord]] = {}
        self._cursor: dict[str, int] = {}
        self._drift: list[DriftRecord] = []
        self._strict = strict_hashing
        self._received_count = 0

    @classmethod
    def from_fixture(
        cls, fixture_dir: Path, *, strict_hashing: bool = False
    ) -> "ReplayStore":
        store = cls(strict_hashing=strict_hashing)
        path = fixture_dir / "llm-responses.jsonl"
        if not path.exists():
            raise FileNotFoundError(f"fixture missing llm-responses.jsonl: {path}")
        for line in path.read_text().splitlines():
            if not line.strip():
                continue
            rec = LLMResponseRecord(**json.loads(line))
            store._records.setdefault(rec.execution_id, []).append(rec)
        for exec_id in store._records:
            store._cursor[exec_id] = 0
        return store

    def total_requests(self) -> int:
        return sum(len(v) for v in self._records.values())

    def next_response(self, *, exec_id: str, messages_hash: str) -> MatchResult:
        self._received_count += 1
        records = self._records.get(exec_id)
        if records is None:
            self._drift.append(DriftRecord(
                exec_id=exec_id, iteration=-1, reason="unknown_exec",
            ))
            return MatchResult(ok=False, reason="unknown_exec")
        idx = self._cursor.get(exec_id, 0)
        if idx >= len(records):
            self._drift.append(DriftRecord(
                exec_id=exec_id, iteration=idx, reason="exhausted",
            ))
            return MatchResult(ok=False, reason="exhausted")
        rec = records[idx]
        if self._strict and rec.messages_hash and rec.messages_hash != messages_hash:
            self._drift.append(DriftRecord(
                exec_id=exec_id, iteration=idx, reason="hash_mismatch",
                expected_hash=rec.messages_hash, received_hash=messages_hash,
            ))
            return MatchResult(
                ok=False, reason="hash_mismatch",
                expected_hash=rec.messages_hash, received_hash=messages_hash,
            )
        self._cursor[exec_id] = idx + 1
        return MatchResult(ok=True, response=rec.response)

    def status(self) -> dict:
        return {
            "expected_requests": self.total_requests(),
            "received_requests": self._received_count,
            "drift_count": len(self._drift),
            "first_drift": (
                {"exec_id": self._drift[0].exec_id,
                 "iteration": self._drift[0].iteration,
                 "reason": self._drift[0].reason}
                if self._drift else None
            ),
            "per_execution_progress": {
                exec_id: self._cursor[exec_id]
                for exec_id in self._records
            },
        }
