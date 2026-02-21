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
//! 5. (Optional) Enrich with knowledge graph context
//! 6. Format as a "Recalled Memory" system message

use std::sync::Arc;

use agent_runtime::llm::embedding::EmbeddingClient;
use gateway_database::{MemoryRepository, ScoredFact};
use knowledge_graph::{GraphService, EntityWithConnections};
#[cfg(test)]
use gateway_database::MemoryFact;

/// Result of a memory recall operation, optionally including graph context.
#[derive(Debug, Clone)]
pub struct RecallResult {
    /// Scored facts from memory search
    pub facts: Vec<ScoredFact>,
    /// Graph context for entities mentioned in facts (if graph service available)
    pub graph_context: Option<GraphContext>,
    /// Pre-formatted output string
    pub formatted: String,
}

/// Graph context gathered from entities mentioned in facts.
#[derive(Debug, Clone)]
pub struct GraphContext {
    /// Entities with their connections
    pub entities: Vec<EntityWithConnections>,
}

/// Retrieves relevant memory facts for injection at session start.
pub struct MemoryRecall {
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
    memory_repo: Arc<MemoryRepository>,
    graph_service: Option<Arc<GraphService>>,
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
            graph_service: None,
        }
    }

    /// Create a new memory recall service with graph support.
    pub fn with_graph(
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        memory_repo: Arc<MemoryRepository>,
        graph_service: Arc<GraphService>,
    ) -> Self {
        Self {
            embedding_client,
            memory_repo,
            graph_service: Some(graph_service),
        }
    }

    /// Set the graph service for enriched recall.
    pub fn set_graph_service(&mut self, service: Arc<GraphService>) {
        self.graph_service = Some(service);
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

    /// Recall relevant facts enriched with knowledge graph context.
    ///
    /// This method extends the basic recall with related entities from the
    /// knowledge graph, providing richer context for the agent.
    pub async fn recall_with_graph(
        &self,
        agent_id: &str,
        user_message: &str,
        limit: usize,
    ) -> Result<RecallResult, String> {
        // 1. Standard fact search
        let facts = self.recall(agent_id, user_message, limit).await?;

        // 2. Extract potential entity names from facts
        let entity_names = extract_entity_names_from_facts(&facts);

        // 3. Get graph context for those entities (if service available)
        let graph_context = if let Some(ref graph_service) = self.graph_service {
            if !entity_names.is_empty() {
                get_graph_context_for_entities(graph_service, agent_id, &entity_names).await?
            } else {
                None
            }
        } else {
            None
        };

        // 4. Format combined result
        let formatted = format_combined_recall(&facts, &graph_context);

        Ok(RecallResult {
            facts,
            graph_context,
            formatted,
        })
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

/// Format facts with optional graph context as a combined system message.
pub fn format_combined_recall(facts: &[ScoredFact], graph_context: &Option<GraphContext>) -> String {
    if facts.is_empty() && graph_context.is_none() {
        return String::new();
    }

    let mut output = String::new();

    // Facts section
    if !facts.is_empty() {
        output.push_str("## Recalled Memory\n");
        for sf in facts {
            output.push_str(&format!(
                "- [{}] {} (confidence: {:.2})\n",
                sf.fact.category, sf.fact.content, sf.fact.confidence
            ));
        }
    }

    // Graph context section
    if let Some(ref ctx) = graph_context {
        if !ctx.entities.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str("## Related Entities\n");

            for entity_conn in &ctx.entities {
                // Show the entity
                output.push_str(&format!(
                    "- **{}** ({})",
                    entity_conn.entity.name,
                    entity_conn.entity.entity_type.as_str()
                ));

                // Show outgoing relationships
                if !entity_conn.outgoing.is_empty() {
                    let rels: Vec<String> = entity_conn.outgoing.iter()
                        .map(|(rel, target)| {
                            format!("{} → {}", rel.relationship_type.as_str(), target.name)
                        })
                        .collect();
                    output.push_str(&format!(": {}", rels.join(", ")));
                }

                output.push('\n');
            }
        }
    }

    output
}

/// Extract potential entity names from fact content.
///
/// Uses simple heuristics to identify capitalized words and quoted strings
/// that might represent entities in the knowledge graph.
fn extract_entity_names_from_facts(facts: &[ScoredFact]) -> Vec<String> {
    let mut names = std::collections::HashSet::new();

    for sf in facts {
        let content = &sf.fact.content;

        // Extract capitalized words (potential proper nouns)
        for word in content.split_whitespace() {
            // Clean up the word (remove punctuation)
            let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric());

            // Check if it starts with uppercase and has more than one character
            if clean_word.len() > 1 {
                if let Some(first_char) = clean_word.chars().next() {
                    if first_char.is_uppercase() {
                        names.insert(clean_word.to_string());
                    }
                }
            }
        }

        // Extract quoted strings (often represent entity names)
        let mut in_quotes = false;
        let mut current_quote = String::new();
        let mut quote_char = ' ';

        for ch in content.chars() {
            if ch == '"' || ch == '\'' {
                if in_quotes && ch == quote_char {
                    // End of quoted string
                    if current_quote.len() > 1 {
                        names.insert(current_quote.clone());
                    }
                    current_quote.clear();
                    in_quotes = false;
                } else if !in_quotes {
                    // Start of quoted string
                    in_quotes = true;
                    quote_char = ch;
                }
            } else if in_quotes {
                current_quote.push(ch);
            }
        }
    }

    // Filter out common words that are likely not entities
    let common_words = [
        "The", "This", "That", "These", "Those", "What", "Which", "When",
        "Where", "Why", "How", "If", "Then", "And", "But", "Or", "Not",
        "User", "Project", "System", "Code", "File", "Data",
    ];

    names.into_iter()
        .filter(|name| !common_words.contains(&name.as_str()))
        .collect()
}

/// Get graph context for a list of entity names.
async fn get_graph_context_for_entities(
    graph_service: &GraphService,
    agent_id: &str,
    entity_names: &[String],
) -> Result<Option<GraphContext>, String> {
    let mut entities_with_connections = Vec::new();

    for name in entity_names {
        match graph_service.get_entity_with_connections(agent_id, name).await {
            Ok(Some(connections)) => {
                // Only include entities that have connections
                if !connections.outgoing.is_empty() || !connections.incoming.is_empty() {
                    entities_with_connections.push(connections);
                }
            }
            Ok(None) => {
                // Entity not found in graph, skip
            }
            Err(e) => {
                tracing::warn!("Failed to get entity connections for '{}': {}", name, e);
            }
        }
    }

    if entities_with_connections.is_empty() {
        Ok(None)
    } else {
        Ok(Some(GraphContext {
            entities: entities_with_connections,
        }))
    }
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

    #[test]
    fn test_extract_entity_names_from_facts() {
        let facts = vec![
            ScoredFact {
                fact: MemoryFact {
                    id: "fact-1".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "preference".to_string(),
                    key: "tool.preferred".to_string(),
                    content: "Alice prefers VSCode for development".to_string(),
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
                    category: "fact".to_string(),
                    key: "org.info".to_string(),
                    content: "Bob works at Acme Corporation".to_string(),
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

        let names = extract_entity_names_from_facts(&facts);

        // Should extract capitalized words
        assert!(names.contains(&"Alice".to_string()));
        assert!(names.contains(&"VSCode".to_string()));
        assert!(names.contains(&"Bob".to_string()));
        assert!(names.contains(&"Acme".to_string()));
        assert!(names.contains(&"Corporation".to_string()));

        // Should filter out common words
        assert!(!names.contains(&"The".to_string()));
        assert!(!names.contains(&"This".to_string()));
    }

    #[test]
    fn test_extract_entity_names_quoted_strings() {
        let facts = vec![
            ScoredFact {
                fact: MemoryFact {
                    id: "fact-1".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "info".to_string(),
                    key: "tool.name".to_string(),
                    content: "The tool is called 'my_awesome_tool' and it helps".to_string(),
                    confidence: 0.95,
                    mention_count: 1,
                    source_summary: None,
                    embedding: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                },
                score: 0.85,
            },
        ];

        let names = extract_entity_names_from_facts(&facts);

        // Should extract quoted strings
        assert!(names.contains(&"my_awesome_tool".to_string()));
    }

    #[test]
    fn test_format_combined_recall_empty() {
        let result = format_combined_recall(&[], &None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_combined_recall_facts_only() {
        let facts = vec![
            ScoredFact {
                fact: MemoryFact {
                    id: "fact-1".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "preference".to_string(),
                    key: "lang".to_string(),
                    content: "Prefers Rust".to_string(),
                    confidence: 0.95,
                    mention_count: 1,
                    source_summary: None,
                    embedding: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                },
                score: 0.85,
            },
        ];

        let formatted = format_combined_recall(&facts, &None);
        assert!(formatted.contains("## Recalled Memory"));
        assert!(formatted.contains("Prefers Rust"));
        assert!(!formatted.contains("## Related Entities"));
    }

    #[test]
    fn test_format_combined_recall_with_empty_graph() {
        let facts = vec![
            ScoredFact {
                fact: MemoryFact {
                    id: "fact-1".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "preference".to_string(),
                    key: "lang".to_string(),
                    content: "Prefers Rust".to_string(),
                    confidence: 0.95,
                    mention_count: 1,
                    source_summary: None,
                    embedding: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                },
                score: 0.85,
            },
        ];

        // Empty graph context
        let graph_ctx = Some(GraphContext { entities: vec![] });
        let formatted = format_combined_recall(&facts, &graph_ctx);
        assert!(formatted.contains("## Recalled Memory"));
        assert!(!formatted.contains("## Related Entities"));
    }
}
