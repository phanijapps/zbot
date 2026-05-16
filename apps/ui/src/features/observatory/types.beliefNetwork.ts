// ============================================================================
// Belief Network types — wire shape for /api/belief-network/{stats,activity}
// ============================================================================
//
// Mirrors the Rust DTOs in gateway/src/http/belief_network.rs. Field names
// match the Rust snake_case serialization so the UI can consume the
// payload without an adapter layer.

export interface BeliefSynthesisStats {
  subjects_examined: number;
  beliefs_synthesized: number;
  beliefs_short_circuited: number;
  beliefs_llm_synthesized: number;
  llm_calls: number;
  errors: number;
  stale_beliefs_resynthesized: number;
}

export interface ContradictionDetectionStats {
  neighborhoods_examined: number;
  pairs_examined: number;
  pairs_skipped_existing: number;
  llm_calls: number;
  contradictions_logical: number;
  contradictions_tension: number;
  duplicates_logged: number;
  compatibles_logged: number;
  errors: number;
  budget_exhausted: boolean;
}

export interface BeliefPropagationStats {
  beliefs_invalidated: number;
  beliefs_retracted: number;
  beliefs_marked_stale: number;
  max_propagation_depth: number;
  errors: number;
}

/** History entries flatten the timestamp alongside the stat fields. */
export type BeliefSynthesisHistoryEntry = BeliefSynthesisStats & {
  timestamp: string;
};

export type ContradictionHistoryEntry = ContradictionDetectionStats & {
  timestamp: string;
};

export type PropagationHistoryEntry = BeliefPropagationStats & {
  timestamp: string;
};

export interface WorkerStats<L, H> {
  latest: L;
  history: H[];
}

export interface BeliefNetworkTotals {
  total_beliefs: number;
  total_contradictions: number;
  total_unresolved_contradictions: number;
}

export interface BeliefNetworkStatsResponse {
  enabled: boolean;
  synthesizer: WorkerStats<BeliefSynthesisStats, BeliefSynthesisHistoryEntry>;
  contradiction_detector: WorkerStats<
    ContradictionDetectionStats,
    ContradictionHistoryEntry
  >;
  propagator: WorkerStats<BeliefPropagationStats, PropagationHistoryEntry>;
  totals: BeliefNetworkTotals;
}

export type BeliefActivityKind =
  | "synthesized"
  | "retracted"
  | "marked_stale"
  | "contradiction_detected"
  | "contradiction_resolved"
  | "propagation_cascade";

export interface BeliefActivityEvent {
  kind: BeliefActivityKind;
  timestamp: string;
  belief_id?: string;
  subject?: string;
  summary: string;
}
