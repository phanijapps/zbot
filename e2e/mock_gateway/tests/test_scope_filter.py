"""Scope filter mirrors gateway/src/websocket/subscriptions.rs:should_send_to_scope."""
from e2e.mock_gateway.scope_filter import (
    EventMetadata, Scope, ScopeState, should_send_to_scope,
)


def test_all_scope_passes_everything():
    meta = EventMetadata(execution_id="exec-1", is_delegation_event=False)
    assert should_send_to_scope(meta, Scope.ALL, None)


def test_session_scope_passes_events_without_execution_id():
    meta = EventMetadata(execution_id=None, is_delegation_event=False)
    assert should_send_to_scope(meta, Scope.SESSION, None)


def test_session_scope_passes_delegation_events():
    meta = EventMetadata(execution_id="exec-child", is_delegation_event=True)
    state = ScopeState(root_execution_ids={"exec-root"})
    assert should_send_to_scope(meta, Scope.SESSION, state)


def test_session_scope_passes_root_executions_only():
    meta_root = EventMetadata(execution_id="exec-root", is_delegation_event=False)
    meta_child = EventMetadata(execution_id="exec-child", is_delegation_event=False)
    state = ScopeState(root_execution_ids={"exec-root"})
    assert should_send_to_scope(meta_root, Scope.SESSION, state)
    assert not should_send_to_scope(meta_child, Scope.SESSION, state)


def test_execution_scope_passes_only_matching_exec_id():
    meta = EventMetadata(execution_id="exec-target", is_delegation_event=False)
    assert should_send_to_scope(
        meta, Scope.execution("exec-target"), None
    )
    assert not should_send_to_scope(
        meta, Scope.execution("exec-other"), None
    )
