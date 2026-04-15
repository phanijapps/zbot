//! # Ingestion Adapter
//!
//! Bridges [`gateway_database::KgEpisodeRepository`] + [`IngestionQueue`]
//! to [`agent_tools::IngestionAccess`]. Wired into the agent tool registry
//! so the `ingest` tool can enqueue chunks for background extraction.

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use agent_tools::IngestionAccess;
use gateway_database::KgEpisodeRepository;

use crate::ingest::{
    chunker::{chunk_text, ChunkOptions},
    IngestionQueue,
};

/// Adapter that implements [`IngestionAccess`] by chunking text and enqueuing
/// one episode per chunk via [`KgEpisodeRepository::upsert_pending`].
pub struct IngestionAdapter {
    queue: Arc<IngestionQueue>,
    episode_repo: Arc<KgEpisodeRepository>,
}

impl IngestionAdapter {
    pub fn new(queue: Arc<IngestionQueue>, episode_repo: Arc<KgEpisodeRepository>) -> Self {
        Self {
            queue,
            episode_repo,
        }
    }
}

#[async_trait]
impl IngestionAccess for IngestionAdapter {
    async fn enqueue(
        &self,
        source_id: &str,
        source_type: &str,
        text: &str,
        session_id: Option<&str>,
        agent_id: &str,
    ) -> std::result::Result<(String, usize), String> {
        let chunks = chunk_text(text, ChunkOptions::default());
        let mut enqueued = 0usize;
        for chunk in &chunks {
            let source_ref = format!("{}#chunk-{}", source_id, chunk.index);
            let mut hasher = Sha256::new();
            hasher.update(chunk.text.as_bytes());
            let content_hash = format!("{:x}", hasher.finalize());
            let episode_id = self.episode_repo.upsert_pending(
                source_type,
                &source_ref,
                &content_hash,
                session_id,
                agent_id,
            )?;
            self.episode_repo.set_payload(&episode_id, &chunk.text)?;
            enqueued += 1;
        }
        self.queue.notify();
        Ok((source_id.to_string(), enqueued))
    }
}
