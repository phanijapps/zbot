// ============================================================================
// GATEWAY EPISODE STORE
// SQLite-backed implementation of the EpisodeStore trait.
// Wraps EpisodeRepository so the SQLite-coupled storage logic stays here
// and the gateway/runtime sees only the trait.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use zero_stores_traits::{EpisodeStats, EpisodeStore};

use crate::episode_repository::EpisodeRepository;
use zero_stores_domain::SessionEpisode;

/// SQLite-backed `EpisodeStore`. Wraps the existing `EpisodeRepository`
/// so the same r2d2 pool + vec0 index serves both legacy concrete
/// callers (still many) and the new trait-routed paths.
pub struct GatewayEpisodeStore {
    repo: Arc<EpisodeRepository>,
}

impl GatewayEpisodeStore {
    pub fn new(repo: Arc<EpisodeRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl EpisodeStore for GatewayEpisodeStore {
    async fn list_by_ward(&self, ward_id: &str, limit: usize) -> Result<Vec<Value>, String> {
        let episodes = self.repo.list_by_ward(ward_id, limit)?;
        episodes
            .into_iter()
            .map(|e| serde_json::to_value(e).map_err(|err| err.to_string()))
            .collect()
    }

    async fn insert_episode(
        &self,
        episode: Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<String, String> {
        let mut typed: SessionEpisode =
            serde_json::from_value(episode).map_err(|e| format!("decode SessionEpisode: {e}"))?;
        if embedding.is_some() {
            typed.embedding = embedding;
        }
        self.repo.insert(&typed)
    }

    async fn search_episodes_by_similarity(
        &self,
        agent_id: &str,
        embedding: &[f32],
        threshold: f32,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let scored =
            self.repo
                .search_by_similarity(agent_id, embedding, threshold as f64, limit)?;
        scored
            .into_iter()
            .map(|(ep, score)| {
                Ok(serde_json::json!({
                    "episode": ep,
                    "score": score,
                }))
            })
            .collect()
    }

    async fn fetch_recent_successful_by_ward(
        &self,
        ward_id: &str,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let episodes = self.repo.fetch_recent_successful_by_ward(ward_id, limit)?;
        episodes
            .into_iter()
            .map(|e| serde_json::to_value(e).map_err(|err| err.to_string()))
            .collect()
    }

    async fn episode_stats(&self) -> Result<EpisodeStats, String> {
        let total = self.repo.count()?;
        Ok(EpisodeStats { total })
    }
}
