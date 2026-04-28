//! DecayEngine — surfaces prune candidates from the knowledge graph.
//!
//! Phase 4 uses a lightweight orphan + age heuristic rather than full-graph
//! decay math: an entity is a candidate when it has no relationships and
//! its `last_seen_at` is older than `min_age_days`. Archival and
//! already-compressed entities are excluded by the underlying query.

use std::sync::Arc;

use zero_stores_sqlite::kg::storage::{GraphStorage, OrphanCandidate};

/// Tuning knobs for the decay pass.
#[derive(Debug, Clone)]
pub struct DecayConfig {
    /// Only consider entities last seen more than this many days ago.
    pub min_age_days: i64,
    /// Upper bound on the number of candidates returned per pass.
    pub limit: usize,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            min_age_days: 30,
            limit: 100,
        }
    }
}

/// A decayed entity slated for soft-deletion by the Pruner.
#[derive(Debug, Clone)]
pub struct PruneCandidate {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    pub reason: String,
}

/// Decay pass over the knowledge graph for a single agent.
pub struct DecayEngine {
    graph: Arc<GraphStorage>,
    config: DecayConfig,
}

impl DecayEngine {
    pub fn new(graph: Arc<GraphStorage>, config: DecayConfig) -> Self {
        Self { graph, config }
    }

    /// Return prune candidates for `agent_id`. On query failure, returns an
    /// empty vec (the sleep worker treats decay as best-effort).
    pub fn list_prune_candidates(&self, agent_id: &str) -> Vec<PruneCandidate> {
        match self.graph.list_orphan_old_candidates(
            agent_id,
            self.config.min_age_days,
            self.config.limit,
        ) {
            Ok(rows) => rows
                .into_iter()
                .map(|c: OrphanCandidate| PruneCandidate {
                    entity_id: c.id,
                    name: c.name,
                    entity_type: c.entity_type,
                    reason: format!(
                        "orphan age>{}d mention_count={}",
                        self.config.min_age_days, c.mention_count
                    ),
                })
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "list_orphan_old_candidates failed");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, Relationship, RelationshipType};
    use std::sync::Arc;
    use zero_stores_sqlite::KnowledgeDatabase;

    fn setup() -> (tempfile::TempDir, Arc<GraphStorage>) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let graph = Arc::new(GraphStorage::new(db).expect("graph"));
        (tmp, graph)
    }

    #[tokio::test]
    async fn decay_engine_returns_only_orphan_old_non_archival() {
        let (_tmp, graph) = setup();
        let agent_id = "agent-decay";

        // 1. An orphan, old entity -> should be returned.
        let mut orphan = Entity::new(
            agent_id.to_string(),
            EntityType::Concept,
            "Stale Topic".to_string(),
        );
        orphan.last_seen_at = chrono::Utc::now() - chrono::Duration::days(90);
        orphan.first_seen_at = orphan.last_seen_at;

        // 2. An entity with relationships -> should NOT be returned.
        let mut connected_a = Entity::new(
            agent_id.to_string(),
            EntityType::Person,
            "Active Alice".to_string(),
        );
        connected_a.last_seen_at = chrono::Utc::now() - chrono::Duration::days(90);
        connected_a.first_seen_at = connected_a.last_seen_at;
        let mut connected_b = Entity::new(
            agent_id.to_string(),
            EntityType::Organization,
            "Active Org".to_string(),
        );
        connected_b.last_seen_at = chrono::Utc::now() - chrono::Duration::days(90);
        connected_b.first_seen_at = connected_b.last_seen_at;

        let rel = Relationship::new(
            agent_id.to_string(),
            connected_a.id.clone(),
            connected_b.id.clone(),
            RelationshipType::WorksFor,
        );

        // 3. A recent orphan -> should NOT be returned (too young).
        let mut recent_orphan = Entity::new(
            agent_id.to_string(),
            EntityType::Concept,
            "Fresh Topic".to_string(),
        );
        recent_orphan.last_seen_at = chrono::Utc::now();
        recent_orphan.first_seen_at = recent_orphan.last_seen_at;

        graph
            .store_knowledge(
                agent_id,
                ExtractedKnowledge {
                    entities: vec![
                        orphan.clone(),
                        connected_a,
                        connected_b,
                        recent_orphan.clone(),
                    ],
                    relationships: vec![rel],
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

        let names: Vec<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"Stale Topic"),
            "expected stale orphan to be returned; got {names:?}"
        );
        assert!(
            !names.contains(&"Active Alice") && !names.contains(&"Active Org"),
            "connected entities should not be returned; got {names:?}"
        );
        assert!(
            !names.contains(&"Fresh Topic"),
            "recent entity should not be returned; got {names:?}"
        );
    }
}
