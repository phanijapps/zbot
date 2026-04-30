//! `EpisodeStore` trait — backend-agnostic interface for session episodes.

use async_trait::async_trait;
use serde_json::Value;
// Domain types live in `zero-stores-domain`; re-export here so the
// trait surface keeps working for callers that import from this crate.
pub use zero_stores_domain::{SessionEpisode, SuccessfulEpisode};

/// Aggregate health metrics returned alongside episode reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpisodeStats {
    /// Total session_episodes rows.
    pub total: i64,
}

/// Backend-agnostic interface for the session-episode subsystem.
///
/// Each row carries the `SessionEpisode` JSON shape from
/// `zero-stores-domain` so the trait surface stays free of SQLite-specific
/// types. Methods that return `Vec<Value>` emit one Value per episode in
/// the canonical shape; callers deserialize via `serde_json::from_value`.
///
/// All methods have safe defaults (empty Vec / Ok with zero count) so
/// impls that don't yet support a method gracefully degrade rather than
/// panic — matching the pattern used by `MemoryFactStore`.
#[async_trait]
pub trait EpisodeStore: Send + Sync {
    /// List episodes for a ward, capped at `limit`. Used by the ward
    /// content endpoint and Observatory views.
    async fn list_by_ward(&self, _ward_id: &str, _limit: usize) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Insert an episode. The `episode` Value carries the full
    /// `SessionEpisode` shape; `embedding` is the optional L2-normalized
    /// vector to persist alongside. Returns the persisted row id.
    async fn insert_episode(
        &self,
        _episode: Value,
        _embedding: Option<Vec<f32>>,
    ) -> Result<String, String> {
        Err("insert_episode not implemented for this store".to_string())
    }

    /// Vector-similarity search for episodes scoped to an agent.
    /// Each returned Value carries an `episode` field (SessionEpisode shape)
    /// and a `score` field (cosine similarity ∈ [0, 1]).
    async fn search_episodes_by_similarity(
        &self,
        _agent_id: &str,
        _embedding: &[f32],
        _threshold: f32,
        _limit: usize,
    ) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Recent successful episodes for a ward. Used by the previous-episodes
    /// recall path to seed agent context with what worked last time.
    /// Returns typed [`SessionEpisode`] (via `zero-stores-domain`) so
    /// callers don't pay the round-trip-through-Value tax.
    async fn fetch_recent_successful_by_ward(
        &self,
        _ward_id: &str,
        _limit: usize,
    ) -> Result<Vec<SessionEpisode>, String> {
        Ok(Vec::new())
    }

    /// Aggregate counts. Default returns zero so backends that don't
    /// track this gracefully degrade.
    async fn episode_stats(&self) -> Result<EpisodeStats, String> {
        Ok(EpisodeStats::default())
    }

    // ---- Sleep-time pattern + synthesis (Phase D4) ----------------------
    //
    // Reads needed by the `Synthesizer` and `PatternExtractor` ops.
    // Default impls return empty so backends that haven't implemented
    // yet make the synthesis/pattern cycle a quiet no-op rather than
    // crashing.

    /// Recent successful episodes (within `lookback_days`) with their
    /// task-summary embedding loaded. Used by `PatternExtractor` to
    /// surface candidate pairs by semantic similarity. `embedding` is
    /// `None` for rows that aren't indexed yet. Default: empty.
    async fn list_successful_episodes_with_embedding(
        &self,
        _lookback_days: i64,
        _limit: usize,
    ) -> Result<Vec<SuccessfulEpisode>, String> {
        Ok(Vec::new())
    }

    /// Task summaries for a set of session ids. Used by `Synthesizer`
    /// to build per-candidate LLM context. Order is unspecified;
    /// duplicates per session are allowed if a session has multiple
    /// episodes. Default: empty.
    async fn task_summaries_for_sessions(
        &self,
        _session_ids: &[String],
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}
