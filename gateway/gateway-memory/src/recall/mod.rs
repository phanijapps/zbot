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
pub mod mmr;
pub mod previous_episodes;
pub mod query_gate;
pub mod scored_item;
pub use mmr::{mmr_select, MmrInput};
pub use query_gate::{GateResponse, LlmQueryGate, QueryGate, QueryGateLlm, RetrievalDecision};
pub use scored_item::{intent_boost, rrf_merge, GoalLite, ItemKind, Provenance, ScoredItem};

use std::sync::Arc;

use crate::{MmrConfig, RecallConfig};
use agent_runtime::llm::embedding::EmbeddingClient;
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
    /// Belief store for Phase B-4 recall integration. Wired only when
    /// the Belief Network is enabled — when `None`, `recall_unified`
    /// stays byte-for-byte identical to pre-B-4 behavior (no extra
    /// fetch, no extra log).
    belief_store: Option<Arc<dyn zero_stores_traits::BeliefStore>>,
    /// Self-RAG retrieval gate. When `None`, recall behaves identically to
    /// pre-gate behavior (raw user message → hybrid search).
    query_gate: Option<Arc<QueryGate>>,
    /// MMR diversity reranking. When `None` or `enabled = false`,
    /// `recall_unified` is byte-for-byte identical to pre-MMR behavior.
    mmr_config: Option<MmrConfig>,
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
            belief_store: None,
            query_gate: None,
            mmr_config: None,
            config,
        }
    }

    /// Wire the belief store so `recall_unified` surfaces beliefs
    /// alongside facts (Phase B-4). Caller is expected to attach this
    /// only when `beliefNetwork.enabled = true`; when unset, beliefs
    /// stay out of the recall pool entirely.
    pub fn set_belief_store(&mut self, store: Arc<dyn zero_stores_traits::BeliefStore>) {
        self.belief_store = Some(store);
    }

    /// Wire the Self-RAG retrieval gate. When set, `recall()` consults the
    /// gate before running hybrid search. The always-inject corrections path
    /// (bootstrap) is unaffected.
    pub fn set_query_gate(&mut self, gate: Arc<QueryGate>) {
        self.query_gate = Some(gate);
    }

    /// Wire MMR diversity reranking. When set with `enabled = true`,
    /// `recall_unified` over-fetches `candidate_pool` items from RRF, then
    /// reranks via MMR before truncating to the caller's budget. When
    /// unset (or `enabled = false`), recall is byte-for-byte identical to
    /// pre-MMR behavior.
    pub fn set_mmr_config(&mut self, cfg: MmrConfig) {
        self.mmr_config = Some(cfg);
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
        // 1. Self-RAG retrieval gate (opt-in via `memory.queryGate.enabled`).
        //    When absent, the gate defaults to Direct(user_message) — keeping
        //    behavior identical to pre-gate recall. The gate scopes ONLY the
        //    hybrid search call below; high-confidence facts, in-recall
        //    corrections, and the bootstrap always-inject path are unaffected.
        let decision = match &self.query_gate {
            Some(gate) => gate.reformulate(user_message).await,
            None => RetrievalDecision::Direct(user_message.to_string()),
        };

        // 2. Run hybrid search according to the gate decision.
        let hybrid_results = self.hybrid_for_decision(agent_id, &decision, limit).await?;

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

        // Drop superseded facts before sorting — no point ranking items we'll discard.
        results.retain(|sf| sf.fact.superseded_by.is_none());
        // Sort by score descending, drop items below min_score, and take top-K
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.retain(|sf| sf.score >= self.config.min_score);
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
        // `memory_store` (wired by AppState), fall back to the
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
                    None, // as_of — default "now" recall
                )
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);
                    // See note on `zero_stores_sqlite::MemoryFact` above — we
                    // decode into the domain type to avoid a dep cycle.
                    serde_json::from_value::<MemoryFact>(v)
                        .ok()
                        .filter(|fact| fact.superseded_by.is_none())
                        .map(|fact| adapters::fact_to_item(&fact, score))
                })
                .filter(|item| item.score >= self.config.min_score)
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

        // 5b. Beliefs (Phase B-4). Only when both a belief_store is
        // wired AND a query embedding was produced — beliefs are
        // semantic-only (no FTS fallback). When the Belief Network is
        // disabled or the embedder is unavailable, this list stays
        // empty and `recall_unified` behaves byte-for-byte identically
        // to pre-B-4. The partition_id used here is `agent_id`,
        // mirroring how the synthesizer writes beliefs (one belief
        // partition per agent).
        let belief_items: Vec<ScoredItem> = match (self.belief_store.as_ref(), query_emb.as_ref()) {
            (Some(store), Some(emb)) => store
                .search_beliefs(agent_id, emb, 10)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|sb| {
                    let weight = self.config.category_weight("belief");
                    let mut item = adapters::belief_to_item(&sb.belief, sb.score);
                    item.score *= weight;
                    item
                })
                .collect(),
            _ => Vec::new(),
        };

        // Intent boost on non-goal lists.
        let mut all_lists = vec![
            fact_items,
            wiki_items,
            procedure_items,
            graph_items,
            episode_items,
            belief_items,
        ];
        for list in &mut all_lists {
            intent_boost(list, active_goals);
        }
        all_lists.push(goal_items);

        // Decide fusion budget: when MMR is enabled, over-fetch from RRF so
        // MMR has a wider candidate pool to diversify over. When disabled,
        // pass `budget` straight through so behavior is byte-for-byte
        // identical to pre-MMR.
        let (fusion_budget, run_mmr) = match self.mmr_config.as_ref() {
            Some(cfg) if cfg.enabled => (cfg.candidate_pool.max(budget), true),
            _ => (budget, false),
        };

        let fused = rrf_merge(all_lists, 60.0, fusion_budget);

        if !run_mmr {
            return Ok(fused);
        }

        let lambda = self.mmr_config.as_ref().map(|c| c.lambda).unwrap_or(0.6);
        let reranked = self.mmr_rerank(fused, lambda, budget).await;
        Ok(reranked)
    }

    /// Resolve a per-item embedding for MMR. Returns `None` when the
    /// candidate's kind has no embedding source or the lookup fails.
    ///
    /// - `Fact` → `memory_store.get_fact_embedding(id)` (single SQLite hop).
    /// - `Belief` / `Wiki` / `Procedure` / `GraphNode` → embed the rendered
    ///   `content` via the embedding client. The belief's stored embedding
    ///   bytes are not carried through `ScoredItem`, and re-embedding the
    ///   rendered content tracks the same semantic neighborhood closely
    ///   enough for diversity scoring at this scale.
    /// - `Goal` / `Episode` → `None` (diversity penalty contributes 0).
    async fn fetch_item_embedding(&self, item: &ScoredItem) -> Option<Vec<f32>> {
        match item.kind {
            ItemKind::Fact => {
                let store = self.memory_store.as_ref()?;
                store.get_fact_embedding(&item.id).await.ok().flatten()
            }
            ItemKind::Belief | ItemKind::Wiki | ItemKind::Procedure | ItemKind::GraphNode => {
                let client = self.embedding_client.as_ref()?;
                match client.embed(&[item.content.as_str()]).await {
                    Ok(mut v) if !v.is_empty() => Some(v.remove(0)),
                    _ => None,
                }
            }
            ItemKind::Goal | ItemKind::Episode => None,
        }
    }

    /// Apply MMR reranking to `candidates`, returning at most `budget`
    /// items in selection order. Embeddings are fetched per item via
    /// [`Self::fetch_item_embedding`]; candidates without embeddings keep
    /// their relevance score but contribute zero diversity penalty.
    async fn mmr_rerank(
        &self,
        candidates: Vec<ScoredItem>,
        lambda: f64,
        budget: usize,
    ) -> Vec<ScoredItem> {
        if candidates.is_empty() || budget == 0 {
            return Vec::new();
        }

        // Serial embedding fetch. `candidate_pool` defaults to 30, each
        // fact lookup is a single SQLite hop, and content-embed calls hit
        // the cached embedding client. At this scale, naive serial is
        // sub-ms per item — batching is not yet warranted.
        let mut embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(candidates.len());
        for item in &candidates {
            embeddings.push(self.fetch_item_embedding(item).await);
        }

        let inputs: Vec<MmrInput<'_>> = candidates
            .iter()
            .zip(embeddings.iter())
            .map(|(item, emb)| MmrInput {
                item,
                embedding: emb.as_deref(),
            })
            .collect();

        let selected_idx = mmr_select(inputs, lambda, budget);
        selected_idx
            .into_iter()
            .map(|i| candidates[i].clone())
            .collect()
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

    /// Run one hybrid search call against the trait-routed memory store.
    /// Returns an empty vector when no memory store is wired (defensive).
    async fn run_hybrid_search(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ScoredFact>, String> {
        let store = match &self.memory_store {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        let query_embedding = self.embed_query(query).await;
        let raw = store
            .search_memory_facts_hybrid(
                Some(agent_id),
                query,
                "hybrid",
                limit * 2,
                None,
                query_embedding.as_deref(),
                None, // as_of — default "now" recall; point-in-time is opt-in
            )
            .await?;
        Ok(raw
            .into_iter()
            .filter_map(|v| {
                serde_json::from_value::<MemoryFact>(v)
                    .ok()
                    .map(|fact| ScoredFact { fact, score: 0.0 })
            })
            .collect())
    }

    /// Apply the gate decision: run zero, one, or several hybrid searches and
    /// dedup-merge the results by fact key. `Skip` returns an empty vector
    /// (high-confidence facts + in-recall corrections are added by the caller).
    async fn hybrid_for_decision(
        &self,
        agent_id: &str,
        decision: &RetrievalDecision,
        limit: usize,
    ) -> Result<Vec<ScoredFact>, String> {
        match decision {
            RetrievalDecision::Skip => Ok(Vec::new()),
            RetrievalDecision::Direct(q) => self.run_hybrid_search(agent_id, q, limit).await,
            RetrievalDecision::Split(subqueries) => {
                let mut merged: Vec<ScoredFact> = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for sq in subqueries {
                    let sub = self.run_hybrid_search(agent_id, sq, limit).await?;
                    for sf in sub {
                        // Dedup by fact id (more reliable than `key`, which can
                        // collide across scopes); preserve first occurrence.
                        if seen.insert(sf.fact.id.clone()) {
                            merged.push(sf);
                        }
                    }
                }
                Ok(merged)
            }
        }
    }
}

/// Apply class-aware penalty to a scored fact based on its epistemic class
/// and whether it has been superseded (`superseded_by` set).
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
    let is_superseded = sf.fact.superseded_by.is_some();
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

    fn make_scored_fact(
        class: Option<&str>,
        superseded_by: Option<&str>,
        score: f64,
    ) -> ScoredFact {
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
                valid_until: None,
                superseded_by: superseded_by.map(|s| s.to_string()),
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

    #[test]
    fn bitemporal_bounded_fact_not_penalised_when_not_superseded() {
        // valid_until set (fact's truth interval ended in the world)
        // but superseded_by is None (no newer fact replaces it).
        // This is bi-temporal history — should NOT be penalized.
        let mut sf = make_scored_fact(Some("current"), None, 1.0);
        sf.fact.valid_until = Some("2026-03-01".to_string());
        apply_class_aware_penalty(&mut sf);
        assert!(
            (sf.score - 1.0).abs() < 1e-6,
            "bi-temporal history (valid_until set, superseded_by None) should not be penalized"
        );
    }

    #[test]
    fn recall_facts_retains_only_items_above_min_score() {
        use std::sync::Arc;
        let config = Arc::new(RecallConfig::default()); // default min_score = 0.3

        // Simulate what recall_facts does: sort → retain → truncate
        let mut results = vec![
            mk_item(ItemKind::Fact, "high", "high relevance", 0.9),
            mk_item(ItemKind::Fact, "mid", "borderline", 0.3),
            mk_item(ItemKind::Fact, "low", "chess procedures", 0.1),
        ];
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.retain(|sf| sf.score >= config.min_score);
        results.truncate(10);

        assert_eq!(results.len(), 2, "low-score item should be filtered");
        assert!(results.iter().any(|i| i.id == "high"));
        assert!(results.iter().any(|i| i.id == "mid"));
        assert!(
            !results.iter().any(|i| i.id == "low"),
            "chess procedures should be suppressed"
        );
    }

    #[test]
    fn recall_facts_excludes_superseded() {
        // Sanity test: retain logic drops items whose underlying fact has
        // superseded_by set. Verifies the filter intent directly without the
        // full recall pipeline.
        use zero_stores_sqlite::MemoryFact;

        let mk_fact = |id: &str, superseded: Option<&str>| MemoryFact {
            id: id.to_string(),
            session_id: None,
            agent_id: "agent".to_string(),
            scope: "agent".to_string(),
            category: "schema".to_string(),
            key: format!("schema.{id}"),
            content: "content".to_string(),
            confidence: 0.9,
            mention_count: 1,
            source_summary: None,
            embedding: None,
            ward_id: "__global__".to_string(),
            contradicted_by: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
            valid_from: None,
            valid_until: None,
            superseded_by: superseded.map(String::from),
            pinned: false,
            epistemic_class: Some("current".to_string()),
            source_episode_id: None,
            source_ref: None,
        };

        let facts = vec![
            mk_fact("a", None),
            mk_fact("b", Some("a")),
            mk_fact("c", None),
        ];

        let kept: Vec<_> = facts
            .into_iter()
            .filter(|f| f.superseded_by.is_none())
            .collect();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().any(|f| f.id == "a"));
        assert!(kept.iter().any(|f| f.id == "c"));
        assert!(!kept.iter().any(|f| f.id == "b"));
    }

    // ========================================================================
    // Test H — Query gate integration: Skip decision still surfaces
    // in-recall corrections + high-confidence facts; only the hybrid-search
    // portion is suppressed.
    // ========================================================================
    use crate::recall::query_gate::{GateResponse, QueryGateLlm};
    use async_trait::async_trait;
    use gateway_services::VaultPaths;
    use std::sync::Mutex;
    use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};
    use zero_stores_sqlite::{GatewayMemoryFactStore, KnowledgeDatabase, MemoryRepository};

    struct FixedDecisionLlm {
        decision: Mutex<&'static str>,
    }

    #[async_trait]
    impl QueryGateLlm for FixedDecisionLlm {
        async fn reformulate(&self, _raw: &str) -> Result<GateResponse, String> {
            let d = *self.decision.lock().unwrap();
            Ok(GateResponse {
                decision: d.to_string(),
                query: None,
                subqueries: None,
            })
        }
    }

    fn make_skip_gate() -> Arc<QueryGate> {
        let llm: Arc<dyn QueryGateLlm> = Arc::new(FixedDecisionLlm {
            decision: Mutex::new("skip"),
        });
        let cfg = crate::QueryGateConfig {
            enabled: true,
            ..Default::default()
        };
        Arc::new(QueryGate::new(llm, cfg))
    }

    #[tokio::test]
    async fn corrections_still_inject_when_gate_returns_skip() {
        // Setup: build a real SQLite-backed memory store, seed a correction
        // and a non-correction fact, attach a gate that always returns Skip.
        let tmp = tempfile::tempdir().unwrap();
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(db, vec_index));
        let memory_store: Arc<dyn zero_stores::MemoryFactStore> =
            Arc::new(GatewayMemoryFactStore::new(memory_repo, None));

        let agent_id = "agent-test-h";

        // Correction fact — should always come through (in-recall path).
        memory_store
            .save_fact(
                agent_id,
                "correction",
                "corr.hard_rule",
                "Always validate user input before processing",
                0.95,
                None,
                None,
            )
            .await
            .unwrap();

        // Domain (non-correction) fact — would only surface via hybrid search.
        memory_store
            .save_fact(
                agent_id,
                "domain",
                "domain.misc_topic",
                "Some unrelated domain knowledge about geography",
                0.8,
                None,
                None,
            )
            .await
            .unwrap();

        // Build recall with the skip gate attached.
        let config = Arc::new(RecallConfig::default());
        let mut recall = MemoryRecall::new(None, config);
        recall.set_memory_store(memory_store.clone());
        recall.set_query_gate(make_skip_gate());

        // Use a query that would never match the domain fact under hybrid
        // search anyway — the gate's Skip means we don't even try.
        let results = recall.recall(agent_id, "thanks!", 10, None).await.unwrap();

        // The correction must be present even under Skip — it comes from the
        // in-recall corrections path (step 3b), not from hybrid search.
        let correction_present = results
            .iter()
            .any(|sf| sf.fact.key == "corr.hard_rule" && sf.fact.category == "correction");
        assert!(
            correction_present,
            "in-recall corrections must survive gate Skip — got keys: {:?}",
            results.iter().map(|sf| &sf.fact.key).collect::<Vec<_>>()
        );

        // High-confidence path (confidence >= 0.9): the correction qualifies
        // there too, so we don't assert absence of the domain fact (it has
        // confidence 0.8 — below the high-conf threshold of 0.9 and won't
        // come through that path).
        // What we DO want to check: hybrid search did not run, so the only
        // way the domain fact would appear is via high-conf (it can't) or
        // via the corrections category (it's not a correction). So it must
        // be absent.
        let domain_present = results.iter().any(|sf| sf.fact.key == "domain.misc_topic");
        assert!(
            !domain_present,
            "non-correction fact below high-conf threshold must NOT appear under Skip (gate suppressed hybrid search)"
        );
    }

    // ========================================================================
    // Phase B-4 — Belief recall integration tests
    // ========================================================================

    use agent_runtime::llm::embedding::{EmbeddingClient, EmbeddingError};
    use zero_stores_domain::{Belief, ScoredBelief};
    use zero_stores_traits::BeliefStore;

    /// Test embedder that returns a fixed vector — only used for B-4
    /// recall tests where the magnitudes don't matter, only presence.
    struct TestEmbed;
    #[async_trait]
    impl EmbeddingClient for TestEmbed {
        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            Ok(texts.iter().map(|_| vec![1.0_f32, 0.0, 0.0]).collect())
        }
        fn dimensions(&self) -> usize {
            3
        }
        fn model_name(&self) -> String {
            "test".to_string()
        }
    }

    /// In-memory belief store that returns a fixed list from
    /// `search_beliefs` and records the call count. Other methods are
    /// no-ops or empty — only what `recall_unified` calls matters.
    struct StubBeliefStore {
        canned: Vec<ScoredBelief>,
        search_calls: std::sync::atomic::AtomicUsize,
    }

    impl StubBeliefStore {
        fn new(canned: Vec<ScoredBelief>) -> Self {
            Self {
                canned,
                search_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
        fn search_calls(&self) -> usize {
            self.search_calls.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl BeliefStore for StubBeliefStore {
        async fn get_belief(
            &self,
            _: &str,
            _: &str,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<Option<Belief>, String> {
            Ok(None)
        }
        async fn list_beliefs(&self, _: &str, _: usize) -> Result<Vec<Belief>, String> {
            Ok(vec![])
        }
        async fn upsert_belief(&self, _: &Belief) -> Result<(), String> {
            Ok(())
        }
        async fn supersede_belief(
            &self,
            _: &str,
            _: &str,
            _: chrono::DateTime<chrono::Utc>,
        ) -> Result<(), String> {
            Ok(())
        }
        async fn mark_stale(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        async fn retract_belief(
            &self,
            _: &str,
            _: chrono::DateTime<chrono::Utc>,
        ) -> Result<(), String> {
            Ok(())
        }
        async fn beliefs_referencing_fact(&self, _: &str) -> Result<Vec<String>, String> {
            Ok(vec![])
        }
        async fn get_belief_by_id(&self, _: &str) -> Result<Option<Belief>, String> {
            Ok(None)
        }
        async fn list_stale(&self, _: &str, _: usize) -> Result<Vec<Belief>, String> {
            Ok(vec![])
        }
        async fn clear_stale(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        async fn search_beliefs(
            &self,
            _: &str,
            _: &[f32],
            _: usize,
        ) -> Result<Vec<ScoredBelief>, String> {
            self.search_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(self.canned.clone())
        }
    }

    fn sample_belief(id: &str, subject: &str, content: &str) -> Belief {
        let now = chrono::Utc::now();
        Belief {
            id: id.to_string(),
            partition_id: "ag".to_string(),
            subject: subject.to_string(),
            content: content.to_string(),
            confidence: 0.85,
            valid_from: Some(now),
            valid_until: None,
            source_fact_ids: vec![],
            synthesizer_version: 1,
            reasoning: None,
            created_at: now,
            updated_at: now,
            superseded_by: None,
            stale: false,
            embedding: None,
        }
    }

    /// When a belief_store is wired AND a query embedding is produced,
    /// `recall_unified` calls `search_beliefs` exactly once and the
    /// returned items carry `ItemKind::Belief`.
    #[tokio::test]
    async fn recall_unified_fetches_beliefs_when_store_wired() {
        let beliefs = vec![
            ScoredBelief {
                belief: sample_belief("b1", "user.location", "User lives in Mason, OH"),
                score: 0.9,
            },
            ScoredBelief {
                belief: sample_belief("b2", "user.diet", "User is vegetarian"),
                score: 0.7,
            },
        ];
        let stub: Arc<StubBeliefStore> = Arc::new(StubBeliefStore::new(beliefs));
        let stub_dyn: Arc<dyn BeliefStore> = stub.clone();

        let config = Arc::new(RecallConfig::default());
        let embed: Arc<dyn EmbeddingClient> = Arc::new(TestEmbed);
        let mut recall = MemoryRecall::new(Some(embed), config);
        recall.set_belief_store(stub_dyn);

        let out = recall
            .recall_unified("ag", "where do I live?", None, &[], 20)
            .await
            .unwrap();

        assert_eq!(stub.search_calls(), 1, "search_beliefs called exactly once");
        let belief_count = out
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Belief))
            .count();
        assert_eq!(belief_count, 2, "both stub beliefs surface");
    }

    /// When no belief_store is wired, `recall_unified` produces zero
    /// belief items — pre-B-4 behavior preserved byte-for-byte.
    #[tokio::test]
    async fn recall_unified_does_not_fetch_beliefs_when_store_absent() {
        let config = Arc::new(RecallConfig::default());
        let embed: Arc<dyn EmbeddingClient> = Arc::new(TestEmbed);
        let recall = MemoryRecall::new(Some(embed), config);

        let out = recall
            .recall_unified("ag", "where do I live?", None, &[], 20)
            .await
            .unwrap();
        assert!(
            !out.iter().any(|i| matches!(i.kind, ItemKind::Belief)),
            "no belief_store wired ⇒ no belief items"
        );
    }

    /// Direct assertion on the category-weight map: belief = 1.5,
    /// matching corrections and sitting below schema (1.6) — the
    /// design doc Q4 decision.
    #[test]
    fn belief_category_weight_is_one_point_five() {
        let config = RecallConfig::default();
        assert!((config.category_weight("belief") - 1.5).abs() < f64::EPSILON);
    }

    /// Schemas (1.6) outrank beliefs (1.5) when both carry the same
    /// raw score — proves the weight ordering survives rescore.
    #[test]
    fn schemas_outrank_beliefs_at_equal_raw_score() {
        let config = RecallConfig::default();
        let raw = 0.5_f64;
        let schema_weighted = raw * config.category_weight("schema");
        let belief_weighted = raw * config.category_weight("belief");
        assert!(
            schema_weighted > belief_weighted,
            "schema ({schema_weighted}) must beat belief ({belief_weighted}) at equal raw score"
        );
    }

    // ========================================================================
    // MMR — Maximal Marginal Relevance integration tests
    //
    // These tests exercise the full recall_unified pipeline with MMR enabled
    // vs disabled, end-to-end through a real SQLite-backed memory store.
    // ========================================================================

    use crate::MmrConfig;

    /// 384-dim is what the SQLite vec0 table expects. We synthesize
    /// orthogonal directions by setting exactly one of the first three
    /// components to 1.0. Cosine similarity is then `1.0` between
    /// same-direction items and `0.0` between different directions.
    const EMBED_DIM: usize = 384;

    /// Directional embedder used by MMR integration tests. Routes input
    /// text to one of three orthogonal directions based on keywords —
    /// lets tests set up controlled near-duplicate vs. diverse scenarios.
    struct DirectionalEmbed;

    #[async_trait]
    impl EmbeddingClient for DirectionalEmbed {
        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            Ok(texts.iter().map(|t| direction_for_text(t)).collect())
        }
        fn dimensions(&self) -> usize {
            EMBED_DIM
        }
        fn model_name(&self) -> String {
            "directional".to_string()
        }
    }

    fn direction_for_text(text: &str) -> Vec<f32> {
        let t = text.to_lowercase();
        let mut v = vec![0.0_f32; EMBED_DIM];
        // Order matters: "apple/banana/fruit" wins over "color/red" if
        // both appear, but our test contents don't overlap.
        let idx = if t.contains("apple") || t.contains("banana") || t.contains("fruit") {
            0
        } else if t.contains("color") || t.contains("red") || t.contains("blue") {
            1
        } else {
            2
        };
        v[idx] = 1.0;
        v
    }

    /// Build a real SQLite-backed memory store wired with an embedder so
    /// `save_fact` generates embeddings that we can later look up via
    /// `get_fact_embedding`.
    async fn make_memory_store_with_embedder(
        tmp: &tempfile::TempDir,
        embed: Arc<dyn EmbeddingClient>,
    ) -> Arc<dyn zero_stores::MemoryFactStore> {
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id")
                .expect("vec index init"),
        );
        let memory_repo = Arc::new(MemoryRepository::new(db, vec_index));
        Arc::new(GatewayMemoryFactStore::new(memory_repo, Some(embed)))
    }

    /// RecallConfig with min_score relaxed to 0 — the hybrid scorer
    /// produces values around 0.01–0.05 for synthetic test corpora,
    /// well below the default 0.3 threshold. Tests that exercise the
    /// real store need this so any facts survive the filter.
    fn relaxed_recall_config() -> Arc<RecallConfig> {
        Arc::new(RecallConfig {
            min_score: 0.0,
            ..RecallConfig::default()
        })
    }

    /// MMR disabled → recall_unified output identical to no-MMR.
    /// Sanity check that the gating is the whole seam.
    #[tokio::test]
    async fn mmr_disabled_pipeline_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let embed: Arc<dyn EmbeddingClient> = Arc::new(DirectionalEmbed);
        let store = make_memory_store_with_embedder(&tmp, embed.clone()).await;

        store
            .save_fact(
                "agent",
                "domain",
                "f.apple",
                "apple is a fruit",
                0.8,
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact(
                "agent",
                "domain",
                "f.banana",
                "banana is a fruit",
                0.8,
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact(
                "agent",
                "domain",
                "f.color",
                "red is a color",
                0.8,
                None,
                None,
            )
            .await
            .unwrap();

        let config = relaxed_recall_config();
        let mut recall = MemoryRecall::new(Some(embed), config);
        recall.set_memory_store(store.clone());

        // Baseline (no MMR wired).
        let baseline = recall
            .recall_unified("agent", "fruit", None, &[], 10)
            .await
            .unwrap();

        // Explicit disabled.
        recall.set_mmr_config(MmrConfig {
            enabled: false,
            lambda: 0.5,
            candidate_pool: 30,
        });
        let disabled = recall
            .recall_unified("agent", "fruit", None, &[], 10)
            .await
            .unwrap();

        assert_eq!(
            baseline.len(),
            disabled.len(),
            "disabled MMR must not change result count"
        );
        let baseline_ids: Vec<&str> = baseline.iter().map(|i| i.id.as_str()).collect();
        let disabled_ids: Vec<&str> = disabled.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            baseline_ids, disabled_ids,
            "disabled MMR must not change result order"
        );
    }

    /// MMR enabled with two duplicate-direction facts → only one survives
    /// in the top-2 output; the diverse third fact takes the other slot.
    #[tokio::test]
    async fn mmr_drops_near_duplicate_in_top_k() {
        let tmp = tempfile::tempdir().unwrap();
        let embed: Arc<dyn EmbeddingClient> = Arc::new(DirectionalEmbed);
        let store = make_memory_store_with_embedder(&tmp, embed.clone()).await;

        // Two facts in the "fruit" direction (near-duplicate embeddings).
        store
            .save_fact(
                "agent",
                "domain",
                "f.apple",
                "apple is a fruit",
                0.9,
                None,
                None,
            )
            .await
            .unwrap();
        store
            .save_fact(
                "agent",
                "domain",
                "f.banana",
                "banana is a fruit",
                0.85,
                None,
                None,
            )
            .await
            .unwrap();
        // One fact in a different direction (orthogonal).
        store
            .save_fact(
                "agent",
                "domain",
                "f.color",
                "red is a color",
                0.6,
                None,
                None,
            )
            .await
            .unwrap();

        let config = relaxed_recall_config();
        let mut recall = MemoryRecall::new(Some(embed), config);
        recall.set_memory_store(store.clone());
        recall.set_mmr_config(MmrConfig {
            enabled: true,
            lambda: 0.5,
            candidate_pool: 30,
        });

        let out = recall
            .recall_unified("agent", "fruit", None, &[], 2)
            .await
            .unwrap();

        assert_eq!(out.len(), 2, "budget=2 → 2 items");
        // The color fact must be present — diversity bonus pushes it over
        // the second fruit fact.
        let color_present = out.iter().any(|i| i.content.contains("f.color"));
        let fruit_count = out
            .iter()
            .filter(|i| i.content.contains("f.apple") || i.content.contains("f.banana"))
            .count();
        assert!(
            color_present,
            "MMR should surface the orthogonal color fact over a second near-duplicate fruit"
        );
        assert_eq!(
            fruit_count, 1,
            "exactly one fruit fact should survive the diversity penalty"
        );
    }

    /// Belief + source fact in the same direction — MMR keeps just one.
    /// This is the canonical B-4 case the design doc Q3 deferred to MMR.
    #[tokio::test]
    async fn mmr_dedups_belief_and_source_fact() {
        let tmp = tempfile::tempdir().unwrap();
        let embed: Arc<dyn EmbeddingClient> = Arc::new(DirectionalEmbed);
        let store = make_memory_store_with_embedder(&tmp, embed.clone()).await;

        // Source fact in the "fruit" direction.
        store
            .save_fact(
                "agent",
                "user",
                "f.fruit_preference",
                "user likes apple",
                0.9,
                None,
                None,
            )
            .await
            .unwrap();
        // An orthogonal fact (color direction) to provide the diverse pick.
        store
            .save_fact(
                "agent",
                "domain",
                "f.color",
                "red is a color the user mentioned",
                0.4,
                None,
                None,
            )
            .await
            .unwrap();

        // Belief overlaps semantically with the fruit fact (the embedder
        // routes "apple" content into the same direction).
        let belief = sample_belief(
            "b-fruit",
            "user.preferences",
            "user prefers apple-flavored things",
        );
        let belief_emb_bytes: Vec<u8> = direction_for_text(&belief.content)
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let mut belief_with_emb = belief.clone();
        belief_with_emb.embedding = Some(belief_emb_bytes);
        let stub: Arc<dyn BeliefStore> = Arc::new(StubBeliefStore::new(vec![ScoredBelief {
            belief: belief_with_emb,
            score: 0.95,
        }]));

        let config = relaxed_recall_config();
        let mut recall = MemoryRecall::new(Some(embed), config);
        recall.set_memory_store(store.clone());
        recall.set_belief_store(stub);
        recall.set_mmr_config(MmrConfig {
            enabled: true,
            lambda: 0.5,
            candidate_pool: 30,
        });

        let out = recall
            .recall_unified("agent", "apple preferences", None, &[], 2)
            .await
            .unwrap();

        assert_eq!(out.len(), 2);
        // We must NOT see both the belief and the source fact at top-2 —
        // they're in the same direction. MMR keeps one, swaps the other
        // for the orthogonal color fact.
        let belief_count = out
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Belief))
            .count();
        let apple_fact_count = out
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Fact) && i.content.contains("apple"))
            .count();
        assert!(
            belief_count + apple_fact_count <= 1,
            "MMR should keep only one of (belief, apple fact) at top-2, got {} belief + {} apple fact",
            belief_count,
            apple_fact_count
        );
        let color_present = out.iter().any(|i| i.content.contains("f.color"));
        assert!(
            color_present,
            "the diverse orthogonal item should be selected"
        );
    }

    /// `candidate_pool` controls the over-fetch from RRF. With a wide
    /// pool, MMR sees the orthogonal item even when many high-relevance
    /// near-duplicates exist. Proves the knob is honored on the recall
    /// path.
    #[tokio::test]
    async fn mmr_candidate_pool_is_respected() {
        let tmp = tempfile::tempdir().unwrap();
        let embed: Arc<dyn EmbeddingClient> = Arc::new(DirectionalEmbed);
        let store = make_memory_store_with_embedder(&tmp, embed.clone()).await;

        // 6 fruit facts (all in the same direction).
        for i in 0..6 {
            store
                .save_fact(
                    "agent",
                    "domain",
                    &format!("f.fruit_{i}"),
                    &format!("fruit fact {i} apple"),
                    0.9 - (i as f64) * 0.01,
                    None,
                    None,
                )
                .await
                .unwrap();
        }
        // One orthogonal color fact at mid-relevance.
        store
            .save_fact(
                "agent",
                "domain",
                "f.color",
                "red is a color",
                0.5,
                None,
                None,
            )
            .await
            .unwrap();

        let config = relaxed_recall_config();
        let mut recall = MemoryRecall::new(Some(embed.clone()), config.clone());
        recall.set_memory_store(store.clone());
        recall.set_mmr_config(MmrConfig {
            enabled: true,
            lambda: 0.5,
            candidate_pool: 30,
        });
        let wide = recall
            .recall_unified("agent", "fruit", None, &[], 3)
            .await
            .unwrap();
        assert!(
            wide.iter().any(|i| i.content.contains("f.color")),
            "with candidate_pool=30, the diverse color fact must appear at top-3 — got {:?}",
            wide.iter().map(|i| &i.content).collect::<Vec<_>>()
        );
    }

    /// Empty candidate list → empty result, no panic.
    #[tokio::test]
    async fn mmr_empty_candidates_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let embed: Arc<dyn EmbeddingClient> = Arc::new(DirectionalEmbed);
        let store = make_memory_store_with_embedder(&tmp, embed.clone()).await;

        let config = relaxed_recall_config();
        let mut recall = MemoryRecall::new(Some(embed), config);
        recall.set_memory_store(store);
        recall.set_mmr_config(MmrConfig {
            enabled: true,
            lambda: 0.5,
            candidate_pool: 30,
        });

        let out = recall
            .recall_unified("agent", "anything", None, &[], 10)
            .await
            .unwrap();
        assert!(out.is_empty());
    }
}
