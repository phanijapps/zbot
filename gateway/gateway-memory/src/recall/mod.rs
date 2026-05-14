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
pub use scored_item::{intent_boost, rrf_merge, GoalLite, ItemKind, Provenance, ScoredItem};

use std::sync::Arc;

use crate::intent_router::{IntentClassifier, IntentProfiles};
use crate::rerank::CrossEncoderReranker;
use crate::RecallConfig;
use agent_runtime::llm::embedding::EmbeddingClient;
use zero_stores_domain::{MemoryFact, Procedure, ScoredFact};

/// Greedy MMR pick-order over a scored candidate pool.
///
/// Pure synchronous core of [`mmr_rerank`] — exposed as its own helper
/// so the math can be unit-tested without standing up a store mock.
/// Returns indices into `scores`/`embeddings`, in the order they were
/// picked. Candidates with `None` embeddings contribute similarity
/// `0.0` against everyone (maximally novel — never dropped).
fn mmr_pick_order(
    scores: &[f64],
    embeddings: &[Option<Vec<f32>>],
    lambda: f64,
    limit: usize,
) -> Vec<usize> {
    let n = scores.len();
    if n == 0 {
        return Vec::new();
    }
    let take = limit.min(n);
    let mut picked: Vec<usize> = Vec::with_capacity(take);
    let mut remaining: Vec<usize> = (0..n).collect();
    while picked.len() < take && !remaining.is_empty() {
        let mut best_idx_in_remaining = 0;
        let mut best_score = f64::MIN;
        for (i, &cand) in remaining.iter().enumerate() {
            let max_sim = picked
                .iter()
                .map(|&p| match (&embeddings[cand], &embeddings[p]) {
                    (Some(a), Some(b)) => cosine_similarity(a, b),
                    _ => 0.0,
                })
                .fold(0.0_f64, f64::max);
            let mmr_score = lambda * scores[cand] - (1.0 - lambda) * max_sim;
            if mmr_score > best_score {
                best_score = mmr_score;
                best_idx_in_remaining = i;
            }
        }
        picked.push(remaining.swap_remove(best_idx_in_remaining));
    }
    picked
}

/// Reorder candidates to balance relevance and diversity (MMR).
///
/// Greedy O(K²) selection: at each step, pick the candidate that
/// maximizes `λ · score − (1 − λ) · max(cosine(candidate, already_picked))`.
///
/// Hydrates missing embeddings via [`MemoryFactStore::get_fact_embedding`].
/// Candidates whose embedding can't be hydrated still get included with a
/// similarity term of 0.0 — better than silently dropping them.
async fn mmr_rerank(
    candidates: &mut Vec<ScoredFact>,
    memory_store: &Arc<dyn zero_stores::MemoryFactStore>,
    cfg: &crate::MmrConfig,
    limit: usize,
) -> Result<(), String> {
    if !cfg.enabled || candidates.len() <= limit {
        return Ok(());
    }

    // 1. Truncate to candidate_pool to bound the greedy loop.
    let pool_size = cfg.candidate_pool.min(candidates.len());
    candidates.truncate(pool_size);

    // 2. Hydrate missing embeddings.
    let mut embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(pool_size);
    for sf in candidates.iter() {
        if let Some(emb) = sf.fact.embedding.clone() {
            embeddings.push(Some(emb));
        } else {
            let hydrated = memory_store
                .get_fact_embedding(&sf.fact.id)
                .await
                .ok()
                .flatten();
            embeddings.push(hydrated);
        }
    }

    // 3. Greedy pick using the pure helper.
    let scores: Vec<f64> = candidates.iter().map(|sf| sf.score).collect();
    let picked = mmr_pick_order(&scores, &embeddings, cfg.lambda, limit);

    // 4. Replace `candidates` with the picked order.
    let reordered: Vec<ScoredFact> = picked.iter().map(|&idx| candidates[idx].clone()).collect();
    *candidates = reordered;

    Ok(())
}

/// Cosine similarity between two `f32` vectors.
///
/// Returns `0.0` for empty or length-mismatched inputs (caller treats
/// as maximally novel rather than dropping the candidate).
///
/// Exposed as `pub(crate)` so [`crate::intent_router::KnnIntentClassifier`]
/// can reuse the same arithmetic as MMR.
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        dot += ai * bi;
        na += ai * ai;
        nb += bi * bi;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

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
    reranker: Option<Arc<dyn CrossEncoderReranker>>,
    classifier: Option<Arc<dyn IntentClassifier>>,
    profiles: Option<Arc<IntentProfiles>>,
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
            reranker: None,
            classifier: None,
            profiles: None,
            config,
        }
    }

    /// Wire the cross-encoder reranker (MEM-007). When set and
    /// `config.rerank.enabled` is true, the reranker runs after MMR
    /// and before the final truncate-to-top-K in [`Self::recall`].
    pub fn set_reranker(&mut self, reranker: Arc<dyn CrossEncoderReranker>) {
        self.reranker = Some(reranker);
    }

    /// Wire the intent classifier (MEM-008). When set, `recall()` calls
    /// `classify(query)` at the start of the pipeline; a `Some(intent)`
    /// result is looked up in [`Self::set_intent_profiles`] to produce a
    /// per-query effective [`RecallConfig`]. `None` means router-disabled
    /// (every query uses base config).
    pub fn set_intent_classifier(&mut self, classifier: Arc<dyn IntentClassifier>) {
        self.classifier = Some(classifier);
    }

    /// Wire the per-intent profile bank (MEM-008). When set alongside
    /// [`Self::set_intent_classifier`], the classifier's intent label is
    /// used to overlay partial [`RecallConfig`] fields onto the base for
    /// that query only.
    pub fn set_intent_profiles(&mut self, profiles: Arc<IntentProfiles>) {
        self.profiles = Some(profiles);
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
        // 0. Intent routing (MEM-008). When a classifier + profile bank are
        //    wired, derive a per-query effective config by overlaying the
        //    intent's profile on top of the base. Every downstream stage
        //    reads from `effective_config` instead of `self.config` so the
        //    overlay applies to category weights, ward affinity, temporal
        //    decay, contradiction, supersession, min_score, MMR, reranker.
        //    Missing classifier / no confident intent / unknown intent →
        //    effective_config equals base.
        let effective_config: RecallConfig =
            match (self.classifier.as_ref(), self.profiles.as_ref()) {
                (Some(classifier), Some(profiles)) => {
                    if let Some(intent) = classifier.classify(user_message).await {
                        tracing::debug!(intent = %intent, "intent router selected profile overlay");
                        profiles.apply(&self.config, &intent)
                    } else {
                        (*self.config).clone()
                    }
                }
                _ => (*self.config).clone(),
            };

        // 1. Embed the user message for vector search
        let query_embedding = self.embed_query(user_message).await;

        // 2. Run hybrid search (FTS5 + vector). Trait-routed when
        //    memory_store is wired .
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
                    // `zero_stores_sqlite::MemoryFact` re-exports the same
                    // struct from `zero_stores_domain`; using the domain
                    // path keeps gateway-memory off the sqlite crate (the
                    // sqlite crate depends on gateway-services, which would
                    // cycle back through gateway-memory).
                    serde_json::from_value::<MemoryFact>(v)
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
                    effective_config.high_confidence_threshold,
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
            let category_weight = effective_config.category_weight(&sf.fact.category);
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
                        sf.score *= effective_config.ward_affinity_boost;
                    }
                }
            }
        }

        // 7. Apply temporal decay — older facts score lower based on per-category half-lives
        if effective_config.temporal_decay.enabled {
            for sf in &mut results {
                // Skill/agent indices don't decay (re-indexed each session)
                if sf.fact.category == "skill" || sf.fact.category == "agent" {
                    continue;
                }
                let half_life = effective_config
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
                sf.score *= effective_config.contradiction_penalty;
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
        results.retain(|sf| sf.score >= effective_config.min_score);

        // 9.5 MMR diversity reranking (MEM-006). Reorders the top-N pool to
        // demote near-duplicates of items already picked. Internal truncation
        // brings the list down to `limit`; the explicit truncate below stays
        // as a defensive no-op.
        if effective_config.mmr.enabled {
            if let Some(store) = self.memory_store.as_ref() {
                mmr_rerank(&mut results, store, &effective_config.mmr, limit).await?;
            }
        }

        // 9.7 Cross-encoder reranker (MEM-007). Runs after MMR so the
        // model sees a diversity-reordered pool. The reranker's own
        // top_k_after caps the output; we still truncate to `limit`
        // below in case top_k_after > limit.
        if effective_config.rerank.enabled {
            if let Some(reranker) = self.reranker.as_ref() {
                results = reranker.rerank(user_message, results).await;
            }
        }

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

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![1.0_f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-9);
    }

    #[test]
    fn cosine_mismatched_lengths_returns_zero() {
        assert_eq!(cosine_similarity(&[1.0_f32], &[1.0_f32, 2.0_f32]), 0.0);
        assert_eq!(cosine_similarity(&[] as &[f32], &[1.0_f32]), 0.0);
        assert_eq!(cosine_similarity(&[1.0_f32], &[] as &[f32]), 0.0);
    }

    #[test]
    fn mmr_lambda_one_preserves_score_order() {
        let scores = vec![1.0, 0.8, 0.6, 0.4];
        // Identical embeddings — diversity term is at maximum but λ=1.0
        // zeroes its contribution, so order is pure-score.
        let embeddings: Vec<Option<Vec<f32>>> = vec![Some(vec![1.0_f32]); 4];
        let order = mmr_pick_order(&scores, &embeddings, 1.0, 4);
        assert_eq!(order, vec![0, 1, 2, 3]);
    }

    #[test]
    fn mmr_demotes_near_duplicate_of_top() {
        // #1: high score, distinct embedding
        // #2: slightly lower score, near-identical embedding to #1
        // #3: lowest score, distinct embedding
        let scores = vec![1.0, 0.95, 0.8];
        let embeddings = vec![
            Some(vec![1.0_f32, 0.0]),
            Some(vec![0.99_f32, 0.14]), // near-duplicate of #1
            Some(vec![0.0_f32, 1.0]),   // orthogonal to #1
        ];
        let order = mmr_pick_order(&scores, &embeddings, 0.6, 3);
        assert_eq!(order[0], 0, "highest score wins first slot");
        assert_eq!(order[1], 2, "diverse #3 beats near-duplicate #2");
        assert_eq!(order[2], 1);
    }

    #[test]
    fn mmr_lambda_zero_picks_most_novel() {
        // λ=0 ignores score entirely. First pick is arbitrary (first in
        // remaining order); then each subsequent pick is the most-novel
        // relative to what's already picked.
        let scores = vec![1.0, 0.9, 0.8];
        let embeddings = vec![
            Some(vec![1.0_f32, 0.0]),
            Some(vec![0.95_f32, 0.31]), // similar to #1
            Some(vec![0.0_f32, 1.0]),   // distinct from #1
        ];
        let order = mmr_pick_order(&scores, &embeddings, 0.0, 3);
        assert_eq!(order[0], 0, "first pick (no diversity term yet)");
        assert_eq!(order[1], 2, "most novel relative to #1");
    }

    #[test]
    fn mmr_missing_embedding_treated_as_zero_similarity() {
        // Candidate with None embedding: similarity term is 0 → looks
        // maximally novel. It should NOT be dropped.
        let scores = vec![1.0, 0.9];
        let embeddings: Vec<Option<Vec<f32>>> = vec![Some(vec![1.0_f32, 0.0]), None];
        let order = mmr_pick_order(&scores, &embeddings, 0.6, 2);
        assert_eq!(order.len(), 2, "both included");
        // With λ=0.6: candidate 0 first (no picks yet, only its score matters).
        // Then candidate 1: 0.6*0.9 − 0.4*0 = 0.54. Score order wins.
        assert_eq!(order, vec![0, 1]);
    }

    /// Minimal `MemoryFactStore` stub for MMR integration tests. The
    /// trait requires `save_fact` and `recall_facts` to be implemented;
    /// every other method (including `get_fact_embedding`, the only one
    /// MMR actually calls) falls through to the trait's default impl —
    /// our candidates already carry their embeddings inline, so the
    /// hydration call short-circuits before reaching the store.
    struct NoopStore;

    #[async_trait::async_trait]
    impl zero_stores::MemoryFactStore for NoopStore {
        async fn save_fact(
            &self,
            _agent_id: &str,
            _category: &str,
            _key: &str,
            _content: &str,
            _confidence: f64,
            _session_id: Option<&str>,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }

        async fn recall_facts(
            &self,
            _agent_id: &str,
            _query: &str,
            _limit: usize,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!([]))
        }
    }

    fn mk_scored(id: &str, score: f64, embedding: Vec<f32>) -> ScoredFact {
        ScoredFact {
            fact: MemoryFact {
                id: id.to_string(),
                session_id: None,
                agent_id: "agent-1".to_string(),
                scope: "agent".to_string(),
                category: "misc".to_string(),
                key: format!("test.{id}"),
                content: format!("content-{id}"),
                confidence: 0.9,
                mention_count: 1,
                source_summary: None,
                embedding: Some(embedding),
                ward_id: "__global__".to_string(),
                contradicted_by: None,
                created_at: String::new(),
                updated_at: String::new(),
                expires_at: None,
                valid_from: None,
                valid_until: None,
                superseded_by: None,
                pinned: false,
                epistemic_class: Some("current".to_string()),
                source_episode_id: None,
                source_ref: None,
            },
            score,
        }
    }

    #[tokio::test]
    async fn mmr_integration_diverse_set_promoted() {
        // Four candidates with descending scores. f1/f2/f3 share a
        // near-identical embedding cluster; f4 is orthogonal. With
        // λ=0.6 and limit=3, the diverse f4 must appear in top-3
        // despite its lowest raw score, evicting one of the cluster.
        let candidates = vec![
            mk_scored("f1", 1.0, vec![1.0, 0.0]),
            mk_scored("f2", 0.9, vec![0.99, 0.14]),
            mk_scored("f3", 0.8, vec![0.98, 0.20]),
            mk_scored("f4", 0.7, vec![0.0, 1.0]),
        ];
        let store: Arc<dyn zero_stores::MemoryFactStore> = Arc::new(NoopStore);
        let mut working = candidates;
        let cfg = crate::MmrConfig {
            enabled: true,
            lambda: 0.6,
            candidate_pool: 4,
        };
        mmr_rerank(&mut working, &store, &cfg, 3).await.unwrap();
        let ids: Vec<String> = working.iter().map(|sf| sf.fact.id.clone()).collect();
        assert_eq!(working.len(), 3, "limit honored");
        assert!(
            ids.contains(&"f4".to_string()),
            "diverse f4 should be in top-3, got {ids:?}"
        );
        assert_eq!(ids[0], "f1", "highest score still wins first slot");
    }

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
}
