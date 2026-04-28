//! Compactor — merges near-duplicate entities (same type, cosine >= threshold).
//!
//! Pipeline per entity type:
//!   find_duplicate_candidates -> optional LLM verify -> merge_entity_into ->
//!   CompactionRepository::record_merge.
//!
//! Archival and already-compressed entities are filtered out at
//! `find_duplicate_candidates`, so no extra guard is needed here.

use async_trait::async_trait;
use std::sync::Arc;

use knowledge_graph::{Entity, EntityType};
use zero_stores_sqlite::kg::storage::GraphStorage;
use zero_stores_sqlite::CompactionRepository;

/// Default cosine threshold for considering two entities near-duplicates.
const DEFAULT_COSINE_THRESHOLD: f32 = 0.92;
/// Default per-type limit on candidate pairs.
const DEFAULT_PER_TYPE_LIMIT: usize = 50;

/// Counts emitted from a single compaction run.
#[derive(Debug, Default, Clone)]
pub struct CompactionStats {
    /// Total candidate pairs considered across all entity types.
    pub candidates_considered: u64,
    /// Merges successfully committed.
    pub merges_performed: u64,
    /// Pairs rejected by the optional LLM verifier.
    pub merges_skipped_by_verifier: u64,
}

/// Optional LLM-based pairwise verifier. Phase 4 leaves this off by default —
/// threshold-only merges are conservative enough for background operation.
#[async_trait]
pub trait PairwiseVerifier: Send + Sync {
    async fn should_merge(&self, a: &Entity, b: &Entity) -> bool;
}

/// Entity types that see the most duplicates and are worth scanning hourly.
const DEFAULT_TYPES: &[EntityType] = &[
    EntityType::Person,
    EntityType::Organization,
    EntityType::Location,
    EntityType::Event,
    EntityType::Concept,
];

/// Threshold-based duplicate merger.
pub struct Compactor {
    graph: Arc<GraphStorage>,
    compaction_repo: Arc<CompactionRepository>,
    verifier: Option<Arc<dyn PairwiseVerifier>>,
    cosine_threshold: f32,
    per_type_limit: usize,
}

impl Compactor {
    /// Build a compactor with default cosine / limit settings.
    pub fn new(
        graph: Arc<GraphStorage>,
        compaction_repo: Arc<CompactionRepository>,
        verifier: Option<Arc<dyn PairwiseVerifier>>,
    ) -> Self {
        Self {
            graph,
            compaction_repo,
            verifier,
            cosine_threshold: DEFAULT_COSINE_THRESHOLD,
            per_type_limit: DEFAULT_PER_TYPE_LIMIT,
        }
    }

    /// Override the cosine threshold (default: 0.92).
    pub fn with_cosine_threshold(mut self, threshold: f32) -> Self {
        self.cosine_threshold = threshold;
        self
    }

    /// Override the per-type candidate pair limit (default: 50).
    pub fn with_per_type_limit(mut self, limit: usize) -> Self {
        self.per_type_limit = limit;
        self
    }

    /// Run one compaction pass for `agent_id`, recording every merge under `run_id`.
    pub async fn run(&self, run_id: &str, agent_id: &str) -> CompactionStats {
        let mut stats = CompactionStats::default();

        for ty in DEFAULT_TYPES {
            self.run_for_type(run_id, agent_id, ty, &mut stats).await;
        }

        stats
    }

    async fn run_for_type(
        &self,
        run_id: &str,
        agent_id: &str,
        ty: &EntityType,
        stats: &mut CompactionStats,
    ) {
        let type_str = ty.as_str();
        let candidates = match self.graph.find_duplicate_candidates(
            agent_id,
            type_str,
            self.cosine_threshold,
            self.per_type_limit,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(%type_str, error = %e, "find_duplicate_candidates failed");
                return;
            }
        };

        if candidates.is_empty() {
            return;
        }

        // Load entities once per type; pick_loser_winner and verifier both
        // need them. The agent_id filter already restricts the set.
        let entities = self.graph.get_entities(agent_id).unwrap_or_default();

        for (a_id, b_id, cosine) in candidates {
            stats.candidates_considered += 1;
            self.process_pair(run_id, &entities, a_id, b_id, cosine, stats)
                .await;
        }
    }

    async fn process_pair(
        &self,
        run_id: &str,
        entities: &[Entity],
        a_id: String,
        b_id: String,
        cosine: f32,
        stats: &mut CompactionStats,
    ) {
        if let Some(verifier) = &self.verifier {
            let a = entities.iter().find(|e| e.id == a_id);
            let b = entities.iter().find(|e| e.id == b_id);
            if let (Some(a), Some(b)) = (a, b) {
                if !verifier.should_merge(a, b).await {
                    stats.merges_skipped_by_verifier += 1;
                    return;
                }
            }
        }

        let (loser, winner) = pick_loser_winner(entities, &a_id, &b_id);

        match self.graph.merge_entity_into(&loser, &winner) {
            Ok(_result) => {
                stats.merges_performed += 1;
                let reason = format!("cosine={cosine:.2}");
                if let Err(e) = self
                    .compaction_repo
                    .record_merge(run_id, &loser, &winner, &reason)
                {
                    tracing::warn!(
                        loser = %loser,
                        winner = %winner,
                        error = %e,
                        "record_merge failed"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    loser = %loser,
                    winner = %winner,
                    error = %e,
                    "merge_entity_into failed"
                );
            }
        }
    }
}

/// Loser = entity with smaller mention_count. Ties resolved by picking `b`
/// as the loser (arbitrary but deterministic).
fn pick_loser_winner(entities: &[Entity], a: &str, b: &str) -> (String, String) {
    let a_mentions = entities
        .iter()
        .find(|e| e.id == a)
        .map(|e| e.mention_count)
        .unwrap_or(0);
    let b_mentions = entities
        .iter()
        .find(|e| e.id == b)
        .map(|e| e.mention_count)
        .unwrap_or(0);
    if a_mentions < b_mentions {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use knowledge_graph::{Entity, EntityType, ExtractedKnowledge};
    use std::sync::Arc;
    use zero_stores_sqlite::kg::storage::GraphStorage;
    use zero_stores_sqlite::KnowledgeDatabase;

    fn setup() -> (
        tempfile::TempDir,
        Arc<GraphStorage>,
        Arc<CompactionRepository>,
    ) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let graph = Arc::new(GraphStorage::new(db.clone()).expect("graph"));
        let repo = Arc::new(CompactionRepository::new(db));
        (tmp, graph, repo)
    }

    /// Two entities with identical L2-normalized embeddings -> cosine 1.0,
    /// so they cross any reasonable threshold.
    fn unit_embedding(seed: f32) -> Vec<f32> {
        // kg_name_index uses 384-dim embeddings (matches production encoder).
        let raw: Vec<f32> = (0..384).map(|i| seed + (i as f32) * 0.001).collect();
        let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
        raw.iter().map(|x| x / norm).collect()
    }

    #[tokio::test]
    async fn compactor_merges_near_duplicates_and_records_audit() {
        let (_tmp, graph, repo) = setup();
        let agent_id = "agent-compact";

        // Two persons with close-but-not-identical embeddings. The resolver
        // dedups at cosine >= 0.87 on store; we stay below that (~0.75) so both
        // entities land, then run the compactor with a lower threshold.
        let emb_a = unit_embedding(0.1);
        let emb_b = unit_embedding(0.9);
        let mut a = Entity::new(
            agent_id.to_string(),
            EntityType::Person,
            "Alice Smith".to_string(),
        );
        a.name_embedding = Some(emb_a);
        a.mention_count = 1;
        let mut b = Entity::new(
            agent_id.to_string(),
            EntityType::Person,
            "Alice S.".to_string(),
        );
        b.name_embedding = Some(emb_b);
        b.mention_count = 5;

        graph
            .store_knowledge(
                agent_id,
                ExtractedKnowledge {
                    entities: vec![a, b],
                    relationships: vec![],
                },
            )
            .expect("store");

        let compactor = Compactor::new(graph.clone(), repo.clone(), None)
            .with_cosine_threshold(0.5)
            .with_per_type_limit(10);

        let run_id = "run-compact-test";
        let stats = compactor.run(run_id, agent_id).await;

        assert!(
            stats.candidates_considered >= 1,
            "expected at least one candidate, got {stats:?}"
        );
        assert!(
            stats.merges_performed >= 1,
            "expected at least one merge, got {stats:?}"
        );

        let rows = repo.list_run(run_id).expect("list run");
        assert!(!rows.is_empty(), "kg_compactions should have a merge row");
        assert_eq!(rows[0].operation, "merge");
    }
}
