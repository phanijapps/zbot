//! Pruner — soft-deletes orphan candidates via the `__pruned__` sentinel on
//! `kg_entities.compressed_into`, then records each prune in `kg_compactions`.
//!
//! Keeping the row (rather than hard-deleting) preserves referential
//! integrity with episodes and distillations that may still point at the
//! entity id. The Archiver can hard-delete later.

use std::sync::Arc;

use gateway_database::CompactionRepository;
use knowledge_graph::GraphStorage;

use crate::sleep::decay::PruneCandidate;

/// Counts emitted from a single prune pass.
#[derive(Debug, Default, Clone)]
pub struct PruneStats {
    pub pruned: u64,
    pub failed: u64,
}

/// Soft-deletes the candidates produced by `DecayEngine`.
pub struct Pruner {
    graph: Arc<GraphStorage>,
    compaction_repo: Arc<CompactionRepository>,
}

impl Pruner {
    pub fn new(graph: Arc<GraphStorage>, compaction_repo: Arc<CompactionRepository>) -> Self {
        Self {
            graph,
            compaction_repo,
        }
    }

    /// Soft-delete every candidate and log each outcome under `run_id`.
    pub fn prune(&self, run_id: &str, candidates: &[PruneCandidate]) -> PruneStats {
        let mut stats = PruneStats::default();
        for c in candidates {
            match self.graph.mark_pruned(&c.entity_id) {
                Ok(()) => {
                    stats.pruned += 1;
                    if let Err(e) =
                        self.compaction_repo
                            .record_prune(run_id, &c.entity_id, &c.reason)
                    {
                        tracing::warn!(
                            entity = %c.entity_id,
                            error = %e,
                            "record_prune failed"
                        );
                    }
                }
                Err(e) => {
                    stats.failed += 1;
                    tracing::warn!(entity = %c.entity_id, error = %e, "mark_pruned failed");
                }
            }
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sleep::decay::{DecayConfig, DecayEngine};
    use gateway_database::{CompactionRepository, KnowledgeDatabase};
    use gateway_services::VaultPaths;
    use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, GraphStorage};
    use std::sync::Arc;

    fn setup() -> (
        tempfile::TempDir,
        Arc<KnowledgeDatabase>,
        Arc<GraphStorage>,
        Arc<CompactionRepository>,
    ) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let graph = Arc::new(GraphStorage::new(db.clone()).expect("graph"));
        let repo = Arc::new(CompactionRepository::new(db.clone()));
        (tmp, db, graph, repo)
    }

    #[tokio::test]
    async fn pruner_soft_deletes_orphan_and_records_audit() {
        let (_tmp, db, graph, repo) = setup();
        let agent_id = "agent-prune";

        // One orphan, old entity.
        let mut orphan = Entity::new(
            agent_id.to_string(),
            EntityType::Concept,
            "Abandoned Concept".to_string(),
        );
        orphan.last_seen_at = chrono::Utc::now() - chrono::Duration::days(90);
        orphan.first_seen_at = orphan.last_seen_at;
        let orphan_id = orphan.id.clone();

        graph
            .store_knowledge(
                agent_id,
                ExtractedKnowledge {
                    entities: vec![orphan],
                    relationships: vec![],
                },
            )
            .expect("store");

        let engine = DecayEngine::new(
            graph.clone(),
            DecayConfig {
                min_age_days: 30,
                limit: 100,
            },
        );
        let candidates = engine.list_prune_candidates(agent_id);
        assert!(
            !candidates.is_empty(),
            "decay engine must produce a candidate"
        );

        let pruner = Pruner::new(graph.clone(), repo.clone());
        let run_id = "run-prune-test";
        let stats = pruner.prune(run_id, &candidates);

        assert!(stats.pruned >= 1, "expected prunes, got {stats:?}");
        assert_eq!(stats.failed, 0);

        // Verify compressed_into sentinel.
        let sentinel: Option<String> = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT compressed_into FROM kg_entities WHERE id = ?1",
                    rusqlite::params![&orphan_id],
                    |r| r.get::<_, Option<String>>(0),
                )
            })
            .expect("query");
        assert_eq!(sentinel.as_deref(), Some("__pruned__"));

        // Verify audit row.
        let rows = repo.list_run(run_id).expect("list run");
        assert!(!rows.is_empty(), "kg_compactions should have a prune row");
        assert_eq!(rows[0].operation, "prune");
        assert_eq!(rows[0].entity_id.as_deref(), Some(orphan_id.as_str()));
    }
}
