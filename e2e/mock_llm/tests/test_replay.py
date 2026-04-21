"""Unit tests for mock-llm fixture matcher."""
from pathlib import Path

from e2e.fixtures.seed_synthetic import (
    SIMPLE_QA_EXEC_ROOT,
    build_simple_qa_fixture,
)
from e2e.mock_llm.replay import MatchResult, ReplayStore


def test_loads_fixture_and_reports_expected_count(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    store = ReplayStore.from_fixture(tmp_path / "simple-qa")
    assert store.total_requests() == 1


def test_matches_first_iteration_by_exec_id(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    store = ReplayStore.from_fixture(tmp_path / "simple-qa")
    result = store.next_response(exec_id=SIMPLE_QA_EXEC_ROOT, messages_hash="any")
    assert isinstance(result, MatchResult)
    assert result.ok
    assert result.response["choices"][0]["finish_reason"] == "tool_calls"


def test_miss_after_fixture_exhausted(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    store = ReplayStore.from_fixture(tmp_path / "simple-qa")
    store.next_response(exec_id=SIMPLE_QA_EXEC_ROOT, messages_hash="any")
    result = store.next_response(exec_id=SIMPLE_QA_EXEC_ROOT, messages_hash="any")
    assert not result.ok
    assert result.reason == "exhausted"


def test_drift_reported_when_hash_mismatches_in_strict_mode(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    store = ReplayStore.from_fixture(
        tmp_path / "simple-qa", strict_hashing=True
    )
    store._records[SIMPLE_QA_EXEC_ROOT][0].messages_hash = "sha256:expected"
    result = store.next_response(
        exec_id=SIMPLE_QA_EXEC_ROOT, messages_hash="sha256:different"
    )
    assert not result.ok
    assert result.reason == "hash_mismatch"


def test_status_reports_per_execution_progress(tmp_path: Path):
    build_simple_qa_fixture(tmp_path / "simple-qa")
    store = ReplayStore.from_fixture(tmp_path / "simple-qa")
    store.next_response(exec_id=SIMPLE_QA_EXEC_ROOT, messages_hash="any")
    status = store.status()
    assert status["expected_requests"] == 1
    assert status["received_requests"] == 1
    assert status["drift_count"] == 0
    assert status["per_execution_progress"][SIMPLE_QA_EXEC_ROOT] == 1
