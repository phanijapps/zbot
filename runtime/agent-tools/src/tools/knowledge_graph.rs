// ============================================================================
// KNOWLEDGE GRAPH TOOLS
// Tools for agents to access and manipulate their knowledge graph
// ============================================================================

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{Tool, ToolContext, Result};

// ============================================================================
// LIST ENTITIES TOOL
// ============================================================================

/// Tool for listing all entities in the agent's knowledge graph
pub struct ListEntitiesTool;

#[async_trait]
impl Tool for ListEntitiesTool {
    fn name(&self) -> &str {
        "list_entities"
    }

    fn description(&self) -> &str {
        "List all entities in your knowledge graph. Optionally filter by entity type (person, organization, location, concept, tool, project)."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "entity_type": {
                    "type": "string",
                    "description": "Optional entity type to filter by (person, organization, location, concept, tool, project)",
                    "enum": ["person", "organization", "location", "concept", "tool", "project"]
                }
            }
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Get agent_id from context
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing agent_id in context".to_string()))?;

        let entity_type_filter = args.get("entity_type")
            .and_then(|v| v.as_str());


        // Get entities from knowledge graph storage
        let storage = get_graph_storage(ctx.clone())?;
        let entities = storage.get_entities(&agent_id).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to get entities: {}", e)))?;

        // Filter by entity type if specified
        let filtered_entities: Vec<_> = if let Some(filter) = entity_type_filter {
            entities.into_iter()
                .filter(|e| e.entity_type.as_str() == filter)
                .collect()
        } else {
            entities
        };

        // Format results
        let results: Vec<Value> = filtered_entities.into_iter().map(|e| {
            json!({
                "id": e.id,
                "name": e.name,
                "type": e.entity_type.as_str(),
                "mention_count": e.mention_count,
                "first_seen_at": e.first_seen_at.to_rfc3339(),
                "last_seen_at": e.last_seen_at.to_rfc3339(),
                "properties": e.properties,
            })
        }).collect();

        Ok(json!({
            "entities": results,
            "total": results.len(),
        }))
    }
}

// ============================================================================
// SEARCH ENTITIES TOOL
// ============================================================================

/// Tool for searching entities by name
pub struct SearchEntitiesTool;

#[async_trait]
impl Tool for SearchEntitiesTool {
    fn name(&self) -> &str {
        "search_entities"
    }

    fn description(&self) -> &str {
        "Search for entities in your knowledge graph by name. Returns entities matching the search query."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query for entity name (partial match)"
                }
            },
            "required": ["query"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'query' parameter".to_string()))?;

        // Get agent_id from context
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing agent_id in context".to_string()))?;


        // Search entities
        let storage = get_graph_storage(ctx.clone())?;
        let entities = storage.search_entities(&agent_id, query).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to search entities: {}", e)))?;

        // Format results
        let results: Vec<Value> = entities.into_iter().map(|e| {
            json!({
                "id": e.id,
                "name": e.name,
                "type": e.entity_type.as_str(),
                "mention_count": e.mention_count,
                "first_seen_at": e.first_seen_at.to_rfc3339(),
                "last_seen_at": e.last_seen_at.to_rfc3339(),
                "properties": e.properties,
            })
        }).collect();

        Ok(json!({
            "entities": results,
            "total": results.len(),
            "query": query,
        }))
    }
}

// ============================================================================
// GET ENTITY RELATIONSHIPS TOOL
// ============================================================================

/// Tool for getting relationships for a specific entity
pub struct GetEntityRelationshipsTool;

#[async_trait]
impl Tool for GetEntityRelationshipsTool {
    fn name(&self) -> &str {
        "get_entity_relationships"
    }

    fn description(&self) -> &str {
        "Get all relationships for a specific entity. Shows how the entity is connected to other entities."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "entity_name": {
                    "type": "string",
                    "description": "Name of the entity to get relationships for"
                }
            },
            "required": ["entity_name"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let entity_name = args.get("entity_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'entity_name' parameter".to_string()))?;

        // Get agent_id from context
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing agent_id in context".to_string()))?;


        // Get all entities and relationships
        let storage = get_graph_storage(ctx.clone())?;
        let entities = storage.get_entities(&agent_id).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to get entities: {}", e)))?;
        let relationships = storage.get_relationships(&agent_id).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to get relationships: {}", e)))?;

        // Find the entity by name
        let entity = entities.iter()
            .find(|e| e.name == entity_name)
            .ok_or_else(|| zero_core::ZeroError::Tool(format!("Entity '{}' not found", entity_name)))?;

        // Filter relationships where this entity is source or target
        let related_relationships: Vec<_> = relationships.iter()
            .filter(|r| r.source_entity_id == entity.id || r.target_entity_id == entity.id)
            .collect();

        // Build results with entity names
        let mut results = Vec::new();
        for rel in related_relationships {
            // Find source and target entity names
            let source_name = entities.iter()
                .find(|e| e.id == rel.source_entity_id)
                .map(|e| e.name.as_str())
                .unwrap_or("Unknown");
            let target_name = entities.iter()
                .find(|e| e.id == rel.target_entity_id)
                .map(|e| e.name.as_str())
                .unwrap_or("Unknown");

            let direction = if rel.source_entity_id == entity.id {
                "outgoing"
            } else {
                "incoming"
            };

            results.push(json!({
                "type": rel.relationship_type.as_str(),
                "direction": direction,
                "source": source_name,
                "target": target_name,
                "mention_count": rel.mention_count,
                "first_seen_at": rel.first_seen_at.to_rfc3339(),
                "last_seen_at": rel.last_seen_at.to_rfc3339(),
                "properties": rel.properties,
            }));
        }

        Ok(json!({
            "entity": entity_name,
            "entity_type": entity.entity_type.as_str(),
            "relationships": results,
            "total": results.len(),
        }))
    }
}

// ============================================================================
// ADD ENTITY TOOL
// ============================================================================

/// Tool for adding a new entity to the knowledge graph
pub struct AddEntityTool;

#[async_trait]
impl Tool for AddEntityTool {
    fn name(&self) -> &str {
        "add_entity"
    }

    fn description(&self) -> &str {
        "Add a new entity to your knowledge graph. Useful for remembering important people, organizations, concepts, etc."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the entity (e.g., 'John Smith', 'Google', 'Rust')"
                },
                "entity_type": {
                    "type": "string",
                    "description": "Type of entity",
                    "enum": ["person", "organization", "location", "concept", "tool", "project"],
                    "default": "concept"
                },
                "properties": {
                    "type": "object",
                    "description": "Optional additional properties as key-value pairs (e.g., {\"role\": \"engineer\", \"email\": \"john@example.com\"})"
                }
            },
            "required": ["name"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'name' parameter".to_string()))?;

        let entity_type_str = args.get("entity_type")
            .and_then(|v| v.as_str())
            .unwrap_or("concept");

        let properties_obj = args.get("properties")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        // Get agent_id from context
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing agent_id in context".to_string()))?;


        // Create entity
        let entity_type = knowledge_graph::types::EntityType::from_str(entity_type_str);
        let mut entity = knowledge_graph::types::Entity::new(agent_id.clone(), entity_type, name.to_string());

        // Add properties
        for (key, value) in properties_obj {
            entity.properties.insert(key, value);
        }

        // Store entity
        let storage = get_graph_storage(ctx.clone())?;
        storage.store_knowledge(&agent_id, knowledge_graph::types::ExtractedKnowledge {
            entities: vec![entity.clone()],
            relationships: vec![],
        }).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to store entity: {}", e)))?;

        Ok(json!({
            "id": entity.id,
            "name": entity.name,
            "type": entity.entity_type.as_str(),
            "message": "Entity added successfully",
        }))
    }
}

// ============================================================================
// ADD RELATIONSHIP TOOL
// ============================================================================

/// Tool for adding a new relationship to the knowledge graph
pub struct AddRelationshipTool;

#[async_trait]
impl Tool for AddRelationshipTool {
    fn name(&self) -> &str {
        "add_relationship"
    }

    fn description(&self) -> &str {
        "Add a relationship between two entities in your knowledge graph. Both entities must already exist."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "Name of the source entity"
                },
                "target": {
                    "type": "string",
                    "description": "Name of the target entity"
                },
                "relationship_type": {
                    "type": "string",
                    "description": "Type of relationship",
                    "enum": ["works_for", "located_in", "related_to", "created", "uses", "part_of", "mentions"],
                    "default": "related_to"
                },
                "properties": {
                    "type": "object",
                    "description": "Optional additional properties as key-value pairs"
                }
            },
            "required": ["source", "target"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let source_name = args.get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'source' parameter".to_string()))?;

        let target_name = args.get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'target' parameter".to_string()))?;

        let rel_type_str = args.get("relationship_type")
            .and_then(|v| v.as_str())
            .unwrap_or("related_to");

        let properties_obj = args.get("properties")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        // Get agent_id from context
        let agent_id = ctx.get_state("app:agent_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing agent_id in context".to_string()))?;


        // Get entities to find their IDs
        let storage = get_graph_storage(ctx.clone())?;
        let entities = storage.get_entities(&agent_id).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to get entities: {}", e)))?;

        let source_entity = entities.iter()
            .find(|e| e.name == source_name)
            .ok_or_else(|| zero_core::ZeroError::Tool(format!("Source entity '{}' not found. Create it first with add_entity.", source_name)))?;

        let target_entity = entities.iter()
            .find(|e| e.name == target_name)
            .ok_or_else(|| zero_core::ZeroError::Tool(format!("Target entity '{}' not found. Create it first with add_entity.", target_name)))?;

        // Create relationship
        let relationship_type = knowledge_graph::types::RelationshipType::from_str(rel_type_str);
        let mut relationship = knowledge_graph::types::Relationship::new(
            agent_id.clone(),
            source_entity.id.clone(),
            target_entity.id.clone(),
            relationship_type,
        );

        // Add properties
        for (key, value) in properties_obj {
            relationship.properties.insert(key, value);
        }

        // Store relationship
        storage.store_knowledge(&agent_id, knowledge_graph::types::ExtractedKnowledge {
            entities: vec![],
            relationships: vec![relationship.clone()],
        }).await
            .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to store relationship: {}", e)))?;

        Ok(json!({
            "id": relationship.id,
            "source": source_name,
            "target": target_name,
            "type": relationship.relationship_type.as_str(),
            "message": "Relationship added successfully",
        }))
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get the knowledge graph storage from context
fn get_graph_storage(ctx: Arc<dyn ToolContext>) -> Result<knowledge_graph::storage::GraphStorage> {
    use std::path::PathBuf;

    // Get db_path from context (set by the runtime)
    let db_path = ctx.get_state("app:db_path")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| zero_core::ZeroError::Tool("Missing db_path in context".to_string()))?;

    let path = PathBuf::from(db_path);
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            knowledge_graph::storage::GraphStorage::new(path)
                .map_err(|e| zero_core::ZeroError::Tool(format!("Failed to create graph storage: {}", e)))
        })
    })
}
