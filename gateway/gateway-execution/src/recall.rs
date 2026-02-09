// ============================================================================
// SMART RECALL
// Automatically retrieve relevant facts at session start
// ============================================================================

//! At the start of each session, `MemoryRecall` retrieves relevant facts
//! from the memory system and formats them for injection into the agent's
//! context. This gives the agent automatic access to prior knowledge without
//! needing to explicitly search memory.
//!
//! ## Recall Strategy
//!
//! 1. Embed the user's first message
//! 2. Run hybrid search (vector + FTS5) against memory_facts
//! 3. Also fetch all high-confidence facts (>= 0.9) — always relevant
//! 4. Merge, dedup by key, take top-K
//! 5. Format as a "Recalled Memory" system message

use std::sync::Arc;

use agent_runtime::llm::embedding::EmbeddingClient;
use gateway_database::{MemoryRepository, ScoredFact};
#[cfg(test)]
use gateway_database::MemoryFact;

/// Retrieves relevant memory facts for injection at session start.
pub struct MemoryRecall {
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
    memory_repo: Arc<MemoryRepository>,
}

impl MemoryRecall {
    /// Create a new memory recall service.
    pub fn new(
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        memory_repo: Arc<MemoryRepository>,
    ) -> Self {
        Self {
            embedding_client,
            memory_repo,
        }
    }

    /// Recall relevant facts for a given agent and user message.
    ///
    /// Returns scored facts sorted by relevance (highest first).
    pub async fn recall(
        &self,
        agent_id: &str,
        user_message: &str,
        limit: usize,
    ) -> Result<Vec<ScoredFact>, String> {
        // 1. Embed the user message for vector search
        let query_embedding = self.embed_query(user_message).await;

        // 2. Run hybrid search (FTS5 + vector)
        let hybrid_results = self.memory_repo.search_memory_facts_hybrid(
            user_message,
            query_embedding.as_deref(),
            agent_id,
            limit * 2, // Fetch more than needed, we'll merge and trim
            0.7,       // vector weight
            0.3,       // bm25 weight
        )?;

        // 3. Also fetch high-confidence facts (always relevant)
        let high_conf_facts = self.memory_repo
            .get_high_confidence_facts(agent_id, 0.9, limit)
            .unwrap_or_default();

        // 4. Merge, dedup by key, take top-K
        let mut seen_keys = std::collections::HashSet::new();
        let mut results: Vec<ScoredFact> = Vec::new();

        // Add hybrid results first (already sorted by score)
        for sf in hybrid_results {
            if seen_keys.insert(sf.fact.key.clone()) {
                results.push(sf);
            }
        }

        // Add high-confidence facts (with score = confidence)
        for fact in high_conf_facts {
            if seen_keys.insert(fact.key.clone()) {
                results.push(ScoredFact {
                    score: fact.confidence,
                    fact,
                });
            }
        }

        // Sort by score descending and take top-K
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Embed a query string for vector search.
    async fn embed_query(&self, text: &str) -> Option<Vec<f32>> {
        let client = self.embedding_client.as_ref()?;

        match client.embed(&[text]).await {
            Ok(mut embeddings) if !embeddings.is_empty() => Some(embeddings.remove(0)),
            Ok(_) => None,
            Err(e) => {
                tracing::warn!("Failed to embed query for recall: {}", e);
                None
            }
        }
    }
}

/// Format recalled facts as a system message for injection into context.
pub fn format_recalled_facts(facts: &[ScoredFact]) -> String {
    if facts.is_empty() {
        return String::new();
    }

    let mut lines = Vec::with_capacity(facts.len() + 1);
    lines.push("## Recalled Memory".to_string());

    for sf in facts {
        lines.push(format!(
            "- [{}] {} (confidence: {:.2})",
            sf.fact.category, sf.fact.content, sf.fact.confidence
        ));
    }

    lines.join("\n")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_recalled_facts_empty() {
        let result = format_recalled_facts(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_recalled_facts() {
        let facts = vec![
            ScoredFact {
                fact: MemoryFact {
                    id: "fact-1".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "preference".to_string(),
                    key: "lang.preferred".to_string(),
                    content: "User prefers Rust for backend".to_string(),
                    confidence: 0.95,
                    mention_count: 3,
                    source_summary: None,
                    embedding: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                },
                score: 0.85,
            },
            ScoredFact {
                fact: MemoryFact {
                    id: "fact-2".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "decision".to_string(),
                    key: "db.engine".to_string(),
                    content: "Project uses SQLite for all persistence".to_string(),
                    confidence: 0.90,
                    mention_count: 2,
                    source_summary: None,
                    embedding: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                },
                score: 0.70,
            },
        ];

        let formatted = format_recalled_facts(&facts);
        assert!(formatted.contains("## Recalled Memory"));
        assert!(formatted.contains("[preference]"));
        assert!(formatted.contains("User prefers Rust"));
        assert!(formatted.contains("[decision]"));
        assert!(formatted.contains("SQLite"));
    }
}
