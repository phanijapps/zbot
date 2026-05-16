// ============================================================================
// Belief Network — wire types mirroring the Rust HTTP shapes.
//
// Backend definitions live in `gateway/src/http/beliefs.rs`. The wire
// contract is: ISO-8601 timestamps as strings, JSON `null` for absent
// optionals (parsed as `null` here, not `undefined`).
// ============================================================================

export interface Belief {
  id: string;
  partition_id: string;
  subject: string;
  content: string;
  confidence: number;
  valid_from: string | null;
  valid_until: string | null;
  source_fact_ids: string[];
  synthesizer_version: number;
  reasoning: string | null;
  stale: boolean;
  created_at: string;
  updated_at: string;
  superseded_by: string | null;
}

export type ContradictionType = "logical" | "tension" | "temporal";

export type ContradictionResolution =
  | "a_won"
  | "b_won"
  | "compatible"
  | "unresolved";

export interface BeliefContradiction {
  id: string;
  belief_a_id: string;
  belief_b_id: string;
  contradiction_type: ContradictionType;
  severity: number;
  judge_reasoning: string | null;
  detected_at: string;
  resolved_at: string | null;
  resolution: ContradictionResolution | null;
}

export interface SourceFactSummary {
  id: string;
  content: string;
  category: string;
  confidence: number;
}

export interface BeliefDetailResponse {
  belief: Belief;
  source_facts: SourceFactSummary[];
  contradictions: BeliefContradiction[];
}

export interface BeliefListResponse {
  beliefs: Belief[];
}

export interface ContradictionListResponse {
  contradictions: BeliefContradiction[];
}
