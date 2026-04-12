//! # Graph Storage Adapter
//!
//! Bridges `knowledge_graph::GraphStorage` to `agent_tools::GraphStorageAccess`
//! so the `GraphQueryTool` can query the knowledge graph without depending on
//! the concrete storage crate.

use std::sync::Arc;

use agent_tools::{EntityInfo, GraphStorageAccess, NeighborInfo};
use async_trait::async_trait;
use knowledge_graph::{Direction, GraphStorage};

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
            .await
            .map_err(|e| format!("graph search failed: {e}"))?;

        let results: Vec<EntityInfo> = entities
            .into_iter()
            .filter(|e| {
                entity_type
                    .map(|t| e.entity_type.as_str() == t)
                    .unwrap_or(true)
            })
            .take(limit)
            .map(|e| EntityInfo {
                name: e.name,
                entity_type: e.entity_type.as_str().to_string(),
                mention_count: e.mention_count,
            })
            .collect();

        Ok(results)
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
            .await
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
            .await
            .map_err(|e| format!("neighbor query failed: {e}"))?;

        Ok(neighbors
            .into_iter()
            .map(|n| {
                let dir_str = match n.direction {
                    Direction::Outgoing => "outgoing",
                    Direction::Incoming => "incoming",
                    Direction::Both => "both",
                };
                NeighborInfo {
                    entity: EntityInfo {
                        name: n.entity.name,
                        entity_type: n.entity.entity_type.as_str().to_string(),
                        mention_count: n.entity.mention_count,
                    },
                    relationship_type: n.relationship.relationship_type.as_str().to_string(),
                    direction: dir_str.to_string(),
                }
            })
            .collect())
    }

    async fn get_entity_by_name(&self, name: &str) -> Result<Option<EntityInfo>, String> {
        let entity = self
            .storage
            .get_entity_by_name(GLOBAL_AGENT_ID, name)
            .await
            .map_err(|e| format!("entity lookup failed: {e}"))?;

        Ok(entity.map(|e| EntityInfo {
            name: e.name,
            entity_type: e.entity_type.as_str().to_string(),
            mention_count: e.mention_count,
        }))
    }
}
