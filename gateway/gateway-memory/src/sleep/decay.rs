//! DecayEngine — surfaces prune candidates from the knowledge graph.
//!
//! Phase 4 uses a lightweight orphan + age heuristic rather than full-graph
//! decay math: an entity is a candidate when it has no relationships and
//! its `last_seen_at` is older than `min_age_days`. Archival and
//! already-compressed entities are excluded by the underlying query.

use std::sync::Arc;

use zero_stores::KnowledgeGraphStore;

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

/// Counts returned by [`DecayEngine::decay_kg_confidence`].
#[derive(Debug, Default, Clone)]
pub struct KgDecayStats {
    pub entities_decayed: u64,
    pub relationships_decayed: u64,
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
///
/// Phase D3: trait-routed. Calls `kg_store.list_orphan_old_candidates`
/// which works on both backends — SQLite uses the orphan-age JOIN
/// against `kg_relationships`, Surreal uses a subquery against the
/// `relationship` edge table with server-side date arithmetic.
pub struct DecayEngine {
    kg_store: Arc<dyn KnowledgeGraphStore>,
    config: DecayConfig,
}

impl DecayEngine {
    pub fn new(kg_store: Arc<dyn KnowledgeGraphStore>, config: DecayConfig) -> Self {
        Self { kg_store, config }
    }

    /// Apply temporal confidence decay to KG entities and relationships.
    /// Conservative: errors are logged and the cycle returns whatever stats
    /// were collected before the failure.
    pub async fn decay_kg_confidence(
        &self,
        agent_id: &str,
        config: &crate::KgDecayConfig,
    ) -> KgDecayStats {
        let mut stats = KgDecayStats::default();
        if !config.enabled {
            return stats;
        }
        match self
            .kg_store
            .decay_entity_confidence(
                agent_id,
                config.entity_half_life_days,
                config.min_confidence,
                config.skip_recent_hours,
            )
            .await
        {
            Ok(n) => stats.entities_decayed = n,
            Err(e) => tracing::warn!(error = %e, "decay_entity_confidence failed"),
        }
        match self
            .kg_store
            .decay_relationship_confidence(
                agent_id,
                config.relationship_half_life_days,
                config.min_confidence,
                config.skip_recent_hours,
            )
            .await
        {
            Ok(n) => stats.relationships_decayed = n,
            Err(e) => tracing::warn!(error = %e, "decay_relationship_confidence failed"),
        }
        stats
    }

    /// Return prune candidates for `agent_id`. On query failure, returns an
    /// empty vec (the sleep worker treats decay as best-effort).
    pub async fn list_prune_candidates(&self, agent_id: &str) -> Vec<PruneCandidate> {
        match self
            .kg_store
            .list_orphan_old_candidates(agent_id, self.config.min_age_days, self.config.limit)
            .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|c| PruneCandidate {
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
    use zero_stores::KnowledgeGraphStore;
    use zero_stores_sqlite::kg::storage::GraphStorage;
    use zero_stores_sqlite::{KnowledgeDatabase, SqliteKgStore};

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

        let kg_store: Arc<dyn KnowledgeGraphStore> = Arc::new(SqliteKgStore::new(graph.clone()));
        let engine = DecayEngine::new(
            kg_store,
            DecayConfig {
                min_age_days: 30,
                limit: 100,
            },
        );
        let candidates = engine.list_prune_candidates(agent_id).await;

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

    #[tokio::test]
    async fn decay_kg_confidence_returns_stats_when_enabled() {
        let (_tmp, graph) = setup();
        let agent_id = "agent-kg-decay";

        // Seed one old entity.
        graph
            .knowledge_db()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO kg_entities
                        (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                         epistemic_class, confidence, mention_count, access_count,
                         first_seen_at, last_seen_at)
                     VALUES ('old-1', ?1, 'Concept', 'Old', 'old', 'h1', 'current',
                             0.8, 1, 0, ?2, ?2)",
                    rusqlite::params![
                        agent_id,
                        (chrono::Utc::now() - chrono::Duration::days(180)).to_rfc3339()
                    ],
                )?;
                Ok(())
            })
            .unwrap();

        let kg_store: Arc<dyn KnowledgeGraphStore> =
            Arc::new(zero_stores_sqlite::SqliteKgStore::new(graph.clone()));
        let engine = DecayEngine::new(kg_store, DecayConfig::default());
        let config = crate::KgDecayConfig::default();
        let stats = engine.decay_kg_confidence(agent_id, &config).await;
        assert_eq!(stats.entities_decayed, 1);
        assert_eq!(stats.relationships_decayed, 0);
    }

    #[tokio::test]
    async fn decay_kg_confidence_no_op_when_disabled() {
        let (_tmp, graph) = setup();
        let kg_store: Arc<dyn KnowledgeGraphStore> =
            Arc::new(zero_stores_sqlite::SqliteKgStore::new(graph));
        let engine = DecayEngine::new(kg_store, DecayConfig::default());
        let config = crate::KgDecayConfig {
            enabled: false,
            ..Default::default()
        };
        let stats = engine.decay_kg_confidence("any", &config).await;
        assert_eq!(stats.entities_decayed, 0);
        assert_eq!(stats.relationships_decayed, 0);
    }
}
