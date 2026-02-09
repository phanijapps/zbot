// ============================================================================
// GATEWAY MEMORY FACT STORE
// Implements MemoryFactStore trait using MemoryRepository + EmbeddingClient
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use agent_runtime::llm::embedding::{content_hash, EmbeddingClient};
use zero_core::MemoryFactStore;

use crate::memory_repository::{MemoryFact, MemoryRepository};

/// Database-backed implementation of `MemoryFactStore`.
///
/// Wraps `MemoryRepository` for SQLite persistence and an optional
/// `EmbeddingClient` for generating embeddings for hybrid search.
pub struct GatewayMemoryFactStore {
    memory_repo: Arc<MemoryRepository>,
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
}

impl GatewayMemoryFactStore {
    /// Create a new store with the given repository and optional embedding client.
    pub fn new(
        memory_repo: Arc<MemoryRepository>,
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
    ) -> Self {
        Self {
            memory_repo,
            embedding_client,
        }
    }

    /// Embed a text and optionally cache it. Returns None if no client is configured.
    async fn embed_text(&self, text: &str) -> Option<Vec<f32>> {
        let client = self.embedding_client.as_ref()?;
        let hash = content_hash(text);
        let model = client.model_name().to_string();

        // Check cache first
        if let Ok(Some(cached)) = self.memory_repo.get_cached_embedding(&hash, &model) {
            return Some(cached);
        }

        // Generate embedding
        match client.embed(&[text]).await {
            Ok(mut embeddings) if !embeddings.is_empty() => {
                let emb = embeddings.remove(0);
                // Cache for future use (fire-and-forget)
                let _ = self.memory_repo.cache_embedding(&hash, &model, &emb);
                Some(emb)
            }
            Ok(_) => None,
            Err(e) => {
                tracing::warn!("Failed to embed text: {}", e);
                None
            }
        }
    }
}

#[async_trait]
impl MemoryFactStore for GatewayMemoryFactStore {
    async fn save_fact(
        &self,
        agent_id: &str,
        category: &str,
        key: &str,
        content: &str,
        confidence: f64,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        // Generate embedding for the content
        let embedding = self.embed_text(content).await;

        let now = chrono::Utc::now().to_rfc3339();
        let fact = MemoryFact {
            id: format!("fact-{}", uuid::Uuid::new_v4()),
            session_id: session_id.map(String::from),
            agent_id: agent_id.to_string(),
            scope: "agent".to_string(),
            category: category.to_string(),
            key: key.to_string(),
            content: content.to_string(),
            confidence,
            mention_count: 1,
            source_summary: None,
            embedding,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
        };

        self.memory_repo.upsert_memory_fact(&fact)?;

        Ok(json!({
            "success": true,
            "action": "save_fact",
            "key": key,
            "category": category,
            "confidence": confidence,
            "message": format!("Fact saved: [{}] {}", category, content),
        }))
    }

    async fn recall_facts(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        // Generate embedding for the query
        let query_embedding = self.embed_text(query).await;

        let results = self.memory_repo.search_memory_facts_hybrid(
            query,
            query_embedding.as_deref(),
            agent_id,
            limit,
            0.7, // vector weight
            0.3, // bm25 weight
        )?;

        let items: Vec<Value> = results
            .iter()
            .map(|sf| {
                json!({
                    "key": sf.fact.key,
                    "category": sf.fact.category,
                    "content": sf.fact.content,
                    "confidence": sf.fact.confidence,
                    "score": sf.score,
                    "source": "memory_db",
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results": items,
            "count": items.len(),
            "source": "memory_db",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseManager;
    use tempfile::TempDir;

    fn create_test_store() -> GatewayMemoryFactStore {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let _ = temp_dir.keep();
        let db = Arc::new(DatabaseManager::new(path).unwrap());
        let repo = Arc::new(MemoryRepository::new(db));
        GatewayMemoryFactStore::new(repo, None) // No embedding client in tests
    }

    #[tokio::test]
    async fn test_save_and_recall_fact() {
        let store = create_test_store();

        let result = store
            .save_fact("agent-1", "preference", "lang.main", "Prefers Rust", 0.9, None)
            .await
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["key"], "lang.main");

        let recall = store.recall_facts("agent-1", "Rust", 5).await.unwrap();
        assert!(recall["count"].as_u64().unwrap() >= 1);

        let first = &recall["results"][0];
        assert_eq!(first["key"], "lang.main");
        assert_eq!(first["content"], "Prefers Rust");
    }

    #[tokio::test]
    async fn test_save_fact_upsert() {
        let store = create_test_store();

        store
            .save_fact("agent-1", "preference", "editor", "VS Code", 0.7, None)
            .await
            .unwrap();

        // Save again with same key — should upsert
        store
            .save_fact("agent-1", "preference", "editor", "Neovim", 0.9, None)
            .await
            .unwrap();

        let recall = store.recall_facts("agent-1", "editor", 10).await.unwrap();
        assert_eq!(recall["count"].as_u64().unwrap(), 1);
        assert_eq!(recall["results"][0]["content"], "Neovim");
    }

    #[tokio::test]
    async fn test_recall_empty() {
        let store = create_test_store();
        let recall = store.recall_facts("agent-1", "nonexistent", 5).await.unwrap();
        assert_eq!(recall["count"].as_u64().unwrap(), 0);
    }
}
