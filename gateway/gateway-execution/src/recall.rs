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
#[cfg(test)]
use gateway_database::MemoryFact;
use gateway_database::{
    EpisodeRepository, MemoryRepository, RecallLogRepository, ScoredFact, SessionEpisode,
};
use gateway_services::RecallConfig;
use knowledge_graph::{EntityWithConnections, GraphService, GraphTraversal};

/// Result of a memory recall operation, optionally including graph context.
#[derive(Debug, Clone)]
pub struct RecallResult {
    /// Scored facts from memory search
    pub facts: Vec<ScoredFact>,
    /// Relevant past episodes from episodic recall
    pub episodes: Vec<SessionEpisode>,
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
    traversal: Option<Arc<dyn GraphTraversal>>,
    config: Arc<RecallConfig>,
    episode_repo: Option<Arc<EpisodeRepository>>,
    recall_log: Option<Arc<RecallLogRepository>>,
}

impl MemoryRecall {
    /// Create a new memory recall service.
    pub fn new(
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        memory_repo: Arc<MemoryRepository>,
        config: Arc<RecallConfig>,
    ) -> Self {
        Self {
            embedding_client,
            memory_repo,
            graph_service: None,
            traversal: None,
            config,
            episode_repo: None,
            recall_log: None,
        }
    }

    /// Access the recall configuration.
    pub fn config(&self) -> &RecallConfig {
        &self.config
    }

    /// Create a new memory recall service with graph support.
    pub fn with_graph(
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        memory_repo: Arc<MemoryRepository>,
        graph_service: Arc<GraphService>,
        config: Arc<RecallConfig>,
    ) -> Self {
        Self {
            embedding_client,
            memory_repo,
            graph_service: Some(graph_service),
            traversal: None,
            config,
            episode_repo: None,
            recall_log: None,
        }
    }

    /// Set the episode repository for episodic recall.
    pub fn set_episode_repo(&mut self, repo: Arc<EpisodeRepository>) {
        self.episode_repo = Some(repo);
    }

    /// Set the graph service for enriched recall.
    pub fn set_graph_service(&mut self, service: Arc<GraphService>) {
        self.graph_service = Some(service);
    }

    /// Set the graph traversal engine for graph-driven expansion.
    pub fn set_traversal(&mut self, t: Arc<dyn GraphTraversal>) {
        self.traversal = Some(t);
    }

    /// Set the recall log repository for tracking recalled facts per session.
    pub fn set_recall_log(&mut self, repo: Arc<RecallLogRepository>) {
        self.recall_log = Some(repo);
    }

    /// Recall relevant facts for a given agent and user message.
    ///
    /// Returns scored facts sorted by relevance (highest first), with
    /// category weights and optional ward affinity boost applied.
    pub async fn recall(
        &self,
        agent_id: &str,
        user_message: &str,
        limit: usize,
        ward_id: Option<&str>,
    ) -> Result<Vec<ScoredFact>, String> {
        // 1. Embed the user message for vector search
        let query_embedding = self.embed_query(user_message).await;

        // 2. Run hybrid search (FTS5 + vector) using config weights
        let hybrid_results = self.memory_repo.search_memory_facts_hybrid(
            user_message,
            query_embedding.as_deref(),
            agent_id,
            limit * 2, // Fetch more than needed, we'll merge and trim
            self.config.vector_weight,
            self.config.bm25_weight,
            None, // ward_id — no ward filtering in recall service (for now)
        )?;

        // 3. Also fetch high-confidence facts (always relevant)
        let high_conf_facts = self
            .memory_repo
            .get_high_confidence_facts(agent_id, self.config.high_confidence_threshold, limit)
            .unwrap_or_default();

        // 3b. Include relevant corrections — corrections get a 1.5x category boost
        //     but must still have minimum relevance to the query. This prevents
        //     "WiZ lights" corrections appearing for currency questions.
        let all_corrections = self
            .memory_repo
            .get_facts_by_category(agent_id, "correction", 10)
            .unwrap_or_default();

        // Filter corrections by minimum cosine similarity to query (if embedding available)
        let corrections: Vec<_> = if let Some(ref qe) = query_embedding {
            all_corrections
                .into_iter()
                .filter(|fact| {
                    if let Some(ref fact_emb) = fact.embedding {
                        let sim = cosine_similarity(fact_emb, qe);
                        sim >= 0.15 // Low threshold — broadly relevant corrections pass
                    } else {
                        true // No embedding = include (benefit of the doubt)
                    }
                })
                .take(5)
                .collect()
        } else {
            // No query embedding = include top 5 (fallback)
            all_corrections.into_iter().take(5).collect()
        };

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

        // Add corrections with pre-boost (category weight 1.5x applied later too)
        for fact in corrections {
            if seen_keys.insert(fact.key.clone()) {
                results.push(ScoredFact {
                    score: fact.confidence * 1.5,
                    fact,
                });
            }
        }

        // 5. Apply category weights from config
        for sf in &mut results {
            let category_weight = self.config.category_weight(&sf.fact.category);
            sf.score *= category_weight;
        }

        // 6. Apply ward affinity boost — facts whose key starts with the
        //    ward prefix get a relevance boost (ward_id filtering in the DB
        //    is not yet available — Task 21 will add ward_id to MemoryFact).
        if let Some(current_ward) = ward_id {
            if !current_ward.is_empty() && current_ward != "scratch" {
                let ward_prefix = format!("{}/", current_ward);
                for sf in &mut results {
                    if sf.fact.key.starts_with(&ward_prefix) || sf.fact.category == "ward" {
                        sf.score *= self.config.ward_affinity_boost;
                    }
                }
            }
        }

        // 7. Apply temporal decay — older facts score lower based on per-category half-lives
        if self.config.temporal_decay.enabled {
            for sf in &mut results {
                // Skill/agent indices don't decay (re-indexed each session)
                if sf.fact.category == "skill" || sf.fact.category == "agent" {
                    continue;
                }
                let half_life = self
                    .config
                    .temporal_decay
                    .half_life_days
                    .get(&sf.fact.category)
                    .copied()
                    .unwrap_or(30.0);
                let last_seen = chrono::DateTime::parse_from_rfc3339(&sf.fact.updated_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                let decay = temporal_decay(last_seen, half_life);
                let mention_boost = 1.0 + (sf.fact.mention_count as f64).max(1.0).log2();
                sf.score *= decay * mention_boost;
            }
        }

        // 8. Penalize contradicted facts
        for sf in &mut results {
            if sf.fact.contradicted_by.is_some() {
                sf.score *= self.config.contradiction_penalty;
            }
        }

        // 9. Penalize superseded facts — prefer current over outdated
        for sf in &mut results {
            if sf.fact.valid_until.is_some() {
                sf.score *= 0.3;
            }
        }

        // Sort by score descending and take top-K
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Recall relevant facts enriched with knowledge graph context and episodes.
    ///
    /// This method extends the basic recall with related entities from the
    /// knowledge graph and relevant past episodes, providing richer context
    /// for the agent. Output is capped at `config.max_recall_tokens`.
    ///
    /// `session_id` is used for two purposes:
    /// 1. Log which facts were recalled (for future predictive recall)
    /// 2. Find similar past sessions to boost correlated facts
    pub async fn recall_with_graph(
        &self,
        agent_id: &str,
        user_message: &str,
        limit: usize,
        ward_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<RecallResult, String> {
        // 1. Standard fact search with priority scoring
        let mut facts = self.recall(agent_id, user_message, limit, ward_id).await?;

        // Build seen_keys from recalled facts so graph expansion doesn't duplicate
        let mut seen_keys: std::collections::HashSet<String> =
            facts.iter().map(|sf| sf.fact.key.clone()).collect();

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

        // 4. Episodic recall — search past episodes by vector similarity
        let episodes = if let Some(ref episode_repo) = self.episode_repo {
            let query_embedding = self.embed_query(user_message).await;
            if let Some(ref emb) = query_embedding {
                match episode_repo.search_by_similarity(
                    agent_id,
                    emb,
                    0.5,
                    self.config.max_episodes,
                ) {
                    Ok(scored_episodes) => {
                        scored_episodes.into_iter().map(|(ep, _score)| ep).collect()
                    }
                    Err(e) => {
                        tracing::warn!("Episode recall failed: {}", e);
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // 5. Graph expansion: discover related facts via entity connections
        let mut graph_facts: Vec<ScoredFact> = Vec::new();
        if self.config.graph_traversal.enabled {
            if let Some(ref traversal) = self.traversal {
                // Extract entity names from top recalled facts
                let entity_names: Vec<String> = facts
                    .iter()
                    .take(5)
                    .flat_map(|sf| extract_potential_entity_names(&sf.fact.key, &sf.fact.content))
                    .collect();

                let name_refs: Vec<&str> = entity_names.iter().map(|s| s.as_str()).collect();

                if !name_refs.is_empty() {
                    match traversal
                        .connected_entities(&name_refs, self.config.graph_traversal.max_hops, 20)
                        .await
                    {
                        Ok(nodes) => {
                            // For each discovered entity, search for related facts
                            let mut seen_graph_keys = std::collections::HashSet::new();
                            for node in &nodes {
                                if let Ok(related) = self.memory_repo.search_memory_facts_fts(
                                    &node.entity_name,
                                    agent_id,
                                    2,
                                    ward_id,
                                ) {
                                    for sf in related {
                                        if seen_graph_keys.insert(sf.fact.key.clone())
                                            && !seen_keys.contains(&sf.fact.key)
                                        {
                                            graph_facts.push(ScoredFact {
                                                score: sf.score * node.relevance,
                                                fact: sf.fact,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => tracing::warn!("Graph traversal failed: {}", e),
                    }
                }
            }
        }

        // Merge graph-discovered facts into results (capped)
        graph_facts.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        graph_facts.truncate(self.config.graph_traversal.max_graph_facts);
        for sf in graph_facts {
            if seen_keys.insert(sf.fact.key.clone()) {
                facts.push(sf);
            }
        }

        // 6. Predictive recall — boost facts that were recalled in similar past successful sessions
        if self.config.predictive_recall.enabled {
            if let (Some(ref episode_repo), Some(ref recall_log)) =
                (&self.episode_repo, &self.recall_log)
            {
                let query_embedding = self.embed_query(user_message).await;
                if let Some(ref emb) = query_embedding {
                    match episode_repo.search_by_similarity(
                        agent_id,
                        emb,
                        0.5,
                        self.config.predictive_recall.max_episodes_to_check,
                    ) {
                        Ok(similar) => {
                            let success_ids: Vec<&str> = similar
                                .iter()
                                .filter(|(ep, _)| ep.outcome == "success")
                                .map(|(ep, _)| ep.session_id.as_str())
                                .collect();

                            if !success_ids.is_empty() {
                                if let Ok(key_counts) =
                                    recall_log.get_keys_for_sessions(&success_ids)
                                {
                                    let min_count =
                                        self.config.predictive_recall.min_similar_successes;
                                    let boost = self.config.predictive_recall.predictive_boost;
                                    let mut boosted = 0usize;
                                    for sf in &mut facts {
                                        if let Some(&count) = key_counts.get(&sf.fact.key) {
                                            if count >= min_count {
                                                sf.score *= boost;
                                                boosted += 1;
                                            }
                                        }
                                    }
                                    if boosted > 0 {
                                        tracing::debug!(
                                            boosted_count = boosted,
                                            success_sessions = success_ids.len(),
                                            "Predictive recall boosted facts"
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => tracing::warn!("Predictive recall failed: {}", e),
                    }
                }
            }
        }

        // Re-sort after predictive boost and truncate to limit
        facts.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        facts.truncate(limit);

        // 7. Log recalled fact keys for this session (enables future predictive recall)
        if let (Some(sid), Some(ref recall_log)) = (session_id, &self.recall_log) {
            for sf in &facts {
                let _ = recall_log.log_recall(sid, &sf.fact.key);
            }
        }

        // 8. Format combined result with token budget
        let formatted = format_prioritized_recall(
            &facts,
            &episodes,
            &graph_context,
            self.config.max_recall_tokens,
        );

        Ok(RecallResult {
            facts,
            episodes,
            graph_context,
            formatted,
        })
    }

    /// Recall memory context specifically for intent analysis.
    ///
    /// Returns a formatted `<memory_context>` string containing corrections,
    /// proven strategies, domain knowledge, and similar past sessions. This is
    /// injected into the intent analysis LLM call so the planner benefits from
    /// prior experience.
    ///
    /// Uses `"__global__"` as agent_id for cross-agent recall since intent
    /// analysis runs at root level before any specific agent is selected.
    pub async fn recall_for_intent(
        &self,
        user_message: &str,
        limit: usize,
    ) -> Result<String, String> {
        const GLOBAL_AGENT: &str = "__global__";

        // 1. Run hybrid search for top facts relevant to the user message
        let facts = self.recall(GLOBAL_AGENT, user_message, limit, None).await?;

        if facts.is_empty() {
            tracing::debug!("recall_for_intent: no facts found");
            return Ok(String::new());
        }

        // 2. Group facts by category
        let mut corrections: Vec<&ScoredFact> = Vec::new();
        let mut strategies: Vec<&ScoredFact> = Vec::new();
        let mut domain: Vec<&ScoredFact> = Vec::new();

        for sf in &facts {
            match sf.fact.category.as_str() {
                "correction" | "instruction" => corrections.push(sf),
                "strategy" | "user" => strategies.push(sf),
                _ => domain.push(sf),
            }
        }

        let mut output = String::from("<memory_context>\n");

        // Corrections — strongest language, must follow
        if !corrections.is_empty() {
            output.push_str("## Corrections (MUST follow)\n");
            for sf in &corrections {
                output.push_str(&format!("- {}\n", sf.fact.content));
            }
            output.push('\n');
        }

        // Proven strategies
        if !strategies.is_empty() {
            output.push_str("## Proven Strategies\n");
            for sf in &strategies {
                output.push_str(&format!("- {}\n", sf.fact.content));
            }
            output.push('\n');
        }

        // Domain knowledge
        if !domain.is_empty() {
            output.push_str("## Domain Knowledge\n");
            for sf in &domain {
                output.push_str(&format!("- {}\n", sf.fact.content));
            }
            output.push('\n');
        }

        // 3. Optionally query graph for related entities (1-hop neighbors)
        if let Some(ref graph_service) = self.graph_service {
            let entity_names = extract_entity_names_from_facts(&facts);
            if !entity_names.is_empty() {
                match get_graph_context_for_entities(graph_service, GLOBAL_AGENT, &entity_names)
                    .await
                {
                    Ok(Some(ctx)) if !ctx.entities.is_empty() => {
                        output.push_str("## Related Entities\n");
                        for ec in &ctx.entities {
                            let mut line = format!(
                                "- {} ({})",
                                ec.entity.name,
                                ec.entity.entity_type.as_str()
                            );
                            if !ec.outgoing.is_empty() {
                                let rels: Vec<String> = ec
                                    .outgoing
                                    .iter()
                                    .map(|(rel, target)| {
                                        format!(
                                            "{} -> {}",
                                            rel.relationship_type.as_str(),
                                            target.name
                                        )
                                    })
                                    .collect();
                                line.push_str(&format!(": {}", rels.join(", ")));
                            }
                            output.push_str(&format!("{}\n", line));
                        }
                        output.push('\n');
                    }
                    Ok(_) => {}
                    Err(e) => tracing::debug!("recall_for_intent graph lookup skipped: {}", e),
                }
            }
        }

        // 4. Optionally search episodes for similar past sessions
        if let Some(ref episode_repo) = self.episode_repo {
            let query_embedding = self.embed_query(user_message).await;
            if let Some(ref emb) = query_embedding {
                match episode_repo.search_by_similarity(GLOBAL_AGENT, emb, 0.5, 3) {
                    Ok(scored_episodes) if !scored_episodes.is_empty() => {
                        output.push_str("## Similar Past Sessions\n");
                        for (ep, _score) in &scored_episodes {
                            let strategy = ep.strategy_used.as_deref().unwrap_or("unknown");
                            output.push_str(&format!(
                                "- \"{}\" -> {}, strategy: {}\n",
                                ep.task_summary, ep.outcome, strategy
                            ));
                        }
                        output.push('\n');
                    }
                    Ok(_) => {}
                    Err(e) => tracing::debug!("recall_for_intent episode search skipped: {}", e),
                }
            }
        }

        output.push_str("</memory_context>");

        tracing::info!(
            facts_count = facts.len(),
            corrections = corrections.len(),
            strategies = strategies.len(),
            domain_facts = domain.len(),
            "recall_for_intent complete"
        );

        Ok(output)
    }

    /// Recall facts for a delegated subagent — corrections, skills, domain context.
    /// Output formatted as <primed_context> block for system prompt injection.
    pub async fn recall_for_delegation(
        &self,
        agent_id: &str,
        task: &str,
        ward_id: Option<&str>,
        limit: usize,
    ) -> Result<String, String> {
        // 1. Recall facts relevant to the task, scoped to ward
        let facts = self.recall(agent_id, task, limit, ward_id).await?;

        // Also try global recall for cross-agent corrections
        let global_facts = self
            .recall("__global__", task, 3, ward_id)
            .await
            .unwrap_or_default();

        if facts.is_empty() && global_facts.is_empty() {
            return Ok(String::new());
        }

        let mut sections: Vec<String> = Vec::new();

        // Corrections first (highest priority) — from both agent-specific and global
        let mut correction_lines: Vec<String> = Vec::new();
        for f in facts.iter().chain(global_facts.iter()) {
            if f.fact.category == "correction" || f.fact.category == "instruction" {
                let line = format!("- {}", f.fact.content);
                if !correction_lines.contains(&line) {
                    correction_lines.push(line);
                }
            }
        }
        if !correction_lines.is_empty() {
            sections.push(format!(
                "## Corrections (MUST follow)\n{}",
                correction_lines.join("\n")
            ));
        }

        // Strategy/pattern knowledge
        let strategies: Vec<String> = facts
            .iter()
            .filter(|f| f.fact.category == "strategy" || f.fact.category == "pattern")
            .map(|f| format!("- {}", f.fact.content))
            .collect();
        if !strategies.is_empty() {
            sections.push(format!("## Strategies\n{}", strategies.join("\n")));
        }

        // Domain knowledge
        let domain: Vec<String> = facts
            .iter()
            .filter(|f| {
                !["correction", "instruction", "strategy", "pattern"]
                    .contains(&f.fact.category.as_str())
            })
            .map(|f| format!("- [{}] {}", f.fact.category, f.fact.content))
            .collect();
        if !domain.is_empty() {
            sections.push(format!("## Context\n{}", domain.join("\n")));
        }

        if sections.is_empty() {
            return Ok(String::new());
        }

        Ok(format!(
            "<primed_context>\n{}\n</primed_context>",
            sections.join("\n\n")
        ))
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
pub fn format_combined_recall(
    facts: &[ScoredFact],
    graph_context: &Option<GraphContext>,
) -> String {
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
                    let rels: Vec<String> = entity_conn
                        .outgoing
                        .iter()
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

/// Format recalled facts, episodes, and graph context into structured sections
/// with a token budget. Trims lowest-scored items first to stay within budget.
///
/// Estimates ~4 characters per token for budget enforcement.
pub fn format_prioritized_recall(
    facts: &[ScoredFact],
    episodes: &[SessionEpisode],
    graph_context: &Option<GraphContext>,
    max_tokens: usize,
) -> String {
    if facts.is_empty() && episodes.is_empty() && graph_context.is_none() {
        return String::new();
    }

    let max_chars = max_tokens * 4;

    // Separate corrections (hard rules) from preferences and domain context
    let mut rules: Vec<&ScoredFact> = Vec::new();
    let mut preferences: Vec<&ScoredFact> = Vec::new();
    let mut domain: Vec<&ScoredFact> = Vec::new();

    for sf in facts {
        match sf.fact.category.as_str() {
            "correction" => rules.push(sf),
            "user" | "instruction" | "strategy" => preferences.push(sf),
            _ => domain.push(sf),
        }
    }

    let mut output = String::new();

    // Section 1: Rules (corrections) — FIRST, strongest language
    // These come before everything else so the LLM processes them first.
    if !rules.is_empty() {
        output.push_str("## Rules (from past corrections — ALWAYS follow these)\n");
        for sf in &rules {
            let line = format!("- {}\n", sf.fact.content);
            if output.len() + line.len() > max_chars {
                break;
            }
            output.push_str(&line);
        }
        output.push('\n');
    }

    output.push_str("## Recalled Context\n");

    // Section 2: Preferences & Instructions (high-priority but softer than rules)
    if !preferences.is_empty() && output.len() < max_chars {
        output.push_str("### Preferences & Instructions\n");
        for sf in &preferences {
            let line = format!(
                "- [{}] {} ({:.2})\n",
                sf.fact.category, sf.fact.content, sf.fact.confidence
            );
            if output.len() + line.len() > max_chars {
                break;
            }
            output.push_str(&line);
        }
    }

    // Section 3: Failed episode warnings (highest priority after rules)
    let failed_episodes: Vec<&SessionEpisode> = episodes
        .iter()
        .filter(|ep| ep.outcome == "failed" || ep.outcome == "crashed")
        .collect();
    if !failed_episodes.is_empty() && output.len() < max_chars {
        output.push_str("### Warnings (past failures — avoid these approaches)\n");
        for ep in &failed_episodes {
            let strategy = ep.strategy_used.as_deref().unwrap_or("unknown approach");
            let learnings = ep.key_learnings.as_deref().unwrap_or("");
            let line = format!(
                "- FAILED: {} — strategy: {}. {}\n",
                ep.task_summary,
                strategy,
                if learnings.is_empty() {
                    "Avoid this approach.".to_string()
                } else {
                    learnings.to_string()
                }
            );
            if output.len() + line.len() > max_chars {
                break;
            }
            output.push_str(&line);
        }
    }

    // Section 4: Successful past experiences
    let successful_episodes: Vec<&SessionEpisode> = episodes
        .iter()
        .filter(|ep| ep.outcome == "success" || ep.outcome == "partial")
        .collect();
    if !successful_episodes.is_empty() && output.len() < max_chars {
        output.push_str("### Past Experiences\n");
        for ep in &successful_episodes {
            let strategy = ep.strategy_used.as_deref().unwrap_or("unknown");
            let tokens = ep.token_cost.unwrap_or(0);
            let date = ep.created_at.split('T').next().unwrap_or(&ep.created_at);
            let line = format!(
                "- {} ({}): {} — {}, {} tokens\n",
                ep.task_summary,
                date,
                ep.outcome.to_uppercase(),
                strategy,
                tokens
            );
            if output.len() + line.len() > max_chars {
                break;
            }
            output.push_str(&line);
        }
    }

    // Section 4: Domain Context (remaining facts)
    if !domain.is_empty() && output.len() < max_chars {
        output.push_str("### Domain Knowledge\n");
        for sf in &domain {
            let line = format!(
                "- [{}] {} ({:.2})\n",
                sf.fact.category, sf.fact.content, sf.fact.confidence
            );
            if output.len() + line.len() > max_chars {
                break;
            }
            output.push_str(&line);
        }
    }

    // Section 5: Graph entities (lowest priority, fills remaining budget)
    if let Some(ref ctx) = graph_context {
        if !ctx.entities.is_empty() && output.len() < max_chars {
            output.push_str("### Related Entities\n");
            for entity_conn in &ctx.entities {
                let mut line = format!(
                    "- **{}** ({})",
                    entity_conn.entity.name,
                    entity_conn.entity.entity_type.as_str()
                );
                if !entity_conn.outgoing.is_empty() {
                    let rels: Vec<String> = entity_conn
                        .outgoing
                        .iter()
                        .map(|(rel, target)| {
                            format!("{} -> {}", rel.relationship_type.as_str(), target.name)
                        })
                        .collect();
                    line.push_str(&format!(": {}", rels.join(", ")));
                }
                line.push('\n');
                if output.len() + line.len() > max_chars {
                    break;
                }
                output.push_str(&line);
            }
        }
    }

    output
}

/// Compute temporal decay for a fact based on its last-seen timestamp.
///
/// Uses the formula `1 / (1 + age/half_life)`, which yields:
/// - 1.0 for a fact seen just now
/// - 0.5 for a fact whose age equals the half-life
/// - Monotonically decreasing toward 0 for very old facts
fn temporal_decay(last_seen: chrono::DateTime<chrono::Utc>, half_life_days: f64) -> f64 {
    let age_days = (chrono::Utc::now() - last_seen).num_days().max(0) as f64;
    1.0 / (1.0 + (age_days / half_life_days))
}

/// Extract potential entity names from a fact key and content.
///
/// Used by graph expansion to find entity names to seed traversal from.
/// Extracts meaningful key segments and capitalized words from content.
fn extract_potential_entity_names(key: &str, content: &str) -> Vec<String> {
    let mut names = Vec::new();
    // Extract from key segments (e.g., "domain.finance.spy" → "spy", "finance")
    for part in key.split('.') {
        if part.len() > 2 && part != "domain" && part != "pattern" && part != "correction" {
            names.push(part.to_string());
        }
    }
    // Extract capitalized words from content as potential entity names
    for word in content.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if clean.len() > 2
            && clean
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
        {
            names.push(clean.to_string());
        }
    }
    names.dedup();
    names
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
        "The", "This", "That", "These", "Those", "What", "Which", "When", "Where", "Why", "How",
        "If", "Then", "And", "But", "Or", "Not", "User", "Project", "System", "Code", "File",
        "Data",
    ];

    names
        .into_iter()
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
        match graph_service
            .get_entity_with_connections(agent_id, name)
            .await
        {
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

/// Convert embedding blob (little-endian bytes) to f32 vector.
#[allow(dead_code)]
fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Cosine similarity between two vectors. Returns 0.0 on length mismatch.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| *x as f64 * *y as f64)
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
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
                    ward_id: "__global__".to_string(),
                    contradicted_by: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
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
                    ward_id: "__global__".to_string(),
                    contradicted_by: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
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
                    ward_id: "__global__".to_string(),
                    contradicted_by: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
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
                    ward_id: "__global__".to_string(),
                    contradicted_by: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
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
        let facts = vec![ScoredFact {
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
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
            },
            score: 0.85,
        }];

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
        let facts = vec![ScoredFact {
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
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
            },
            score: 0.85,
        }];

        let formatted = format_combined_recall(&facts, &None);
        assert!(formatted.contains("## Recalled Memory"));
        assert!(formatted.contains("Prefers Rust"));
        assert!(!formatted.contains("## Related Entities"));
    }

    #[test]
    fn test_format_combined_recall_with_empty_graph() {
        let facts = vec![ScoredFact {
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
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
            },
            score: 0.85,
        }];

        // Empty graph context
        let graph_ctx = Some(GraphContext { entities: vec![] });
        let formatted = format_combined_recall(&facts, &graph_ctx);
        assert!(formatted.contains("## Recalled Memory"));
        assert!(!formatted.contains("## Related Entities"));
    }

    #[test]
    fn test_format_prioritized_recall_empty() {
        let result = format_prioritized_recall(&[], &[], &None, 3000);
        assert!(result.is_empty());
    }

    #[test]
    fn test_format_prioritized_recall_sections() {
        let facts = vec![
            ScoredFact {
                fact: MemoryFact {
                    id: "f-1".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "correction".to_string(),
                    key: "fix.typo".to_string(),
                    content: "Always use kebab-case for file names".to_string(),
                    confidence: 0.95,
                    mention_count: 2,
                    source_summary: None,
                    embedding: None,
                    ward_id: "__global__".to_string(),
                    contradicted_by: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
                },
                score: 1.4,
            },
            ScoredFact {
                fact: MemoryFact {
                    id: "f-2".to_string(),
                    session_id: None,
                    agent_id: "agent-1".to_string(),
                    scope: "agent".to_string(),
                    category: "domain".to_string(),
                    key: "db.info".to_string(),
                    content: "Database uses WAL mode".to_string(),
                    confidence: 0.80,
                    mention_count: 1,
                    source_summary: None,
                    embedding: None,
                    ward_id: "__global__".to_string(),
                    contradicted_by: None,
                    created_at: String::new(),
                    updated_at: String::new(),
                    expires_at: None,
                    valid_from: None,
                    valid_until: None,
                    superseded_by: None,
                    pinned: false,
                },
                score: 0.7,
            },
        ];

        let episodes = vec![SessionEpisode {
            id: "ep-1".to_string(),
            session_id: "sess-1".to_string(),
            agent_id: "agent-1".to_string(),
            ward_id: "__global__".to_string(),
            task_summary: "Fixed database migration".to_string(),
            outcome: "success".to_string(),
            strategy_used: Some("direct".to_string()),
            key_learnings: None,
            token_cost: Some(1200),
            embedding: None,
            created_at: "2026-03-28T10:00:00Z".to_string(),
        }];

        let formatted = format_prioritized_recall(&facts, &episodes, &None, 3000);

        // Rules section comes first with correction content (no category prefix)
        assert!(formatted.contains("## Rules (from past corrections"));
        assert!(formatted.contains("kebab-case"));
        // Corrections should NOT have [correction] prefix — they're rules now
        assert!(!formatted.contains("[correction]"));

        // Recalled Context section
        assert!(formatted.contains("## Recalled Context"));
        assert!(formatted.contains("### Past Experiences"));
        assert!(formatted.contains("Fixed database migration"));
        assert!(formatted.contains("SUCCESS"));
        assert!(formatted.contains("1200 tokens"));
        assert!(formatted.contains("### Domain Knowledge"));
        assert!(formatted.contains("[domain]"));
        assert!(formatted.contains("WAL mode"));
    }

    #[test]
    fn test_format_prioritized_recall_token_budget() {
        // Create a fact with very long content
        let facts = vec![ScoredFact {
            fact: MemoryFact {
                id: "f-1".to_string(),
                session_id: None,
                agent_id: "agent-1".to_string(),
                scope: "agent".to_string(),
                category: "correction".to_string(),
                key: "fix.something".to_string(),
                content: "x".repeat(500),
                confidence: 0.95,
                mention_count: 1,
                source_summary: None,
                embedding: None,
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
            },
            score: 1.0,
        }];

        // Set a very tight token budget (50 tokens = 200 chars)
        let formatted = format_prioritized_recall(&facts, &[], &None, 50);

        // The output should be limited — the 500-char content should NOT fit
        // within a 200-char budget (50 tokens * 4 chars)
        assert!(formatted.len() <= 250); // Some header overhead allowed
    }

    #[test]
    fn test_temporal_decay_fresh() {
        let now = chrono::Utc::now();
        let decay = temporal_decay(now, 30.0);
        assert!((decay - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_temporal_decay_at_half_life() {
        let half_life_ago = chrono::Utc::now() - chrono::Duration::days(30);
        let decay = temporal_decay(half_life_ago, 30.0);
        assert!((decay - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_temporal_decay_old() {
        let old = chrono::Utc::now() - chrono::Duration::days(180);
        let decay = temporal_decay(old, 30.0);
        assert!(decay < 0.2); // Very old = very low
    }

    #[test]
    fn test_extract_potential_entity_names() {
        let names = extract_potential_entity_names(
            "domain.finance.spy",
            "SPY is an ETF tracking the S&P 500",
        );
        // Should extract key segments
        assert!(names.contains(&"finance".to_string()));
        assert!(names.contains(&"spy".to_string()));
        // Should extract capitalized words from content
        assert!(names.contains(&"SPY".to_string()));
        assert!(names.contains(&"ETF".to_string()));
        // Should skip short segments and common key prefixes
        assert!(!names.contains(&"domain".to_string()));
    }
}
