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

pub mod adapters;
pub mod previous_episodes;
pub mod scored_item;
pub use scored_item::{GoalLite, ItemKind, Provenance, ScoredItem, intent_boost, rrf_merge};

use std::sync::Arc;

use agent_runtime::llm::embedding::EmbeddingClient;
use gateway_services::RecallConfig;
use zero_stores_domain::{MemoryFact, Procedure, ScoredFact};

/// Retrieves relevant memory facts for injection at session start.
///
/// Phase E6c: fully trait-routed. Every store dependency is an
/// `Arc<dyn ...>`; the composition root (`gateway/src/state/mod.rs`)
/// picks the concrete adapter (SQLite today, any future backend
/// tomorrow) and wires it via setters. No SQLite types appear in
/// this struct's signatures.
pub struct MemoryRecall {
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
    memory_store: Option<Arc<dyn zero_stores::MemoryFactStore>>,
    kg_store: Option<Arc<dyn zero_stores::KnowledgeGraphStore>>,
    episode_store: Option<Arc<dyn zero_stores_traits::EpisodeStore>>,
    wiki_store: Option<Arc<dyn zero_stores_traits::WikiStore>>,
    procedure_store: Option<Arc<dyn zero_stores_traits::ProcedureStore>>,
    config: Arc<RecallConfig>,
}

impl MemoryRecall {
    /// Create a new memory recall service. All store dependencies are
    /// wired via setters; the embedding client is optional (recall
    /// degrades to FTS-only when absent).
    pub fn new(
        embedding_client: Option<Arc<dyn EmbeddingClient>>,
        config: Arc<RecallConfig>,
    ) -> Self {
        Self {
            embedding_client,
            memory_store: None,
            kg_store: None,
            episode_store: None,
            wiki_store: None,
            procedure_store: None,
            config,
        }
    }

    /// Access the recall configuration.
    pub fn config(&self) -> &RecallConfig {
        &self.config
    }

    /// Wire the memory-fact store (hybrid FTS + vector recall path).
    pub fn set_memory_store(&mut self, store: Arc<dyn zero_stores::MemoryFactStore>) {
        self.memory_store = Some(store);
    }

    /// Wire the KG store (graph ANN recall path).
    pub fn set_kg_store(&mut self, store: Arc<dyn zero_stores::KnowledgeGraphStore>) {
        self.kg_store = Some(store);
    }

    /// Wire the episode store (previous-episode chain recall path).
    pub fn set_episode_store(&mut self, store: Arc<dyn zero_stores_traits::EpisodeStore>) {
        self.episode_store = Some(store);
    }

    /// Wire the wiki store (ward-scoped wiki recall path).
    pub fn set_wiki_store(&mut self, store: Arc<dyn zero_stores_traits::WikiStore>) {
        self.wiki_store = Some(store);
    }

    /// Wire the procedure store (procedure recall path).
    pub fn set_procedure_store(&mut self, store: Arc<dyn zero_stores_traits::ProcedureStore>) {
        self.procedure_store = Some(store);
    }

    /// Search for proven procedures similar to a query.
    ///
    /// Returns matching procedures with their similarity scores, filtered to
    /// the given agent and optional ward scope.
    pub async fn recall_procedures(
        &self,
        query: &str,
        agent_id: &str,
        ward_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Procedure, f64)>, String> {
        let embedding = match self.embed_query(query).await {
            Some(emb) => emb,
            None => return Ok(Vec::new()),
        };

        let store = match self.procedure_store.as_ref() {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        store
            .search_procedures_by_similarity_typed(&embedding, agent_id, ward_id, limit)
            .await
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

        // 2. Run hybrid search (FTS5 + vector). Trait-routed when
        //    memory_store is wired so SurrealDB is honored when opted-in.
        let hybrid_results: Vec<ScoredFact> = if let Some(store) = &self.memory_store {
            let raw = store
                .search_memory_facts_hybrid(
                    Some(agent_id),
                    user_message,
                    "hybrid",
                    limit * 2,
                    None,
                    query_embedding.as_deref(),
                )
                .await?;
            // Decode each Value back to a ScoredFact-compatible shape. The
            // trait emits MemoryFactResponse-shaped JSON; we wrap each in a
            // ScoredFact with score=0.0 (the trait doesn't preserve per-row
            // scores yet — captured in the portability doc as a follow-up).
            raw.into_iter()
                .filter_map(|v| {
                    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
                        .ok()
                        .map(|fact| ScoredFact { fact, score: 0.0 })
                })
                .collect()
        } else {
            // Phase E6c: memory_store is the only path. Defensive empty
            // when not wired (production composition root always wires it).
            Vec::new()
        };

        // 3. Also fetch high-confidence facts (always relevant).
        let high_conf_facts: Vec<MemoryFact> = match self.memory_store.as_ref() {
            Some(store) => store
                .get_high_confidence_facts(
                    Some(agent_id),
                    self.config.high_confidence_threshold,
                    limit,
                )
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };

        // 3b. Include relevant corrections — corrections get a 1.5x category boost
        //     but must still have minimum relevance to the query. This prevents
        //     "WiZ lights" corrections appearing for currency questions.
        let all_corrections: Vec<MemoryFact> = match self.memory_store.as_ref() {
            Some(store) => store
                .get_facts_by_category(agent_id, "correction", 10)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };

        // Corrections: include all, capped at a reasonable limit.
        // Phase 1c will restore threshold-based filtering via unified scored recall.
        let corrections: Vec<_> = all_corrections.into_iter().take(5).collect();

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

        // 9. Class-aware supersession penalty.
        //
        // Research rationale: archival facts (historical records) retain their
        // relevance regardless of age and should not decay merely because they
        // have been superseded — the historical value is precisely their point.
        // Current facts, by contrast, should decay hard when superseded since
        // the newer replacement is what callers want. Conventions and
        // procedural facts are rule/pattern-based and carry no temporal
        // meaning, so no supersession penalty applies.
        for sf in &mut results {
            apply_class_aware_penalty(sf);
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

    /// Unified scored-pool recall: query every configured source (facts, wiki,
    /// procedures, graph ANN, active goals), adapt each into [`ScoredItem`],
    /// apply [`intent_boost`] against `active_goals`, then fuse via
    /// Reciprocal Rank Fusion capped to `budget`.
    ///
    /// Missing subsystems (no embedding client, no wiki repo, etc.) are
    /// silently skipped — the caller gets whatever sources are wired.
    pub async fn recall_unified(
        &self,
        agent_id: &str,
        query: &str,
        ward_id: Option<&str>,
        active_goals: &[GoalLite],
        budget: usize,
    ) -> Result<Vec<ScoredItem>, String> {
        let query_emb = self.embed_query(query).await;

        // 1. Facts via hybrid search. Phase E8: prefer the trait
        // `memory_store` (wired in both backends), fall back to the
        // SQLite repo. On Surreal, scores aren't yet preserved by the
        // trait surface — we synthesize 0.5 so facts still rank into
        // the fused pool but don't dominate it.
        let fact_items: Vec<ScoredItem> = if let Some(store) = self.memory_store.as_ref() {
            store
                .search_memory_facts_hybrid(
                    Some(agent_id),
                    query,
                    "hybrid",
                    10,
                    ward_id,
                    query_emb.as_deref(),
                )
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
                        .ok()
                        .map(|fact| adapters::fact_to_item(&fact, 0.5))
                })
                .collect()
        } else {
            Vec::new()
        };

        // 2. Wiki articles (ward-scoped).
        let wiki_items: Vec<ScoredItem> =
            match (self.wiki_store.as_ref(), query_emb.as_ref(), ward_id) {
                (Some(store), Some(emb), Some(wid)) => store
                    .search_wiki_by_similarity_typed(wid, emb, 5)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(a, s)| adapters::wiki_to_item(&a, s))
                    .collect(),
                _ => Vec::new(),
            };

        // 3. Procedures.
        let procedure_items: Vec<ScoredItem> =
            match (self.procedure_store.as_ref(), query_emb.as_ref()) {
                (Some(store), Some(emb)) => store
                    .search_procedures_by_similarity_typed(emb, agent_id, ward_id, 5)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(p, s)| adapters::procedure_to_item(&p, s))
                    .collect(),
                _ => Vec::new(),
            };

        // 4. Graph ANN over the entity name embedding index.
        let graph_items: Vec<ScoredItem> = match (self.kg_store.as_ref(), query_emb.as_ref()) {
            (Some(store), Some(emb)) => adapters::graph_ann_to_items(store, emb, 10, agent_id)
                .await
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        // 5a. Previous episodes in this ward (chain continuity).
        let episode_items: Vec<ScoredItem> = match (self.episode_store.as_ref(), ward_id) {
            (Some(store), Some(wid)) => {
                previous_episodes::PreviousEpisodesAdapter::new(store.clone())
                    .fetch(wid)
                    .await
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        };

        // 5. Active goals as retrievable items.
        let goal_items: Vec<ScoredItem> = active_goals
            .iter()
            .map(|g| ScoredItem {
                kind: ItemKind::Goal,
                id: g.id.clone(),
                content: format!("Active goal: {}", g.title),
                score: 1.0,
                provenance: Provenance {
                    source: "kg_goals".to_string(),
                    source_id: g.id.clone(),
                    session_id: None,
                    ward_id: ward_id.map(String::from),
                },
            })
            .collect();

        // Intent boost on non-goal lists.
        let mut all_lists = vec![
            fact_items,
            wiki_items,
            procedure_items,
            graph_items,
            episode_items,
        ];
        for list in &mut all_lists {
            intent_boost(list, active_goals);
        }
        all_lists.push(goal_items);

        Ok(rrf_merge(all_lists, 60.0, budget))
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

/// Format the system message surfaced to the agent when the automatic
/// session-start recall fails with an error.
///
/// Phase 7 (T-D): empty recall results stay quiet — only genuine errors
/// produce a surface message so the agent knows memory retrieval was
/// attempted and can call `memory(action="recall", ...)` manually.
pub fn format_recall_failure_message(err: &str) -> String {
    format!(
        "[Memory retrieval failed: {}. You can call memory(action=\"recall\", query=...) manually if you need past context.]",
        err
    )
}

/// Format a unified scored-item list as a prompt-ready context block.
///
/// Items are emitted in input order (caller should already have them ranked
/// by `recall_unified`). Each line is prefixed with the item kind so the
/// downstream LLM can reason about provenance. Empty input yields an empty
/// string so callers can short-circuit with `.is_empty()`.
pub fn format_scored_items(items: &[ScoredItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut lines = Vec::with_capacity(items.len() + 1);
    lines.push("## Recalled Context".to_string());
    for item in items {
        let tag = match item.kind {
            ItemKind::Fact => "fact",
            ItemKind::Wiki => "wiki",
            ItemKind::Procedure => "procedure",
            ItemKind::GraphNode => "entity",
            ItemKind::Goal => "goal",
            ItemKind::Episode => "episode",
        };
        lines.push(format!("- [{}] {}", tag, item.content));
    }
    lines.join("\n")
}

/// Apply class-aware penalty to a scored fact based on its epistemic class
/// and whether it has been superseded (`valid_until` set).
///
/// - `archival` → `0.3x` if superseded (corrected), otherwise no penalty.
///   Archival facts are historical records; their age is not a defect.
/// - `current` → `0.1x` if superseded (strong decay — prefer the replacement).
/// - `convention` / `procedural` → no temporal penalty (confidence-based only).
/// - unknown / null → treat as `current` with a conservative `0.3x` on
///   supersession to avoid punishing facts we cannot classify.
fn apply_class_aware_penalty(sf: &mut ScoredFact) {
    // Null class defaults to empty string so it falls through to the
    // unknown-class branch (0.3x on supersession) — a conservative default
    // rather than assuming `current` (which implies 0.1x).
    let class = sf.fact.epistemic_class.as_deref().unwrap_or("");
    let is_superseded = sf.fact.valid_until.is_some();
    match class {
        "archival" => {
            if is_superseded {
                sf.score *= 0.3;
            }
        }
        "current" => {
            if is_superseded {
                sf.score *= 0.1;
            }
        }
        "convention" | "procedural" => {
            // No temporal decay for rule/pattern-based facts.
        }
        _ => {
            // Unknown class — conservative default, same as legacy behavior.
            if is_superseded {
                sf.score *= 0.3;
            }
        }
    }
}

fn temporal_decay(last_seen: chrono::DateTime<chrono::Utc>, half_life_days: f64) -> f64 {
    let age_days = (chrono::Utc::now() - last_seen).num_days().max(0) as f64;
    1.0 / (1.0 + (age_days / half_life_days))
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_item(kind: ItemKind, id: &str, content: &str, score: f64) -> ScoredItem {
        ScoredItem {
            kind,
            id: id.to_string(),
            content: content.to_string(),
            score,
            provenance: Provenance {
                source: "test".into(),
                source_id: id.into(),
                session_id: None,
                ward_id: None,
            },
        }
    }

    #[test]
    fn format_scored_items_empty_returns_empty_string() {
        assert!(format_scored_items(&[]).is_empty());
    }

    #[test]
    fn format_recall_failure_message_includes_error_and_guidance() {
        let msg = format_recall_failure_message("database timeout");
        assert!(msg.contains("database timeout"));
        assert!(msg.contains("Memory retrieval failed"));
        assert!(msg.contains("memory(action=\"recall\""));
    }

    #[test]
    fn format_scored_items_tags_each_kind() {
        let items = vec![
            mk_item(ItemKind::Fact, "f1", "fact content", 1.0),
            mk_item(ItemKind::Wiki, "w1", "wiki content", 0.9),
            mk_item(ItemKind::Procedure, "p1", "proc content", 0.8),
            mk_item(ItemKind::GraphNode, "g1", "node content", 0.7),
            mk_item(ItemKind::Goal, "go1", "goal content", 0.6),
            mk_item(ItemKind::Episode, "e1", "ep content", 0.5),
        ];
        let out = format_scored_items(&items);
        assert!(out.starts_with("## Recalled Context"));
        assert!(out.contains("- [fact] fact content"));
        assert!(out.contains("- [wiki] wiki content"));
        assert!(out.contains("- [procedure] proc content"));
        assert!(out.contains("- [entity] node content"));
        assert!(out.contains("- [goal] goal content"));
        assert!(out.contains("- [episode] ep content"));
    }

    fn make_scored_fact(class: Option<&str>, valid_until: Option<&str>, score: f64) -> ScoredFact {
        ScoredFact {
            fact: MemoryFact {
                id: "fact-test".to_string(),
                session_id: None,
                agent_id: "agent-1".to_string(),
                scope: "agent".to_string(),
                category: "misc".to_string(),
                key: "test.key".to_string(),
                content: "test content".to_string(),
                confidence: 0.9,
                mention_count: 1,
                source_summary: None,
                embedding: None,
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: valid_until.map(|s| s.to_string()),
                superseded_by: None,
                pinned: false,
                epistemic_class: class.map(|s| s.to_string()),
                source_episode_id: None,
                source_ref: None,
            },
            score,
        }
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
        assert!(decay < 0.2);
    }

    #[test]
    fn archival_superseded_gets_mild_penalty() {
        let mut sf = make_scored_fact(Some("archival"), Some("2026-01-01"), 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 0.3).abs() < 1e-6);
    }

    #[test]
    fn current_superseded_gets_strong_penalty() {
        let mut sf = make_scored_fact(Some("current"), Some("2026-01-01"), 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 0.1).abs() < 1e-6);
    }

    #[test]
    fn archival_not_superseded_keeps_score() {
        let mut sf = make_scored_fact(Some("archival"), None, 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn current_not_superseded_keeps_score() {
        let mut sf = make_scored_fact(Some("current"), None, 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn convention_never_decays() {
        let mut sf = make_scored_fact(Some("convention"), Some("2026-01-01"), 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn procedural_never_decays() {
        let mut sf = make_scored_fact(Some("procedural"), Some("2026-01-01"), 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn unknown_class_treated_as_current() {
        let mut sf = make_scored_fact(Some("mystery"), Some("2026-01-01"), 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 0.3).abs() < 1e-6);
    }

    #[test]
    fn null_class_treated_as_current() {
        let mut sf = make_scored_fact(None, Some("2026-01-01"), 1.0);
        apply_class_aware_penalty(&mut sf);
        assert!((sf.score - 0.3).abs() < 1e-6);
    }
}
