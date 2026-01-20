//! # Knowledge Graph Types
//!
//! Core data structures for entities and relationships.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Entity type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    /// Custom entity type
    Custom(String),
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
            EntityType::Custom(s) => s,
        }
    }
}

/// Relationship type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
