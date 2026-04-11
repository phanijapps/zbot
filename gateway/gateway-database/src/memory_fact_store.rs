// ============================================================================
// GATEWAY MEMORY FACT STORE
// Implements MemoryFactStore trait using MemoryRepository + EmbeddingClient
// ============================================================================

use std::collections::HashMap;
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

    /// Mark similar facts as contradicted by the new fact with the given `key`.
    fn mark_contradicted_facts(
        &self,
        similar_facts: Vec<crate::memory_repository::ScoredFact>,
        key: &str,
        category: &str,
    ) {
        for sf in similar_facts {
            if sf.fact.key != key && sf.fact.category == category {
                if let Err(e) = self.memory_repo.mark_contradicted(&sf.fact.id, key) {
                    tracing::warn!(
                        fact_id = %sf.fact.id,
                        contradicted_by = %key,
                        "Failed to mark fact as contradicted: {}",
                        e
                    );
                } else {
                    tracing::info!(
                        fact_id = %sf.fact.id,
                        fact_key = %sf.fact.key,
                        contradicted_by = %key,
                        similarity = %sf.score,
                        "Marked fact as contradicted by newer fact"
                    );
                }
            }
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
            embedding: embedding.clone(),
            ward_id: "__global__".to_string(),
            contradicted_by: None,
            created_at: now.clone(),
            updated_at: now,
            expires_at: None,
            pinned: false,
        };

        self.memory_repo.upsert_memory_fact(&fact)?;

        // Best-effort contradiction detection: if the new fact has an embedding,
        // search for semantically similar facts with a DIFFERENT key but same
        // category and mark them as contradicted.
        if let Some(ref emb) = embedding {
            if let Ok(similar_facts) = self.memory_repo.search_similar_facts(
                emb, agent_id, 0.8, // high threshold to avoid false positives
                5, None, // no ward filtering
            ) {
                self.mark_contradicted_facts(similar_facts, key, category);
            }
        }

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
            0.7,  // vector weight
            0.3,  // bm25 weight
            None, // ward_id — no ward filtering from trait method
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

    async fn recall_facts_prioritized(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Value, String> {
        // Generate embedding for the query
        let query_embedding = self.embed_text(query).await;

        // Fetch more results than needed so we can re-rank
        let mut results = self.memory_repo.search_memory_facts_hybrid(
            query,
            query_embedding.as_deref(),
            agent_id,
            limit * 2,
            0.7,  // vector weight
            0.3,  // bm25 weight
            None, // ward_id — no ward filtering from trait method
        )?;

        // Also fetch high-confidence facts (>= 0.9) — always relevant
        let high_conf_facts = self
            .memory_repo
            .get_high_confidence_facts(agent_id, 0.9, limit)
            .unwrap_or_default();

        // Include relevant corrections — filter by minimum cosine similarity
        // to avoid injecting "WiZ lights" corrections for currency questions.
        let all_corrections = self
            .memory_repo
            .get_facts_by_category(agent_id, "correction", 10)
            .unwrap_or_default();
        let corrections: Vec<_> = if let Some(ref qe) = query_embedding {
            all_corrections
                .into_iter()
                .filter(|fact| {
                    if let Some(ref fact_emb) = fact.embedding {
                        let sim = crate::memory_repository::cosine_similarity(qe, fact_emb);
                        sim >= 0.15
                    } else {
                        true
                    }
                })
                .take(5)
                .collect()
        } else {
            all_corrections.into_iter().take(5).collect()
        };

        // Merge, dedup by key
        let mut seen_keys = std::collections::HashSet::new();
        let mut merged = Vec::new();
        for sf in results.drain(..) {
            if seen_keys.insert(sf.fact.key.clone()) {
                merged.push(sf);
            }
        }
        for fact in high_conf_facts {
            if seen_keys.insert(fact.key.clone()) {
                merged.push(crate::memory_repository::ScoredFact {
                    score: fact.confidence,
                    fact,
                });
            }
        }
        // Add corrections with pre-boost (category weight 1.5x applied later too)
        for fact in corrections {
            if seen_keys.insert(fact.key.clone()) {
                merged.push(crate::memory_repository::ScoredFact {
                    score: fact.confidence * 1.5,
                    fact,
                });
            }
        }

        // Apply category priority weights (same as system-level recall)
        let category_weights: HashMap<&str, f64> = HashMap::from([
            ("correction", 1.5),
            ("strategy", 1.4),
            ("user", 1.3),
            ("instruction", 1.2),
            ("domain", 1.0),
            ("pattern", 0.9),
            ("ward", 0.8),
            ("skill", 0.7),
            ("agent", 0.7),
        ]);

        for sf in &mut merged {
            let weight = category_weights
                .get(sf.fact.category.as_str())
                .copied()
                .unwrap_or(1.0);
            sf.score *= weight;
        }

        // Sort by weighted score descending
        merged.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        merged.truncate(limit);

        // --- Format output with Rules section for corrections ---
        let mut rules: Vec<&crate::memory_repository::ScoredFact> = Vec::new();
        let mut context: Vec<&crate::memory_repository::ScoredFact> = Vec::new();

        for sf in &merged {
            match sf.fact.category.as_str() {
                "correction" => rules.push(sf),
                _ => context.push(sf),
            }
        }

        let mut formatted = String::new();

        // Rules section comes FIRST — imperative language
        if !rules.is_empty() {
            formatted.push_str("## Rules (from past corrections — ALWAYS follow these)\n");
            for sf in &rules {
                formatted.push_str(&format!("- {}\n", sf.fact.content));
            }
            formatted.push('\n');
        }

        // Regular recalled context
        if !context.is_empty() {
            formatted.push_str("## Recalled Context\n");
            for sf in &context {
                formatted.push_str(&format!(
                    "- [{}] {} ({:.2})\n",
                    sf.fact.category, sf.fact.content, sf.fact.confidence
                ));
            }
        }

        // --- Capability gap detection ---
        // Check if any of the returned results include skill/agent categories.
        // If none do, the query is likely outside known capabilities.
        let has_skill_or_agent = merged
            .iter()
            .any(|sf| sf.fact.category == "skill" || sf.fact.category == "agent");

        if !has_skill_or_agent {
            // Fetch top skills and agents by confidence for the gap section
            let top_skills = self
                .memory_repo
                .get_facts_by_category(agent_id, "skill", 3)
                .unwrap_or_default();
            let top_agents = self
                .memory_repo
                .get_facts_by_category(agent_id, "agent", 3)
                .unwrap_or_default();

            // Only show capability gap if there ARE known capabilities to suggest
            if !top_skills.is_empty() || !top_agents.is_empty() {
                formatted.push_str("\n## Capability Gap\n");
                formatted.push_str("No matching skills or agents found for this request.\n");
                formatted.push_str("Closest available capabilities:\n");

                for skill in &top_skills {
                    formatted.push_str(&format!("- skill: {} ({})\n", skill.key, skill.content));
                }
                for agent in &top_agents {
                    formatted.push_str(&format!("- agent: {} ({})\n", agent.key, agent.content));
                }

                formatted.push_str("\nConsider creating a plan to build the missing capability, or inform the user about current limitations.\n");
            }
        }

        let items: Vec<Value> = merged
            .iter()
            .map(|sf| {
                json!({
                    "key": sf.fact.key,
                    "category": sf.fact.category,
                    "content": sf.fact.content,
                    "confidence": sf.fact.confidence,
                    "score": sf.score,
                    "source": "memory_db",
                    "prioritized": true,
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results": items,
            "count": items.len(),
            "source": "memory_db",
            "prioritized": true,
            "formatted": formatted,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseManager;
    use tempfile::TempDir;

    fn create_test_store() -> GatewayMemoryFactStore {
        use gateway_services::VaultPaths;

        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep();
        let db = Arc::new(DatabaseManager::new(paths).unwrap());
        let repo = Arc::new(MemoryRepository::new(db));
        GatewayMemoryFactStore::new(repo, None) // No embedding client in tests
    }

    #[tokio::test]
    async fn test_save_and_recall_fact() {
        let store = create_test_store();

        let result = store
            .save_fact(
                "agent-1",
                "preference",
                "lang.main",
                "Prefers Rust",
                0.9,
                None,
            )
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
        let recall = store
            .recall_facts("agent-1", "nonexistent", 5)
            .await
            .unwrap();
        assert_eq!(recall["count"].as_u64().unwrap(), 0);
    }
}
