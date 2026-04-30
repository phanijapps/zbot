//! KG-side request/response shapes used by store ports.
//!
//! These are the values that flow across the `KnowledgeGraphStore`
//! port boundary for sleep-time maintenance and synthesis. They live
//! here (not in `knowledge-graph`) so the trait crate can name them
//! without taking on the full domain crate's database dependencies,
//! and so backends round-trip the same shape regardless of how they
//! materialise an entity internally.

use serde::{Deserialize, Serialize};

/// Ranking strategy for entity searches. MAGMA-style multi-view
/// queries: different question types are best served by different
/// ranking strategies. Backends that can't natively distinguish a
/// view (e.g. don't have a `mention_count` index) may degrade to
/// `Semantic` with a tracing warn — the caller still gets results,
/// they're just ordered by the backend's default heuristic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphView {
    /// Order by `mention_count DESC` (default).
    #[default]
    Semantic,
    /// Order by `last_seen_at DESC` (most recent first).
    Temporal,
    /// Order by relationship count (most-connected first).
    Entity,
    /// Reciprocal-rank-fusion merge of the other three views.
    Hybrid,
}

impl GraphView {
    /// Parse a view name string. Unknown values default to
    /// [`GraphView::Semantic`]. Infallible — using `std::str::FromStr`
    /// would require an error type the call sites don't need.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "temporal" => Self::Temporal,
            "entity" => Self::Entity,
            "hybrid" => Self::Hybrid,
            _ => Self::Semantic,
        }
    }
}

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

/// One hit returned by `KnowledgeGraphStore::search_entities_by_name_embedding`.
/// `distance` is L2-squared on normalized vectors — convert to cosine
/// similarity at the caller via `1 - distance / 2`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityNameEmbeddingHit {
    pub name: String,
    pub entity_type: String,
    pub distance: f32,
}
