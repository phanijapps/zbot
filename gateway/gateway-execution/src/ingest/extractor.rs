//! Two-pass LLM extractor: pass 1 entities, pass 2 relationships conditioned
//! on the entity list. Concrete LLM-backed impl lands in Tasks 5 and 6.
//! `NoopExtractor` provides a test-friendly no-op for queue-level tests.

use async_trait::async_trait;
use gateway_database::KgEpisode;
use knowledge_graph::GraphStorage;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Processes one episode — runs extraction + writes to graph.
/// Errors propagate to the worker which marks the episode failed.
#[async_trait]
pub trait Extractor: Send + Sync {
    async fn process(
        &self,
        episode: &KgEpisode,
        chunk_text: &str,
        graph: &Arc<GraphStorage>,
    ) -> Result<(), String>;
}

/// Test-only extractor: records each episode id and always succeeds.
pub struct NoopExtractor {
    pub seen: Mutex<Vec<String>>,
}

impl NoopExtractor {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl Default for NoopExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Extractor for NoopExtractor {
    async fn process(
        &self,
        episode: &KgEpisode,
        _chunk_text: &str,
        _graph: &Arc<GraphStorage>,
    ) -> Result<(), String> {
        self.seen.lock().await.push(episode.id.clone());
        Ok(())
    }
}
