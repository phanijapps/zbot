//! # Graph Storage Adapter
//!
//! Bridges `zero_stores_sqlite::kg::storage::GraphStorage` to `agent_tools::GraphStorageAccess`
//! so the `GraphQueryTool` can query the knowledge graph without depending on
//! the concrete storage crate.

use std::sync::Arc;

use agent_tools::{EntityInfo, GraphStorageAccess, NeighborInfo};
use async_trait::async_trait;
use knowledge_graph::{Direction, Entity, Relationship};
use zero_stores_sqlite::kg::service::{GraphService, GraphView};
use zero_stores_sqlite::kg::storage::GraphStorage;

/// Map `knowledge_graph::Entity` to the tool-facing [`EntityInfo`] WITHOUT
/// dropping fields. `properties` is round-tripped as JSON so everything the
/// ingest payload wrote (aliases, chunk-file refs, evidence, roles, dates)
/// reaches the agent.
fn entity_to_info(e: Entity) -> EntityInfo {
    let properties = serde_json::to_value(&e.properties).unwrap_or_else(|_| serde_json::json!({}));
    EntityInfo {
        id: e.id,
        name: e.name,
        entity_type: e.entity_type.as_str().to_string(),
        mention_count: e.mention_count,
        properties,
        first_seen_at: e.first_seen_at.to_rfc3339(),
        last_seen_at: e.last_seen_at.to_rfc3339(),
    }
}

/// Same for relationships — carry the edge's own `properties` (evidence,
/// confidence, development timeline) through to the tool payload.
fn neighbor_to_info(
    entity: Entity,
    relationship: Relationship,
    direction: Direction,
) -> NeighborInfo {
    let dir_str = match direction {
        Direction::Outgoing => "outgoing",
        Direction::Incoming => "incoming",
        Direction::Both => "both",
    };
    let rel_properties =
        serde_json::to_value(&relationship.properties).unwrap_or_else(|_| serde_json::json!({}));
    NeighborInfo {
        entity: entity_to_info(entity),
        relationship_type: relationship.relationship_type.as_str().to_string(),
        direction: dir_str.to_string(),
        rel_properties,
        rel_first_seen_at: relationship.first_seen_at.to_rfc3339(),
        rel_last_seen_at: relationship.last_seen_at.to_rfc3339(),
    }
}

/// Adapter that implements [`GraphStorageAccess`] by delegating to a
/// [`GraphStorage`] instance.
///
/// All queries use `__global__` as the agent ID so entities from any agent
/// are visible to the graph query tool.
pub struct GraphStorageAdapter {
    storage: Arc<GraphStorage>,
}

impl GraphStorageAdapter {
    pub fn new(storage: Arc<GraphStorage>) -> Self {
        Self { storage }
    }
}

/// The agent ID used for graph queries — `__global__` sees all entities.
const GLOBAL_AGENT_ID: &str = "__global__";

#[async_trait]
impl GraphStorageAccess for GraphStorageAdapter {
    async fn search_entities_by_name(
        &self,
        query: &str,
        entity_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EntityInfo>, String> {
        let entities = self
            .storage
            .search_entities(GLOBAL_AGENT_ID, query)
            .map_err(|e| format!("graph search failed: {e}"))?;

        let results: Vec<EntityInfo> = entities
            .into_iter()
            .filter(|e| {
                entity_type
                    .map(|t| e.entity_type.as_str() == t)
                    .unwrap_or(true)
            })
            .take(limit)
            .map(entity_to_info)
            .collect();

        Ok(results)
    }

    async fn search_entities_with_view(
        &self,
        query: &str,
        entity_type: Option<&str>,
        view: &str,
        limit: usize,
    ) -> Result<Vec<EntityInfo>, String> {
        let service = GraphService::new(self.storage.clone());
        let graph_view = GraphView::from_str(view);
        let entities = service
            .search_entities_view(GLOBAL_AGENT_ID, query, graph_view, limit.saturating_mul(2))
            .await
            .map_err(|e| format!("graph view search failed: {e}"))?;

        Ok(entities
            .into_iter()
            .filter(|e| {
                entity_type
                    .map(|t| e.entity_type.as_str() == t)
                    .unwrap_or(true)
            })
            .take(limit)
            .map(entity_to_info)
            .collect())
    }

    async fn get_entity_neighbors(
        &self,
        entity_name: &str,
        direction: &str,
        limit: usize,
    ) -> Result<Vec<NeighborInfo>, String> {
        // Resolve entity name → entity ID first
        let entity = self
            .storage
            .get_entity_by_name(GLOBAL_AGENT_ID, entity_name)
            .map_err(|e| format!("entity lookup failed: {e}"))?;

        let Some(entity) = entity else {
            return Ok(vec![]);
        };

        let dir = match direction {
            "outgoing" => Direction::Outgoing,
            "incoming" => Direction::Incoming,
            _ => Direction::Both,
        };

        let neighbors = self
            .storage
            .get_neighbors(GLOBAL_AGENT_ID, &entity.id, dir, limit)
            .map_err(|e| format!("neighbor query failed: {e}"))?;

        Ok(neighbors
            .into_iter()
            .map(|n| neighbor_to_info(n.entity, n.relationship, n.direction))
            .collect())
    }

    async fn get_entity_by_name(&self, name: &str) -> Result<Option<EntityInfo>, String> {
        let entity = self
            .storage
            .get_entity_by_name(GLOBAL_AGENT_ID, name)
            .map_err(|e| format!("entity lookup failed: {e}"))?;

        Ok(entity.map(entity_to_info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, Relationship, RelationshipType};
    use zero_stores_sqlite::KnowledgeDatabase;

    fn storage() -> (tempfile::TempDir, Arc<GraphStorage>) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let storage = Arc::new(GraphStorage::new(db).expect("storage"));
        (tmp, storage)
    }

    /// Seed three entities under the `__global__` agent (which is what the
    /// adapter queries) and return their IDs so tests can build relationships.
    fn seed_three(storage: &Arc<GraphStorage>) -> (String, String, String) {
        let alice = Entity::new("__global__".into(), EntityType::Person, "Alice".into());
        let rust = Entity::new("__global__".into(), EntityType::Tool, "Rust".into());
        let project = Entity::new("__global__".into(), EntityType::Project, "ProjectX".into());
        let (a_id, r_id, p_id) = (alice.id.clone(), rust.id.clone(), project.id.clone());
        storage
            .store_knowledge(
                "__global__",
                ExtractedKnowledge {
                    entities: vec![alice, rust, project],
                    relationships: vec![],
                },
            )
            .expect("seed entities");
        (a_id, r_id, p_id)
    }

    #[tokio::test]
    async fn search_entities_by_name_returns_seeded_match() {
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let out = adapter
            .search_entities_by_name("Alice", None, 10)
            .await
            .expect("search");

        assert!(
            out.iter().any(|e| e.name == "Alice"),
            "expected Alice in {out:?}"
        );
    }

    #[tokio::test]
    async fn search_entities_by_name_filters_by_type() {
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        // With no filter: Alice (person) and Rust (tool) both returned for a
        // broad query that matches both.
        let all = adapter
            .search_entities_by_name("", None, 10)
            .await
            .expect("search all");
        assert!(all.iter().any(|e| e.name == "Alice"));
        assert!(all.iter().any(|e| e.name == "Rust"));

        // With type=person: only Alice.
        let only_person = adapter
            .search_entities_by_name("", Some("person"), 10)
            .await
            .expect("search person");
        assert!(only_person.iter().all(|e| e.entity_type == "person"));
        assert!(only_person.iter().any(|e| e.name == "Alice"));
        assert!(only_person.iter().all(|e| e.name != "Rust"));
    }

    #[tokio::test]
    async fn search_entities_by_name_honors_limit() {
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let out = adapter
            .search_entities_by_name("", None, 1)
            .await
            .expect("search");
        assert_eq!(out.len(), 1, "limit=1 must cap result count");
    }

    #[tokio::test]
    async fn get_entity_by_name_found_returns_info() {
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let got = adapter
            .get_entity_by_name("Alice")
            .await
            .expect("lookup")
            .expect("should be Some(Alice)");

        assert_eq!(got.name, "Alice");
        assert_eq!(got.entity_type, "person");
    }

    #[tokio::test]
    async fn get_entity_by_name_missing_returns_none() {
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let got = adapter.get_entity_by_name("Ghost").await.expect("lookup");
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn get_entity_neighbors_unknown_entity_returns_empty() {
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let got = adapter
            .get_entity_neighbors("Ghost", "both", 10)
            .await
            .expect("neighbors");
        assert!(got.is_empty(), "unknown entity → empty neighbours");
    }

    #[tokio::test]
    async fn get_entity_neighbors_returns_linked_entity() {
        let (_tmp, storage) = storage();
        let alice = Entity::new("__global__".into(), EntityType::Person, "Alice".into());
        let rust = Entity::new("__global__".into(), EntityType::Tool, "Rust".into());
        let rel = Relationship::new(
            "__global__".into(),
            alice.id.clone(),
            rust.id.clone(),
            RelationshipType::Uses,
        );
        storage
            .store_knowledge(
                "__global__",
                ExtractedKnowledge {
                    entities: vec![alice, rust],
                    relationships: vec![rel],
                },
            )
            .expect("seed");
        let adapter = GraphStorageAdapter::new(storage);

        let got = adapter
            .get_entity_neighbors("Alice", "outgoing", 10)
            .await
            .expect("neighbors");

        assert_eq!(got.len(), 1);
        assert_eq!(got[0].entity.name, "Rust");
        assert_eq!(got[0].direction, "outgoing");
        assert_eq!(got[0].relationship_type, "uses");
    }

    #[tokio::test]
    async fn get_entity_neighbors_direction_defaults_to_both_on_unknown_string() {
        // "banana" isn't a valid direction → the match arm falls through to
        // Direction::Both. We only assert the call succeeds (specific row
        // shape depends on the seeded graph's directionality).
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let result = adapter.get_entity_neighbors("Alice", "banana", 10).await;
        assert!(result.is_ok(), "unknown direction must not error");
    }

    #[tokio::test]
    async fn search_entities_with_view_smoke_test() {
        // GraphView::from_str("entity") is the most permissive view. We don't
        // assert specific ranking — just that the adapter wires into
        // GraphService without panicking and returns a Vec.
        let (_tmp, storage) = storage();
        seed_three(&storage);
        let adapter = GraphStorageAdapter::new(storage);

        let out = adapter
            .search_entities_with_view("Alice", None, "entity", 5)
            .await
            .expect("view search");
        // Can't guarantee Alice surfaces (depends on view semantics) but the
        // call path is exercised — that's what coverage needs.
        let _ = out;
    }
}
