//! # Knowledge Graph Types
//!
//! Core data structures for entities and relationships.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Entity type classification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// Person (e.g., "John Doe")
    Person,
    /// Organization (e.g., "Google")
    Organization,
    /// Location (e.g., "San Francisco")
    Location,
    /// Concept/Topic (e.g., "machine learning")
    Concept,
    /// Tool/Technology (e.g., "React")
    Tool,
    /// Project (e.g., "Project X")
    Project,
    /// File (e.g., "main.rs", "config.toml")
    File,
    /// Custom entity type
    Custom(String),
}

// Custom serialization to serialize as string instead of {Custom: "value"} format
impl Serialize for EntityType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EntityType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(EntityType::from_str(&s))
    }
}

impl EntityType {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "person" => EntityType::Person,
            "organization" | "org" => EntityType::Organization,
            "location" | "place" => EntityType::Location,
            "concept" | "topic" => EntityType::Concept,
            "tool" | "technology" => EntityType::Tool,
            "project" => EntityType::Project,
            "file" => EntityType::File,
            other => EntityType::Custom(other.to_string()),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            EntityType::Person => "person",
            EntityType::Organization => "organization",
            EntityType::Location => "location",
            EntityType::Concept => "concept",
            EntityType::Tool => "tool",
            EntityType::Project => "project",
            EntityType::File => "file",
            EntityType::Custom(s) => s,
        }
    }
}

/// Relationship type classification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    /// Works for/at
    WorksFor,
    /// Located in
    LocatedIn,
    /// Related to (general)
    RelatedTo,
    /// Created/owns
    Created,
    /// Uses/depends on
    Uses,
    /// Part of
    PartOf,
    /// Mentions
    Mentions,
    /// Custom relationship type
    Custom(String),
}

// Custom serialization to serialize as string instead of {Custom: "value"} format
impl Serialize for RelationshipType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RelationshipType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(RelationshipType::from_str(&s))
    }
}

impl RelationshipType {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().replace('_', "").as_str() {
            "worksfor" => RelationshipType::WorksFor,
            "locatedin" => RelationshipType::LocatedIn,
            "relatedto" => RelationshipType::RelatedTo,
            "created" => RelationshipType::Created,
            "uses" => RelationshipType::Uses,
            "partof" => RelationshipType::PartOf,
            "mentions" => RelationshipType::Mentions,
            other => RelationshipType::Custom(other.to_string()),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            RelationshipType::WorksFor => "works_for",
            RelationshipType::LocatedIn => "located_in",
            RelationshipType::RelatedTo => "related_to",
            RelationshipType::Created => "created",
            RelationshipType::Uses => "uses",
            RelationshipType::PartOf => "part_of",
            RelationshipType::Mentions => "mentions",
            RelationshipType::Custom(s) => s,
        }
    }
}

/// Knowledge graph entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique entity ID
    pub id: String,
    /// Agent ID this entity belongs to
    pub agent_id: String,
    /// Entity type
    pub entity_type: EntityType,
    /// Entity name (e.g., "John", "Google")
    pub name: String,
    /// Additional properties (aliases, descriptions, etc.)
    pub properties: HashMap<String, serde_json::Value>,
    /// First time this entity was seen
    pub first_seen_at: DateTime<Utc>,
    /// Last time this entity was referenced
    pub last_seen_at: DateTime<Utc>,
    /// Number of times this entity appears
    pub mention_count: i64,
}

/// Knowledge graph relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Unique relationship ID
    pub id: String,
    /// Agent ID this relationship belongs to
    pub agent_id: String,
    /// Source entity ID
    pub source_entity_id: String,
    /// Target entity ID
    pub target_entity_id: String,
    /// Relationship type
    pub relationship_type: RelationshipType,
    /// Additional properties (confidence, context, etc.)
    pub properties: HashMap<String, serde_json::Value>,
    /// First time this relationship was seen
    pub first_seen_at: DateTime<Utc>,
    /// Last time this relationship was referenced
    pub last_seen_at: DateTime<Utc>,
    /// Number of times this relationship appears
    pub mention_count: i64,
}

impl Entity {
    /// Create a new entity
    pub fn new(agent_id: String, entity_type: EntityType, name: String) -> Self {
        let now = Utc::now();
        let id = format!("entity_{}_{}", agent_id, uuid::Uuid::new_v4());

        Self {
            id,
            agent_id,
            entity_type,
            name,
            properties: HashMap::new(),
            first_seen_at: now,
            last_seen_at: now,
            mention_count: 1,
        }
    }

    /// Update the last seen time and increment mention count
    pub fn touch(&mut self) {
        self.last_seen_at = Utc::now();
        self.mention_count += 1;
    }

    /// Add a property
    pub fn with_property(mut self, key: String, value: serde_json::Value) -> Self {
        self.properties.insert(key, value);
        self
    }
}

impl Relationship {
    /// Create a new relationship
    pub fn new(
        agent_id: String,
        source_entity_id: String,
        target_entity_id: String,
        relationship_type: RelationshipType,
    ) -> Self {
        let now = Utc::now();
        let id = format!("rel_{}_{}_{}_{}",
            agent_id,
            source_entity_id,
            target_entity_id,
            uuid::Uuid::new_v4()
        );

        Self {
            id,
            agent_id,
            source_entity_id,
            target_entity_id,
            relationship_type,
            properties: HashMap::new(),
            first_seen_at: now,
            last_seen_at: now,
            mention_count: 1,
        }
    }

    /// Update the last seen time and increment mention count
    pub fn touch(&mut self) {
        self.last_seen_at = Utc::now();
        self.mention_count += 1;
    }

    /// Add a property
    pub fn with_property(mut self, key: String, value: serde_json::Value) -> Self {
        self.properties.insert(key, value);
        self
    }
}

/// Extracted entities and relationships from a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedKnowledge {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

/// Direction for neighbor queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Entity → Other (outgoing edges)
    Outgoing,
    /// Other → Entity (incoming edges)
    Incoming,
    /// Either direction
    Both,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Both
    }
}

/// Information about a neighboring entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborInfo {
    /// The neighboring entity
    pub entity: Entity,
    /// The relationship connecting to the neighbor
    pub relationship: Relationship,
    /// Direction of the relationship from the source entity's perspective
    pub direction: Direction,
}

/// Entity with its connections (incoming and outgoing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityWithConnections {
    /// The central entity
    pub entity: Entity,
    /// Outgoing relationships: Entity → Other
    pub outgoing: Vec<(Relationship, Entity)>,
    /// Incoming relationships: Other → Entity
    pub incoming: Vec<(Relationship, Entity)>,
}

/// Graph statistics for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    /// Total number of entities
    pub entity_count: usize,
    /// Total number of relationships
    pub relationship_count: usize,
    /// Entity counts by type
    pub entity_types: std::collections::HashMap<String, usize>,
    /// Relationship counts by type
    pub relationship_types: std::collections::HashMap<String, usize>,
    /// Top entities by connection count (entity_name, connection_count)
    pub most_connected_entities: Vec<(String, usize)>,
}

/// Subgraph extracted around a center entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    /// All entities in the subgraph
    pub entities: Vec<Entity>,
    /// All relationships in the subgraph
    pub relationships: Vec<Relationship>,
    /// ID of the center entity
    pub center: String,
    /// Maximum hops from center
    pub max_hops: usize,
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_entity_type_from_str() {
        assert_eq!(EntityType::from_str("person"), EntityType::Person);
        assert_eq!(EntityType::from_str("PERSON"), EntityType::Person);
        assert_eq!(EntityType::from_str("Person"), EntityType::Person);
        assert_eq!(EntityType::from_str("organization"), EntityType::Organization);
        assert_eq!(EntityType::from_str("org"), EntityType::Organization);
        assert_eq!(EntityType::from_str("location"), EntityType::Location);
        assert_eq!(EntityType::from_str("concept"), EntityType::Concept);
        assert_eq!(EntityType::from_str("tool"), EntityType::Tool);
        assert_eq!(EntityType::from_str("project"), EntityType::Project);
        assert_eq!(EntityType::from_str("file"), EntityType::File);
        assert_eq!(EntityType::from_str("File"), EntityType::File);
        assert_eq!(EntityType::from_str("custom_type"), EntityType::Custom("custom_type".to_string()));
    }

    #[test]
    fn test_entity_type_as_str() {
        assert_eq!(EntityType::Person.as_str(), "person");
        assert_eq!(EntityType::Organization.as_str(), "organization");
        assert_eq!(EntityType::Location.as_str(), "location");
        assert_eq!(EntityType::Concept.as_str(), "concept");
        assert_eq!(EntityType::Tool.as_str(), "tool");
        assert_eq!(EntityType::Project.as_str(), "project");
        assert_eq!(EntityType::File.as_str(), "file");
        assert_eq!(EntityType::Custom("custom".to_string()).as_str(), "custom");
    }

    #[test]
    fn test_relationship_type_from_str() {
        assert_eq!(RelationshipType::from_str("works_for"), RelationshipType::WorksFor);
        assert_eq!(RelationshipType::from_str("worksfor"), RelationshipType::WorksFor);
        assert_eq!(RelationshipType::from_str("located_in"), RelationshipType::LocatedIn);
        assert_eq!(RelationshipType::from_str("related_to"), RelationshipType::RelatedTo);
        assert_eq!(RelationshipType::from_str("created"), RelationshipType::Created);
        assert_eq!(RelationshipType::from_str("uses"), RelationshipType::Uses);
        assert_eq!(RelationshipType::from_str("part_of"), RelationshipType::PartOf);
        assert_eq!(RelationshipType::from_str("mentions"), RelationshipType::Mentions);
        assert_eq!(RelationshipType::from_str("custom_rel"), RelationshipType::Custom("customrel".to_string()));
    }

    #[test]
    fn test_relationship_type_as_str() {
        assert_eq!(RelationshipType::WorksFor.as_str(), "works_for");
        assert_eq!(RelationshipType::LocatedIn.as_str(), "located_in");
        assert_eq!(RelationshipType::RelatedTo.as_str(), "related_to");
        assert_eq!(RelationshipType::Created.as_str(), "created");
        assert_eq!(RelationshipType::Uses.as_str(), "uses");
        assert_eq!(RelationshipType::PartOf.as_str(), "part_of");
        assert_eq!(RelationshipType::Mentions.as_str(), "mentions");
        assert_eq!(RelationshipType::Custom("custom".to_string()).as_str(), "custom");
    }

    #[test]
    fn test_entity_new() {
        let entity = Entity::new(
            "agent-123".to_string(),
            EntityType::Person,
            "John Smith".to_string(),
        );

        assert_eq!(entity.agent_id, "agent-123");
        assert_eq!(entity.name, "John Smith");
        assert!(matches!(entity.entity_type, EntityType::Person));
        assert_eq!(entity.mention_count, 1);
        assert!(entity.properties.is_empty());
        assert!(entity.id.starts_with("entity_agent-123_"));
    }

    #[test]
    fn test_entity_touch() {
        let mut entity = Entity::new(
            "agent-123".to_string(),
            EntityType::Person,
            "John Smith".to_string(),
        );

        let original_count = entity.mention_count;
        entity.touch();

        assert_eq!(entity.mention_count, original_count + 1);
        assert!(entity.last_seen_at > entity.first_seen_at);
    }

    #[test]
    fn test_entity_with_property() {
        let entity = Entity::new(
            "agent-123".to_string(),
            EntityType::Person,
            "John Smith".to_string(),
        )
        .with_property("email".to_string(), json!("john@example.com"))
        .with_property("role".to_string(), json!("Engineer"));

        assert_eq!(entity.properties.len(), 2);
        assert_eq!(entity.properties.get("email"), Some(&json!("john@example.com")));
        assert_eq!(entity.properties.get("role"), Some(&json!("Engineer")));
    }

    #[test]
    fn test_entity_serialization() {
        let entity = Entity::new(
            "agent-123".to_string(),
            EntityType::Organization,
            "Acme Corp".to_string(),
        );

        let json_str = serde_json::to_string(&entity).unwrap();
        let parsed: Entity = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.id, entity.id);
        assert_eq!(parsed.name, entity.name);
        assert_eq!(parsed.agent_id, entity.agent_id);
    }

    #[test]
    fn test_relationship_new() {
        let relationship = Relationship::new(
            "agent-123".to_string(),
            "entity-1".to_string(),
            "entity-2".to_string(),
            RelationshipType::WorksFor,
        );

        assert_eq!(relationship.agent_id, "agent-123");
        assert_eq!(relationship.source_entity_id, "entity-1");
        assert_eq!(relationship.target_entity_id, "entity-2");
        assert!(matches!(relationship.relationship_type, RelationshipType::WorksFor));
        assert_eq!(relationship.mention_count, 1);
        assert!(relationship.properties.is_empty());
    }

    #[test]
    fn test_relationship_touch() {
        let mut relationship = Relationship::new(
            "agent-123".to_string(),
            "entity-1".to_string(),
            "entity-2".to_string(),
            RelationshipType::Uses,
        );

        let original_count = relationship.mention_count;
        relationship.touch();

        assert_eq!(relationship.mention_count, original_count + 1);
        assert!(relationship.last_seen_at > relationship.first_seen_at);
    }

    #[test]
    fn test_relationship_with_property() {
        let relationship = Relationship::new(
            "agent-123".to_string(),
            "entity-1".to_string(),
            "entity-2".to_string(),
            RelationshipType::RelatedTo,
        )
        .with_property("confidence".to_string(), json!(0.9))
        .with_property("context".to_string(), json!("project meeting"));

        assert_eq!(relationship.properties.len(), 2);
        assert_eq!(relationship.properties.get("confidence"), Some(&json!(0.9)));
    }

    #[test]
    fn test_relationship_serialization() {
        let relationship = Relationship::new(
            "agent-123".to_string(),
            "source".to_string(),
            "target".to_string(),
            RelationshipType::PartOf,
        );

        let json_str = serde_json::to_string(&relationship).unwrap();
        let parsed: Relationship = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.id, relationship.id);
        assert_eq!(parsed.source_entity_id, "source");
        assert_eq!(parsed.target_entity_id, "target");
    }

    #[test]
    fn test_extracted_knowledge_empty() {
        let knowledge = ExtractedKnowledge {
            entities: vec![],
            relationships: vec![],
        };

        assert!(knowledge.entities.is_empty());
        assert!(knowledge.relationships.is_empty());
    }

    #[test]
    fn test_extracted_knowledge_with_data() {
        let entity = Entity::new(
            "agent-1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );

        let relationship = Relationship::new(
            "agent-1".to_string(),
            entity.id.clone(),
            "org-1".to_string(),
            RelationshipType::WorksFor,
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![entity],
            relationships: vec![relationship],
        };

        assert_eq!(knowledge.entities.len(), 1);
        assert_eq!(knowledge.relationships.len(), 1);
        assert_eq!(knowledge.entities[0].name, "Alice");
    }
}
