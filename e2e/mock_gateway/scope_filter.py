"""Subscription scope filter — direct port of
gateway/src/websocket/subscriptions.rs:should_send_to_scope.

Keep in sync with the Rust source. Integration tests validate parity
against the real daemon (Task 24 full-mode spec).
"""
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


class ScopeKind(str, Enum):
    ALL = "all"
    SESSION = "session"
    EXECUTION = "execution"


@dataclass
class Scope:
    kind: ScopeKind
    target_id: Optional[str] = None

    @staticmethod
    def execution(target_id: str) -> "Scope":
        return Scope(ScopeKind.EXECUTION, target_id=target_id)


Scope.ALL = Scope(ScopeKind.ALL)
Scope.SESSION = Scope(ScopeKind.SESSION)


@dataclass
class ScopeState:
    root_execution_ids: set[str] = field(default_factory=set)

    def is_root(self, exec_id: str) -> bool:
        return exec_id in self.root_execution_ids


@dataclass
class EventMetadata:
    execution_id: Optional[str]
    is_delegation_event: bool


def should_send_to_scope(
    metadata: EventMetadata,
    scope: Scope,
    state: Optional[ScopeState],
) -> bool:
    if scope.kind == ScopeKind.ALL:
        return True
    if scope.kind == ScopeKind.SESSION:
        if metadata.is_delegation_event:
            return True
        if metadata.execution_id is None:
            return True
        if state is None:
            return True
        return state.is_root(metadata.execution_id)
    if scope.kind == ScopeKind.EXECUTION:
        return metadata.execution_id == scope.target_id
    return False
