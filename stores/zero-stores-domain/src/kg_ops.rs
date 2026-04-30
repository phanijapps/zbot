//! KG-side request/response shapes used by store ports.
//!
//! These are the values that flow across the `KnowledgeGraphStore`
//! port boundary for sleep-time maintenance and synthesis. They live
//! here (not in `knowledge-graph`) so the trait crate can name them
//! without taking on the full domain crate's database dependencies,
//! and so backends round-trip the same shape regardless of how they
//! materialise an entity internally.

use serde::{Deserialize, Serialize};

/// One row returned by `KnowledgeGraphStore::list_strategy_candidates`.
/// Captures just the fields the Synthesizer needs to seed an LLM call —
/// no embeddings, no full Entity payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyCandidate {
    pub entity_id: String,
    pub agent_id: String,
    pub name: String,
    pub entity_type: String,
    pub n_sessions: i64,
}

/// Result of `KnowledgeGraphStore::relationship_context_for_entity`.
/// `summaries` holds human-readable relationship strings (e.g.
/// `src --[uses]--> tgt`); `session_ids` is the set of distinct
/// sessions whose episodes referenced any of those relationships
/// within the lookback window.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationshipContext {
    pub summaries: Vec<String>,
    pub session_ids: Vec<String>,
}

/// One pair returned by `KnowledgeGraphStore::find_duplicate_candidates`.
/// Used by the sleep-time Compactor to surface merge candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCandidate {
    pub loser_entity_id: String,
    pub winner_entity_id: String,
    pub cosine_similarity: f32,
}

/// One row returned by `KnowledgeGraphStore::list_orphan_old_candidates`.
/// Used by the sleep-time DecayEngine to surface prune candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayCandidate {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub mention_count: i64,
}
