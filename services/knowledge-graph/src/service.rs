//! # Graph Service Layer
//!
//! Business logic layer providing higher-level operations on the knowledge graph.

use crate::error::GraphResult;
use crate::storage::GraphStorage;
use crate::types::{Direction, Entity, EntityWithConnections, GraphStats, Relationship, Subgraph};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Graph service providing high-level operations
pub struct GraphService {
    storage: Arc<GraphStorage>,
}

impl GraphService {
    /// Create a new graph service
    pub fn new(storage: Arc<GraphStorage>) -> Self {
        Self { storage }
    }

    /// Get graph statistics for an agent
    pub async fn get_stats(&self, agent_id: &str) -> GraphResult<GraphStats> {
        // Get basic counts
        let entity_count = self.storage.count_entities(agent_id).await?;
        let relationship_count = self.storage.count_relationships(agent_id).await?;

        // Get all entities to count by type
        let entities = self.storage.list_entities(agent_id, None, 10000, 0).await?;
        let mut entity_types: HashMap<String, usize> = HashMap::new();
        for entity in &entities {
            *entity_types
                .entry(entity.entity_type.as_str().to_string())
                .or_default() += 1;
        }

        // Get all relationships to count by type and find most connected entities
        let relationships = self
            .storage
            .list_relationships(agent_id, None, 10000, 0)
            .await?;
        let mut relationship_types: HashMap<String, usize> = HashMap::new();
        let mut entity_connections: HashMap<String, usize> = HashMap::new();

        for rel in &relationships {
            *relationship_types
                .entry(rel.relationship_type.as_str().to_string())
                .or_default() += 1;
            *entity_connections
                .entry(rel.source_entity_id.clone())
                .or_default() += 1;
            *entity_connections
                .entry(rel.target_entity_id.clone())
                .or_default() += 1;
        }

        // Find most connected entities (top 10)
        let entity_id_to_name: HashMap<&str, &str> = entities
            .iter()
            .map(|e| (e.id.as_str(), e.name.as_str()))
            .collect();

        let mut connection_vec: Vec<(String, usize)> = entity_connections
            .into_iter()
            .filter_map(|(id, count)| {
                entity_id_to_name
                    .get(id.as_str())
                    .map(|name| (name.to_string(), count))
            })
            .collect();
        connection_vec.sort_by(|a, b| b.1.cmp(&a.1));
        connection_vec.truncate(10);

        Ok(GraphStats {
            entity_count,
            relationship_count,
            entity_types,
            relationship_types,
            most_connected_entities: connection_vec,
        })
    }

    /// Get entity with its connections by entity name
    pub async fn get_entity_with_connections(
        &self,
        agent_id: &str,
        entity_name: &str,
    ) -> GraphResult<Option<EntityWithConnections>> {
        // Find the entity by name
        let entity = match self
            .storage
            .get_entity_by_name(agent_id, entity_name)
            .await?
        {
            Some(e) => e,
            None => return Ok(None),
        };

        // Get all neighbors
        let neighbors = self
            .storage
            .get_neighbors(agent_id, &entity.id, Direction::Both, 1000)
            .await?;

        // Separate into incoming and outgoing
        let mut outgoing: Vec<(crate::types::Relationship, Entity)> = Vec::new();
        let mut incoming: Vec<(crate::types::Relationship, Entity)> = Vec::new();

        for neighbor in neighbors {
            match neighbor.direction {
                Direction::Outgoing => outgoing.push((neighbor.relationship, neighbor.entity)),
                Direction::Incoming => incoming.push((neighbor.relationship, neighbor.entity)),
                Direction::Both => {} // Should not happen from get_neighbors
            }
        }

        Ok(Some(EntityWithConnections {
            entity,
            outgoing,
            incoming,
        }))
    }

    /// Search entities by name (fuzzy match using LIKE)
    pub async fn search_entities(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> GraphResult<Vec<Entity>> {
        let mut entities = self.storage.search_entities(agent_id, query).await?;
        entities.truncate(limit);
        Ok(entities)
    }

    /// Get subgraph (entities within N hops of a center entity)
    pub async fn get_subgraph(
        &self,
        agent_id: &str,
        center_entity_id: &str,
        max_hops: usize,
    ) -> GraphResult<Subgraph> {
        let mut visited_entities: HashSet<String> = HashSet::new();
        let mut visited_relationships: HashSet<String> = HashSet::new();
        let mut entities: Vec<Entity> = Vec::new();
        let mut relationships: Vec<crate::types::Relationship> = Vec::new();

        // BFS traversal
        let mut current_hop: Vec<String> = vec![center_entity_id.to_string()];
        visited_entities.insert(center_entity_id.to_string());

        for _hop in 0..max_hops {
            if current_hop.is_empty() {
                break;
            }

            let mut next_hop: Vec<String> = Vec::new();

            for entity_id in &current_hop {
                let neighbors = self
                    .storage
                    .get_neighbors(agent_id, entity_id, Direction::Both, 1000)
                    .await?;
                collect_neighbors(neighbors, &mut visited_entities, &mut visited_relationships, &mut entities, &mut relationships, &mut next_hop);
            }

            current_hop = next_hop;
        }

        // Get the center entity itself
        let center_entities = self.storage.list_entities(agent_id, None, 10000, 0).await?;
        if let Some(center) = center_entities
            .into_iter()
            .find(|e| e.id == center_entity_id)
        {
            // Insert at the beginning
            entities.insert(0, center);
        }

        Ok(Subgraph {
            entities,
            relationships,
            center: center_entity_id.to_string(),
            max_hops,
        })
    }

    /// Get entity by ID
    pub async fn get_entity_by_id(
        &self,
        agent_id: &str,
        entity_id: &str,
    ) -> GraphResult<Option<Entity>> {
        let entities = self.storage.list_entities(agent_id, None, 10000, 0).await?;
        Ok(entities.into_iter().find(|e| e.id == entity_id))
    }

    /// List entities for an agent with optional filters
    pub async fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> GraphResult<Vec<Entity>> {
        self.storage
            .list_entities(agent_id, entity_type, limit, offset)
            .await
    }

    /// List relationships for an agent with optional filters
    pub async fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> GraphResult<Vec<crate::types::Relationship>> {
        self.storage
            .list_relationships(agent_id, relationship_type, limit, offset)
            .await
    }

    /// Get neighbors of an entity (1-hop)
    pub async fn get_neighbors(
        &self,
        agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> GraphResult<Vec<crate::types::NeighborInfo>> {
        self.storage
            .get_neighbors(agent_id, entity_id, direction, limit)
            .await
    }

    /// Count all entities across all agents.
    pub async fn count_all_entities(&self) -> GraphResult<usize> {
        self.storage.count_all_entities().await
    }

    /// Count all relationships across all agents.
    pub async fn count_all_relationships(&self) -> GraphResult<usize> {
        self.storage.count_all_relationships().await
    }

    /// List entities across all agents with optional filters.
    pub async fn list_all_entities(
        &self,
        ward_id: Option<&str>,
        entity_type: Option<&str>,
        limit: usize,
    ) -> GraphResult<Vec<Entity>> {
        self.storage
            .list_all_entities(ward_id, entity_type, limit)
            .await
    }

    /// List all relationships across all agents.
    pub async fn list_all_relationships(&self, limit: usize) -> GraphResult<Vec<Relationship>> {
        self.storage.list_all_relationships(limit).await
    }
}

/// Process a list of neighbors into the BFS accumulators.
fn collect_neighbors(
    neighbors: Vec<crate::types::NeighborInfo>,
    visited_entities: &mut HashSet<String>,
    visited_relationships: &mut HashSet<String>,
    entities: &mut Vec<Entity>,
    relationships: &mut Vec<Relationship>,
    next_hop: &mut Vec<String>,
) {
    for neighbor in neighbors {
        if !visited_relationships.contains(&neighbor.relationship.id) {
            visited_relationships.insert(neighbor.relationship.id.clone());
            relationships.push(neighbor.relationship);
        }
        let neighbor_id = neighbor.entity.id.clone();
        if !visited_entities.contains(&neighbor_id) {
            visited_entities.insert(neighbor_id.clone());
            entities.push(neighbor.entity);
            next_hop.push(neighbor_id);
        }
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Entity, EntityType, ExtractedKnowledge, Relationship, RelationshipType};
    use tempfile::tempdir;

    async fn create_test_service() -> GraphService {
        let dir = tempdir().unwrap();
        let db_path = dir.keep().join("test.db");
        let storage = Arc::new(GraphStorage::new(db_path).unwrap());
        GraphService::new(storage)
    }

    async fn populate_test_graph(service: &GraphService) -> (Entity, Entity, Entity) {
        // Create a small graph: Alice -> uses -> Rust -> configured_in -> ProjectX
        let alice = Entity::new(
            "agent1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );
        let rust = Entity::new("agent1".to_string(), EntityType::Tool, "Rust".to_string());
        let project = Entity::new(
            "agent1".to_string(),
            EntityType::Project,
            "ProjectX".to_string(),
        );

        let alice_uses_rust = Relationship::new(
            "agent1".to_string(),
            alice.id.clone(),
            rust.id.clone(),
            RelationshipType::Uses,
        );
        let rust_in_project = Relationship::new(
            "agent1".to_string(),
            rust.id.clone(),
            project.id.clone(),
            RelationshipType::PartOf,
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![alice.clone(), rust.clone(), project.clone()],
            relationships: vec![alice_uses_rust, rust_in_project],
        };

        service
            .storage
            .store_knowledge("agent1", knowledge)
            .await
            .unwrap();

        (alice, rust, project)
    }

    #[tokio::test]
    async fn test_get_stats() {
        let service = create_test_service().await;
        populate_test_graph(&service).await;

        let stats = service.get_stats("agent1").await.unwrap();

        assert_eq!(stats.entity_count, 3);
        assert_eq!(stats.relationship_count, 2);
        assert!(stats.entity_types.contains_key("person"));
        assert!(stats.entity_types.contains_key("tool"));
        assert!(stats.entity_types.contains_key("project"));
        assert!(stats.relationship_types.contains_key("uses"));
        assert!(stats.relationship_types.contains_key("part_of"));
    }

    #[tokio::test]
    async fn test_get_entity_with_connections() {
        let service = create_test_service().await;
        let (_alice, _rust, _project) = populate_test_graph(&service).await;

        // Get Alice's connections
        let result = service
            .get_entity_with_connections("agent1", "Alice")
            .await
            .unwrap();
        assert!(result.is_some());

        let connections = result.unwrap();
        assert_eq!(connections.entity.name, "Alice");
        assert_eq!(connections.outgoing.len(), 1); // Alice -> Rust
        assert_eq!(connections.incoming.len(), 0);

        // Get Rust's connections (has both incoming and outgoing)
        let result = service
            .get_entity_with_connections("agent1", "Rust")
            .await
            .unwrap();
        let connections = result.unwrap();
        assert_eq!(connections.entity.name, "Rust");
        assert_eq!(connections.incoming.len(), 1); // Alice -> Rust
        assert_eq!(connections.outgoing.len(), 1); // Rust -> ProjectX

        // Non-existent entity
        let result = service
            .get_entity_with_connections("agent1", "NonExistent")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_search_entities() {
        let service = create_test_service().await;
        populate_test_graph(&service).await;

        // Search for "ali" should find Alice
        let results = service.search_entities("agent1", "ali", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice");

        // Search for "pro" should find ProjectX
        let results = service.search_entities("agent1", "pro", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ProjectX");
    }

    #[tokio::test]
    async fn test_get_subgraph() {
        let service = create_test_service().await;
        let (alice, _rust, _project) = populate_test_graph(&service).await;

        // Get subgraph starting from Alice with 2 hops
        let subgraph = service.get_subgraph("agent1", &alice.id, 2).await.unwrap();

        assert_eq!(subgraph.center, alice.id);
        assert_eq!(subgraph.max_hops, 2);
        assert_eq!(subgraph.entities.len(), 3); // Alice, Rust, ProjectX
        assert_eq!(subgraph.relationships.len(), 2);

        // Get subgraph with only 1 hop
        let subgraph = service.get_subgraph("agent1", &alice.id, 1).await.unwrap();
        assert_eq!(subgraph.entities.len(), 2); // Alice, Rust (not ProjectX)
        assert_eq!(subgraph.relationships.len(), 1);
    }

    #[tokio::test]
    async fn test_list_entities_with_type_filter() {
        let service = create_test_service().await;
        populate_test_graph(&service).await;

        // List only persons
        let entities = service
            .list_entities("agent1", Some("person"), 10, 0)
            .await
            .unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Alice");

        // List only tools
        let entities = service
            .list_entities("agent1", Some("tool"), 10, 0)
            .await
            .unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Rust");
    }

    #[tokio::test]
    async fn test_list_relationships_with_type_filter() {
        let service = create_test_service().await;
        populate_test_graph(&service).await;

        // List only "uses" relationships
        let rels = service
            .list_relationships("agent1", Some("uses"), 10, 0)
            .await
            .unwrap();
        assert_eq!(rels.len(), 1);
        assert!(matches!(rels[0].relationship_type, RelationshipType::Uses));

        // List only "part_of" relationships
        let rels = service
            .list_relationships("agent1", Some("part_of"), 10, 0)
            .await
            .unwrap();
        assert_eq!(rels.len(), 1);
        assert!(matches!(
            rels[0].relationship_type,
            RelationshipType::PartOf
        ));
    }
}
