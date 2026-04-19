//! # Graph Storage Adapter
//!
//! Bridges `knowledge_graph::GraphStorage` to `agent_tools::GraphStorageAccess`
//! so the `GraphQueryTool` can query the knowledge graph without depending on
//! the concrete storage crate.

use std::sync::Arc;

use agent_tools::{EntityInfo, GraphStorageAccess, NeighborInfo};
use async_trait::async_trait;
use knowledge_graph::{Direction, Entity, GraphService, GraphStorage, GraphView, Relationship};

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
