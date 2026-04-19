// =============================================================================
// turn-tree — pure helpers for deriving the subagent tree from a flat turn[].
//
// Turns are stored flat in ResearchSessionState.turns; parent/child nesting is
// derived at render time via AgentTurn.parentExecutionId. Keeping these helpers
// in a separate, React-free module makes them trivially unit-testable and keeps
// AgentTurnBlock free of tree bookkeeping.
// =============================================================================

import type { AgentTurn } from "./types";

/** Ascending by startedAt. Stable, so equal timestamps keep array order. */
function byStartedAtAsc(a: AgentTurn, b: AgentTurn): number {
  return a.startedAt - b.startedAt;
}

/**
 * Root turns (no parent). Sorted by startedAt ascending so the main column
 * renders in chronological order.
 */
export function rootTurns(allTurns: AgentTurn[]): AgentTurn[] {
  return allTurns
    .filter((t) => t.parentExecutionId === null)
    .slice()
    .sort(byStartedAtAsc);
}

/**
 * Direct children of `turn`. Does NOT recurse — callers walk the tree by
 * calling `childrenOf` on each child. Sorted by startedAt ascending.
 */
export function childrenOf(turn: AgentTurn, allTurns: AgentTurn[]): AgentTurn[] {
  return allTurns
    .filter((t) => t.parentExecutionId === turn.id)
    .slice()
    .sort(byStartedAtAsc);
}
