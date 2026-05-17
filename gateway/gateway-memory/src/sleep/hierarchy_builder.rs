//! HierarchyBuilder — recursive aggregation of layer-N entities into
//! layer-N+1 aggregates (Phase H-3e).
//!
//! Ties together the four lower H-3 layers:
//!
//!   1. Fetch layer-N entities + embeddings via
//!      `KnowledgeGraphStore::list_entities_with_embeddings_at_layer`
//!      (H-3e-1, shipped).
//!   2. K-means cluster the embeddings via
//!      [`crate::sleep::clustering::kmeans_cosine`] (H-3a, shipped).
//!   3. For each cluster, synthesise an aggregate entity via an LLM
//!      (singletons short-circuit — no LLM call) and write it through
//!      `promote_cluster_to_aggregate` (H-3d, shipped).
//!   4. For each cluster pair whose connectivity strength λ exceeds
//!      `inter_cluster_relation_threshold` (H-3c), synthesise an
//!      inter-cluster relation via the LLM and write it through
//!      `write_inter_cluster_relation` (H-3d, shipped).
//!   5. Compute `cluster_sparsity` (H-3b) and stop the layer loop when
//!      the change vs the previous layer is ≤ `sparsity_epsilon`.
//!
//! Mirrors the shape of `BeliefSynthesizer`: trait-based LLM so tests
//! mock the synthesis without an LLM provider; `embedding_client` is
//! optional so non-embedding setups still build the hierarchy (the
//! aggregates just won't surface in semantic recall until a future
//! reindex pass picks them up).

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use agent_runtime::llm::EmbeddingClient;
use zero_stores::types::EntityId;
use zero_stores::KnowledgeGraphStore;

use crate::sleep::clustering::{
    cluster_sparsity, kmeans_cosine, should_stop_layering, DEFAULT_KMEANS_MAX_ITER,
    DEFAULT_SPARSITY_EPSILON,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// LLM abstraction for aggregate-entity + inter-cluster relation synthesis.
/// Mockable in tests; production wiring goes through a thin wrapper that
/// formats the prompt and parses the response JSON, same shape as
/// `LlmBeliefSynthesizer`.
#[async_trait]
pub trait AggregateEntityLlm: Send + Sync {
    /// Summarise a multi-member cluster into an aggregate entity.
    /// Singleton clusters short-circuit BEFORE this is called — they
    /// just promote the single member with no LLM cost.
    async fn synthesize_aggregate(
        &self,
        members: &[AggregateMemberContext],
    ) -> Result<AggregateResponse, String>;

    /// Pick a relationship type for an inter-cluster edge. `lambda` is
    /// the connectivity strength (the count of underlying edges that
    /// crossed the threshold). The LLM is expected to return a short
    /// lowercase verb-phrase (e.g. "encompasses", "differs-from"); the
    /// orchestrator falls back to "related-via" if the call fails.
    async fn synthesize_relation(
        &self,
        agg_a_name: &str,
        agg_b_name: &str,
        lambda: usize,
    ) -> Result<String, String>;
}

/// Minimal per-member context the LLM sees. We deliberately don't
/// pass the full Entity row — descriptions live in the `properties`
/// JSON on aggregates, and the LLM should make decisions based on
/// names + (optional) descriptions, nothing else.
#[derive(Debug, Clone)]
pub struct AggregateMemberContext {
    pub id: EntityId,
    pub name: String,
    pub description: Option<String>,
}

/// LLM response shape for a synthesised aggregate.
#[derive(Debug, Clone)]
pub struct AggregateResponse {
    pub name: String,
    pub description: String,
}

/// Tuning parameters. Carries safe defaults that match
/// `project_hierarchical_memory_plan.md`.
#[derive(Debug, Clone)]
pub struct HierarchyConfig {
    /// Target cluster size. K-means runs with k = max(2, n / target).
    pub cluster_target_size: usize,
    /// Hard cap on the number of layers built per cycle.
    pub max_layers: u32,
    /// Stop when `cluster_sparsity` between layers changes by ≤ this.
    pub sparsity_epsilon: f32,
    /// Inter-cluster relation gate. Skip when λ ≤ this value.
    pub inter_cluster_relation_threshold: usize,
    /// Per-cycle ceiling on LLM calls. Each cluster synthesis + each
    /// inter-cluster relation counts as one call. Singletons don't.
    pub llm_budget_per_cycle: u32,
    /// K-means seed. Pinned so re-runs produce the same labels for
    /// the same input — useful for debugging clustering quality.
    pub seed: u64,
}

impl Default for HierarchyConfig {
    fn default() -> Self {
        Self {
            cluster_target_size: 20,
            max_layers: 4,
            sparsity_epsilon: DEFAULT_SPARSITY_EPSILON,
            inter_cluster_relation_threshold: 3,
            llm_budget_per_cycle: 50,
            seed: 0x6261_7365_6c69_6e65, // ascii "baseline"
        }
    }
}

/// Counts emitted from one `run_for_agent` call. Used by the future
/// sleep-cycle wiring (H-3f) to populate the observatory's activity
/// feed without re-querying the DB.
#[derive(Debug, Default, Clone)]
pub struct HierarchyStats {
    pub layers_built: u32,
    pub aggregates_created: u64,
    pub singletons_promoted: u64,
    pub inter_cluster_relations_created: u64,
    pub llm_calls: u64,
    pub stopped_reason: StopReason,
    pub errors: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit `max_layers`.
    MaxLayers,
    /// `cluster_sparsity` change ≤ epsilon.
    Converged,
    /// Pool too small to cluster meaningfully.
    PoolTooSmall,
    /// K-means produced a single cluster (degenerate).
    SingleCluster,
    /// LLM budget exhausted mid-cycle.
    BudgetExhausted,
    /// Initial layer fetch returned an error.
    #[default]
    NotStarted,
}

// ---------------------------------------------------------------------------
// HierarchyBuilder
// ---------------------------------------------------------------------------

pub struct HierarchyBuilder {
    kg_store: Arc<dyn KnowledgeGraphStore>,
    llm: Arc<dyn AggregateEntityLlm>,
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
    config: HierarchyConfig,
    /// Minimum time between cycles. `Duration::ZERO` (default) runs
    /// the builder every time the sleep cycle invokes it — matches
    /// the test-friendly shape of `BeliefSynthesizer`. The sleep-cycle
    /// wrapper sets this from `HierarchySettings.interval_hours`.
    interval: Duration,
    last_run: Mutex<Option<Instant>>,
}

impl HierarchyBuilder {
    pub fn new(kg_store: Arc<dyn KnowledgeGraphStore>, llm: Arc<dyn AggregateEntityLlm>) -> Self {
        Self {
            kg_store,
            llm,
            embedding_client: None,
            config: HierarchyConfig::default(),
            interval: Duration::ZERO,
            last_run: Mutex::new(None),
        }
    }

    pub fn with_embedding_client(mut self, client: Option<Arc<dyn EmbeddingClient>>) -> Self {
        self.embedding_client = client;
        self
    }

    pub fn with_config(mut self, config: HierarchyConfig) -> Self {
        self.config = config;
        self
    }

    /// Minimum time between successful cycles. `Duration::ZERO` runs
    /// every tick (the test-friendly default). Production wiring
    /// passes `Duration::from_secs(interval_hours * 3600)` from
    /// `HierarchySettings.interval_hours`.
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Has enough time passed since the last successful cycle? Always
    /// `true` when `interval == ZERO` (the test path).
    fn throttle_allows_run(&self) -> bool {
        if self.interval.is_zero() {
            return true;
        }
        match *self.last_run.lock().unwrap() {
            Some(last) => last.elapsed() >= self.interval,
            None => true,
        }
    }

    /// Run one build cycle for a single agent. Returns stats; never
    /// errors — partial failures (LLM, write) are logged and counted
    /// in `stats.errors` so the daemon's hourly loop keeps moving.
    /// Skipped (no DB / LLM cost) when the throttle interval hasn't
    /// elapsed since the previous successful cycle.
    pub async fn run_for_agent(&self, agent_id: &str) -> HierarchyStats {
        let mut stats = HierarchyStats::default();
        if !self.throttle_allows_run() {
            debug!(agent_id, "hierarchy: throttled (interval not elapsed)");
            return stats;
        }
        // Record the cycle start so subsequent calls within `interval`
        // are skipped. Same shape as BeliefSynthesizer — timestamping
        // any "we got past the throttle" entry, not just successful
        // ones, prevents busy-looping on PoolTooSmall etc.
        *self.last_run.lock().unwrap() = Some(Instant::now());
        let mut prev_sparsity: Option<f32> = None;

        for layer in 0..self.config.max_layers {
            let pool = match self
                .kg_store
                .list_entities_with_embeddings_at_layer(agent_id, layer as i64, 0)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        agent_id,
                        layer, error = ?e, "hierarchy: layer fetch failed"
                    );
                    stats.errors += 1;
                    return stats;
                }
            };

            if pool.len() < self.config.cluster_target_size.max(2) {
                debug!(
                    agent_id,
                    layer,
                    pool_size = pool.len(),
                    "hierarchy: pool too small, stopping"
                );
                stats.stopped_reason = StopReason::PoolTooSmall;
                return stats;
            }

            let n = pool.len();
            let k = (n / self.config.cluster_target_size).max(2);
            let embeddings: Vec<Vec<f32>> = pool.iter().map(|p| p.embedding.clone()).collect();
            let labels = kmeans_cosine(&embeddings, k, self.config.seed, DEFAULT_KMEANS_MAX_ITER);
            let distinct_labels: std::collections::HashSet<_> = labels.iter().copied().collect();
            if distinct_labels.len() < 2 {
                debug!(
                    agent_id,
                    layer, "hierarchy: K-means collapsed to one cluster, stopping"
                );
                stats.stopped_reason = StopReason::SingleCluster;
                return stats;
            }

            let current_sparsity = cluster_sparsity(&labels);
            if let Some(prev) = prev_sparsity {
                if should_stop_layering(prev, current_sparsity, self.config.sparsity_epsilon) {
                    debug!(
                        agent_id,
                        layer, prev, current_sparsity, "hierarchy: sparsity converged"
                    );
                    stats.stopped_reason = StopReason::Converged;
                    return stats;
                }
            }

            // Group entity ids by cluster label.
            let mut clusters: Vec<Vec<EntityId>> = vec![Vec::new(); k];
            for (idx, &lab) in labels.iter().enumerate() {
                clusters[lab].push(pool[idx].id.clone());
            }

            // Materialise each cluster as a layer+1 aggregate.
            let mut aggregate_ids: Vec<Option<EntityId>> = vec![None; clusters.len()];
            for (cluster_idx, members) in clusters.iter().enumerate() {
                if members.is_empty() {
                    continue;
                }
                let agg_id = match self
                    .promote_one_cluster(agent_id, (layer as i64) + 1, members, &mut stats)
                    .await
                {
                    Ok(id) => id,
                    Err(()) => continue,
                };
                aggregate_ids[cluster_idx] = Some(agg_id);
            }

            // Inter-cluster relations: for each (i,j) pair where
            // λ > threshold, synthesise + write. Budget-capped.
            for i in 0..clusters.len() {
                for j in (i + 1)..clusters.len() {
                    if stats.llm_calls >= self.config.llm_budget_per_cycle as u64 {
                        info!(
                            agent_id,
                            layer,
                            calls = stats.llm_calls,
                            "hierarchy: llm budget exhausted; skipping remaining pairs"
                        );
                        stats.stopped_reason = StopReason::BudgetExhausted;
                        break;
                    }
                    let (Some(agg_i), Some(agg_j)) =
                        (aggregate_ids[i].as_ref(), aggregate_ids[j].as_ref())
                    else {
                        continue;
                    };
                    let lambda = match self
                        .kg_store
                        .connectivity_strength(agent_id, &clusters[i], &clusters[j])
                        .await
                    {
                        Ok(l) => l,
                        Err(e) => {
                            warn!(
                                agent_id,
                                layer, error = ?e, "hierarchy: connectivity query failed"
                            );
                            stats.errors += 1;
                            continue;
                        }
                    };
                    if lambda <= self.config.inter_cluster_relation_threshold {
                        continue;
                    }
                    self.write_inter_cluster_pair(
                        agent_id,
                        (layer as i64) + 1,
                        agg_i,
                        agg_j,
                        lambda,
                        &mut stats,
                    )
                    .await;
                }
                if stats.stopped_reason == StopReason::BudgetExhausted {
                    break;
                }
            }

            stats.layers_built = layer + 1;
            prev_sparsity = Some(current_sparsity);

            if stats.stopped_reason == StopReason::BudgetExhausted {
                return stats;
            }
        }

        if stats.stopped_reason == StopReason::NotStarted {
            stats.stopped_reason = StopReason::MaxLayers;
        }
        stats
    }

    // ---- internal helpers ----

    /// Promote one cluster's members up to `layer`. Returns the new
    /// aggregate's id, or `Err(())` when a downstream call failed
    /// (already logged + counted in `stats.errors`).
    async fn promote_one_cluster(
        &self,
        agent_id: &str,
        layer: i64,
        members: &[EntityId],
        stats: &mut HierarchyStats,
    ) -> Result<EntityId, ()> {
        // Singleton short-circuit — no LLM, no extra description, just
        // promote the member by wrapping it under a new layer entry.
        // We still need a name + description for the aggregate row,
        // so we recycle the member's id-as-name (the H-3 v1 cut keeps
        // singleton naming dumb; H-3e-v2 can fetch the real name).
        if members.len() == 1 {
            let name = members[0].0.clone();
            let result = self
                .kg_store
                .promote_cluster_to_aggregate(
                    agent_id,
                    layer,
                    members,
                    &name,
                    "singleton aggregate (no LLM)",
                    None,
                )
                .await;
            match result {
                Ok(id) => {
                    stats.singletons_promoted += 1;
                    return Ok(id);
                }
                Err(e) => {
                    warn!(agent_id, layer, error = ?e, "hierarchy: singleton promote failed");
                    stats.errors += 1;
                    return Err(());
                }
            }
        }

        // Multi-member: LLM call.
        if stats.llm_calls >= self.config.llm_budget_per_cycle as u64 {
            info!(
                agent_id,
                layer, "hierarchy: llm budget exhausted; skipping cluster"
            );
            return Err(());
        }
        let contexts: Vec<AggregateMemberContext> = members
            .iter()
            .map(|id| AggregateMemberContext {
                id: id.clone(),
                // Names come from the kg_entities rows — but the
                // orchestrator already paid to fetch only id + embedding
                // (H-3e-1's narrow API). For v1, pass the id as the
                // name string. Production wiring (the LlmAggregateEntity
                // adapter) can join against kg_entities to enrich.
                name: id.0.clone(),
                description: None,
            })
            .collect();
        stats.llm_calls += 1;
        let resp = match self.llm.synthesize_aggregate(&contexts).await {
            Ok(r) => r,
            Err(e) => {
                warn!(agent_id, layer, error = %e, "hierarchy: aggregate LLM failed");
                stats.errors += 1;
                return Err(());
            }
        };

        // Embed the description for kg_name_index if we have a client.
        let embedding = match &self.embedding_client {
            Some(client) => match client.embed(&[resp.description.as_str()]).await {
                Ok(mut vs) if !vs.is_empty() => vs.pop(),
                Ok(_) => None,
                Err(e) => {
                    warn!(agent_id, layer, error = %e, "hierarchy: embed failed");
                    None
                }
            },
            None => None,
        };

        match self
            .kg_store
            .promote_cluster_to_aggregate(
                agent_id,
                layer,
                members,
                &resp.name,
                &resp.description,
                embedding,
            )
            .await
        {
            Ok(id) => {
                stats.aggregates_created += 1;
                Ok(id)
            }
            Err(e) => {
                warn!(agent_id, layer, error = ?e, "hierarchy: aggregate write failed");
                stats.errors += 1;
                Err(())
            }
        }
    }

    async fn write_inter_cluster_pair(
        &self,
        agent_id: &str,
        layer: i64,
        agg_a: &EntityId,
        agg_b: &EntityId,
        lambda: usize,
        stats: &mut HierarchyStats,
    ) {
        stats.llm_calls += 1;
        let rtype = match self
            .llm
            .synthesize_relation(&agg_a.0, &agg_b.0, lambda)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                warn!(agent_id, layer, error = %e, "hierarchy: relation LLM failed; falling back");
                "related-via".to_string()
            }
        };
        match self
            .kg_store
            .write_inter_cluster_relation(agent_id, layer, agg_a, agg_b, &rtype)
            .await
        {
            Ok(_) => {
                stats.inter_cluster_relations_created += 1;
            }
            Err(e) => {
                warn!(agent_id, layer, error = ?e, "hierarchy: inter-cluster write failed");
                stats.errors += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_runtime::llm::EmbeddingError;
    use gateway_services::VaultPaths;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use zero_stores_sqlite::kg::storage::GraphStorage;
    use zero_stores_sqlite::KnowledgeDatabase;
    use zero_stores_sqlite::SqliteKgStore;

    // ---- fakes ----

    struct MockLlm {
        synth_calls: Mutex<u64>,
        relation_calls: Mutex<u64>,
        relation_response: String,
        synth_should_fail: bool,
    }

    impl MockLlm {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                synth_calls: Mutex::new(0),
                relation_calls: Mutex::new(0),
                relation_response: "encompasses".to_string(),
                synth_should_fail: false,
            })
        }

        fn synth_call_count(&self) -> u64 {
            *self.synth_calls.lock().unwrap()
        }
    }

    #[async_trait]
    impl AggregateEntityLlm for MockLlm {
        async fn synthesize_aggregate(
            &self,
            members: &[AggregateMemberContext],
        ) -> Result<AggregateResponse, String> {
            *self.synth_calls.lock().unwrap() += 1;
            if self.synth_should_fail {
                return Err("mock fail".into());
            }
            Ok(AggregateResponse {
                name: format!("agg-of-{}-members", members.len()),
                description: format!("Aggregate over {} entities.", members.len()),
            })
        }

        async fn synthesize_relation(
            &self,
            _a: &str,
            _b: &str,
            _lambda: usize,
        ) -> Result<String, String> {
            *self.relation_calls.lock().unwrap() += 1;
            Ok(self.relation_response.clone())
        }
    }

    struct MockEmbedder;

    #[async_trait]
    impl EmbeddingClient for MockEmbedder {
        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            // Constant 384-dim vectors so the kg_name_index dim check passes.
            Ok(texts.iter().map(|_| vec![0.01_f32; 384]).collect())
        }

        fn dimensions(&self) -> usize {
            384
        }

        fn model_name(&self) -> String {
            "mock".to_string()
        }
    }

    // ---- fixture builders ----

    fn build_store_with_layer_zero(
        agent_id: &str,
        n_per_cluster: usize,
        n_clusters: usize,
    ) -> (Arc<dyn KnowledgeGraphStore>, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Arc::new(VaultPaths::new(dir.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).unwrap());
        let storage = Arc::new(GraphStorage::new(db.clone()).unwrap());

        // Seed n_clusters tight blobs at layer 0 — each blob has
        // n_per_cluster members near a unique direction on the unit
        // sphere. K-means should recover the blob assignment.
        for c in 0..n_clusters {
            // Direction: spread blobs around the (0..n_clusters) axis.
            let angle = (c as f32) * std::f32::consts::TAU / (n_clusters as f32);
            let dx = angle.cos();
            let dy = angle.sin();
            for m in 0..n_per_cluster {
                let id = format!("c{c}-m{m}");
                let mut emb = vec![0.0_f32; 384];
                emb[0] = dx + (m as f32) * 0.001;
                emb[1] = dy + (m as f32) * 0.001;
                let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
                for v in emb.iter_mut() {
                    *v /= norm;
                }

                let emb_for_db = emb.clone();
                let id_for_db = id.clone();
                let agent_for_db = agent_id.to_string();
                db.with_connection(|conn| {
                    conn.execute(
                        "INSERT INTO kg_entities
                            (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                             first_seen_at, last_seen_at, layer)
                         VALUES (?1, ?2, 'Concept', ?1, ?1, ?1,
                                 datetime('now'), datetime('now'), 0)",
                        rusqlite::params![id_for_db, agent_for_db],
                    )?;
                    let emb_json = serde_json::to_string(&emb_for_db).unwrap();
                    conn.execute(
                        "INSERT INTO kg_name_index (entity_id, name_embedding) \
                         VALUES (?1, ?2)",
                        rusqlite::params![id_for_db, emb_json],
                    )?;
                    Ok(())
                })
                .unwrap();
            }
        }

        let kg: Arc<dyn KnowledgeGraphStore> = Arc::new(SqliteKgStore::new(storage));
        (kg, dir)
    }

    // ---- tests ----

    #[tokio::test]
    async fn empty_agent_yields_no_aggregates() {
        let (kg, _dir) = build_store_with_layer_zero("agent-empty", 0, 0);
        let llm = MockLlm::new();
        let builder = HierarchyBuilder::new(kg, llm.clone());

        let stats = builder.run_for_agent("agent-empty").await;
        assert_eq!(stats.layers_built, 0);
        assert_eq!(stats.aggregates_created, 0);
        assert_eq!(stats.stopped_reason, StopReason::PoolTooSmall);
        assert_eq!(llm.synth_call_count(), 0);
    }

    #[tokio::test]
    async fn pool_smaller_than_target_short_circuits() {
        // 5 entities + target_size=20 → can't cluster meaningfully.
        let (kg, _dir) = build_store_with_layer_zero("agent-tiny", 5, 1);
        let llm = MockLlm::new();
        let builder = HierarchyBuilder::new(kg, llm.clone());
        let stats = builder.run_for_agent("agent-tiny").await;
        assert_eq!(stats.stopped_reason, StopReason::PoolTooSmall);
        assert_eq!(stats.aggregates_created, 0);
    }

    #[tokio::test]
    async fn three_blobs_build_one_layer_with_three_aggregates() {
        // 3 blobs × 10 members = 30 entities. Target 10 → k=3.
        let (kg, _dir) = build_store_with_layer_zero("agent-blob", 10, 3);
        let llm = MockLlm::new();
        let config = HierarchyConfig {
            cluster_target_size: 10,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg.clone(), llm.clone()).with_config(config);
        let stats = builder.run_for_agent("agent-blob").await;

        assert!(stats.layers_built >= 1, "should build at least one layer");
        assert!(
            stats.aggregates_created >= 2,
            "expected ≥2 multi-member aggregates, got {}",
            stats.aggregates_created
        );
        assert_eq!(llm.synth_call_count(), stats.aggregates_created);
    }

    #[tokio::test]
    async fn singletons_short_circuit_no_llm() {
        // 2 blobs × 1 member + 1 blob × 5 members → still need 7 ≥ 7
        // entities; force target_size=3 so k=2. Each blob ends up
        // small enough that some clusters are singletons.
        let (kg, _dir) = build_store_with_layer_zero("agent-mix", 5, 3);
        let llm = MockLlm::new();
        let config = HierarchyConfig {
            cluster_target_size: 3,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg.clone(), llm.clone()).with_config(config);
        let stats = builder.run_for_agent("agent-mix").await;

        // The exact mix depends on K-means landing, but the invariant
        // is: aggregates_created counts only multi-member clusters,
        // singletons_promoted counts the rest. Together they total the
        // number of clusters that produced an aggregate row.
        assert_eq!(
            llm.synth_call_count(),
            stats.aggregates_created,
            "LLM is called exactly once per multi-member aggregate"
        );
    }

    #[tokio::test]
    async fn llm_failure_increments_error_count_but_continues() {
        let (kg, _dir) = build_store_with_layer_zero("agent-fail", 10, 3);
        let llm: Arc<MockLlm> = Arc::new(MockLlm {
            synth_calls: Mutex::new(0),
            relation_calls: Mutex::new(0),
            relation_response: "x".into(),
            synth_should_fail: true,
        });
        let config = HierarchyConfig {
            cluster_target_size: 10,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg, llm.clone()).with_config(config);
        let stats = builder.run_for_agent("agent-fail").await;
        // Every multi-member cluster fails; LLM was called per cluster.
        assert!(stats.errors > 0, "errors must be counted");
        assert_eq!(stats.aggregates_created, 0);
        assert!(llm.synth_call_count() >= 1);
    }

    #[tokio::test]
    async fn budget_exhaustion_stops_cleanly() {
        let (kg, _dir) = build_store_with_layer_zero("agent-budget", 10, 5);
        let llm = MockLlm::new();
        let config = HierarchyConfig {
            cluster_target_size: 10,
            llm_budget_per_cycle: 2,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg.clone(), llm.clone()).with_config(config);
        let stats = builder.run_for_agent("agent-budget").await;
        // Either we got the budget-exhausted reason or we hit it via
        // the cluster loop — assert the budget was respected.
        assert!(
            stats.llm_calls <= 5,
            "must not exceed soft budget by more than the in-flight loop ({})",
            stats.llm_calls
        );
    }

    #[tokio::test]
    async fn nonzero_interval_throttles_second_call() {
        let (kg, _dir) = build_store_with_layer_zero("agent-throttle", 10, 3);
        let llm = MockLlm::new();
        let config = HierarchyConfig {
            cluster_target_size: 10,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg, llm.clone())
            .with_config(config)
            .with_interval(Duration::from_secs(3600));

        // First call runs.
        let first = builder.run_for_agent("agent-throttle").await;
        assert!(first.aggregates_created > 0, "first call must run");
        let first_calls = llm.synth_call_count();

        // Second call is throttled — same hour. No new LLM calls.
        let second = builder.run_for_agent("agent-throttle").await;
        assert_eq!(second.aggregates_created, 0, "second call throttled");
        assert_eq!(second.layers_built, 0);
        assert_eq!(
            llm.synth_call_count(),
            first_calls,
            "LLM must not be invoked while throttled"
        );
    }

    #[tokio::test]
    async fn embedding_client_when_present_produces_indexed_aggregates() {
        let (kg, _dir) = build_store_with_layer_zero("agent-emb", 10, 3);
        let llm = MockLlm::new();
        let embedder: Arc<dyn EmbeddingClient> = Arc::new(MockEmbedder);
        let config = HierarchyConfig {
            cluster_target_size: 10,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg.clone(), llm.clone())
            .with_embedding_client(Some(embedder))
            .with_config(config);
        let stats = builder.run_for_agent("agent-emb").await;
        assert!(stats.aggregates_created >= 2);

        // Each aggregate must be visible at layer 1 with an embedding row.
        let layer1 = kg
            .list_entities_with_embeddings_at_layer("agent-emb", 1, 0)
            .await
            .unwrap();
        assert_eq!(
            layer1.len() as u64,
            stats.aggregates_created,
            "every multi-member aggregate should have an embedding row"
        );
    }

    /// Visible-output demo: seed 48 entities in 4 blobs, run the
    /// hierarchy builder, and print the resulting graph state so a
    /// human can eyeball "this is what the agent's memory looked like
    /// before and after." Run with:
    ///
    /// ```text
    /// cargo test -p gateway-memory --lib \
    ///     sleep::hierarchy_builder::tests::demo -- --nocapture
    /// ```
    ///
    /// Not asserting beyond what the other tests already cover — the
    /// goal here is human-readable stdout.
    #[tokio::test]
    async fn demo_prints_resulting_graph_state() {
        let agent = "agent-demo";
        // 4 blobs × 12 members = 48 layer-0 entities. Target 12 → k=4.
        let (kg, _dir) = build_store_with_layer_zero(agent, 12, 4);
        let llm = MockLlm::new();
        let embedder: Arc<dyn EmbeddingClient> = Arc::new(MockEmbedder);
        let config = HierarchyConfig {
            cluster_target_size: 12,
            max_layers: 3,
            inter_cluster_relation_threshold: 0,
            ..Default::default()
        };
        let builder = HierarchyBuilder::new(kg.clone(), llm.clone())
            .with_embedding_client(Some(embedder))
            .with_config(config);

        println!("\n========================================");
        println!("  H-3 HIERARCHY BUILDER — DEMO RUN");
        println!("========================================");
        println!("Seeded 48 entities across 4 tight blobs at layer 0.");
        println!("Target cluster size: 12 → expect k=4 clusters.\n");

        let stats = builder.run_for_agent(agent).await;

        println!("--- HierarchyStats ---");
        println!("  layers_built:                     {}", stats.layers_built);
        println!(
            "  aggregates_created (multi-member): {}",
            stats.aggregates_created
        );
        println!(
            "  singletons_promoted:              {}",
            stats.singletons_promoted
        );
        println!(
            "  inter_cluster_relations_created:  {}",
            stats.inter_cluster_relations_created
        );
        println!("  llm_calls:                        {}", stats.llm_calls);
        println!(
            "  stopped_reason:                   {:?}",
            stats.stopped_reason
        );
        println!("  errors:                           {}\n", stats.errors);

        // Print entity counts per layer.
        println!("--- Entity counts per layer ---");
        for layer in 0..=(stats.layers_built as i64) {
            let rows = kg
                .list_entities_with_embeddings_at_layer(agent, layer, 0)
                .await
                .unwrap();
            println!("  layer {layer}: {} entities", rows.len());
            for (i, e) in rows.iter().take(4).enumerate() {
                println!("      [{}] {}", i, e.id.0);
            }
            if rows.len() > 4 {
                println!("      ... and {} more", rows.len() - 4);
            }
        }

        println!("\nDemo run complete. Stats reflect what got written to");
        println!("kg_entities (layer + parent_cluster_id) and kg_relationships");
        println!("(layer + is_inter_cluster=1). All in a tempfile DB —");
        println!("nothing persisted outside the test run.\n");
    }
}
