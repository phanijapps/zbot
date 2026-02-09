// ============================================================================
// MEMORY FACT STORE TRAIT
// Abstract interface for durable memory fact storage
// ============================================================================

use async_trait::async_trait;
use serde_json::Value;

/// Abstract interface for durable memory fact storage.
///
/// Implementations can wrap a database (SQLite via `MemoryRepository`),
/// a remote API, or an in-memory store for testing.
///
/// This trait lives in `zero-core` so that `agent-tools` (which depends on
/// `zero-core` but not `gateway-database`) can call DB operations via the trait.
#[async_trait]
pub trait MemoryFactStore: Send + Sync {
    /// Save a structured fact to durable memory.
    ///
    /// On conflict (same agent_id + scope + key), updates content and bumps
    /// mention_count. Returns a JSON summary of the operation.
    async fn save_fact(
        &self,
        agent_id: &str,
        category: &str,
        key: &str,
        content: &str,
        confidence: f64,
        session_id: Option<&str>,
    ) -> Result<Value, String>;

    /// Recall facts relevant to a query using hybrid search.
    ///
    /// Combines FTS5 keyword matching and vector cosine similarity
    /// (when embeddings are available). Returns a JSON array of results.
    async fn recall_facts(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String>;
}
