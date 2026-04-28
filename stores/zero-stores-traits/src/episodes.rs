//! `EpisodeStore` trait — backend-agnostic interface for session episodes.

use async_trait::async_trait;
use serde_json::Value;

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
    async fn fetch_recent_successful_by_ward(
        &self,
        _ward_id: &str,
        _limit: usize,
    ) -> Result<Vec<Value>, String> {
        Ok(Vec::new())
    }

    /// Aggregate counts. Default returns zero so backends that don't
    /// track this gracefully degrade.
    async fn episode_stats(&self) -> Result<EpisodeStats, String> {
        Ok(EpisodeStats::default())
    }
}
