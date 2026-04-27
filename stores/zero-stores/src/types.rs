use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub String);

impl From<String> for EntityId {
    fn from(s: String) -> Self {
        EntityId(s)
    }
}
impl From<&str> for EntityId {
    fn from(s: &str) -> Self {
        EntityId(s.to_string())
    }
}
impl AsRef<str> for EntityId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationshipId(pub String);

impl From<String> for RelationshipId {
    fn from(s: String) -> Self {
        RelationshipId(s)
    }
}
impl AsRef<str> for RelationshipId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Debug, Clone)]
pub enum ResolveOutcome {
    Match(EntityId),
    NoMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neighbor {
    pub entity_id: EntityId,
    pub relationship_id: RelationshipId,
    pub relationship_type: String,
    pub direction: Direction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalHit {
    pub entity_id: EntityId,
    pub hop: usize,
    pub path: String,
    pub mention_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KgStats {
    pub entity_count: u64,
    pub relationship_count: u64,
    pub alias_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReindexReport {
    pub tables_rebuilt: Vec<String>,
    pub rows_indexed: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreOutcome {
    pub entities_inserted: u64,
    pub entities_merged: u64,
    pub relationships_inserted: u64,
}

// ============================================================================
// From conversions: knowledge_graph types → zero_stores types
// ============================================================================

impl From<knowledge_graph::types::Direction> for Direction {
    fn from(d: knowledge_graph::types::Direction) -> Self {
        match d {
            knowledge_graph::types::Direction::Outgoing => Direction::Outgoing,
            knowledge_graph::types::Direction::Incoming => Direction::Incoming,
            knowledge_graph::types::Direction::Both => Direction::Both,
        }
    }
}

impl From<Direction> for knowledge_graph::types::Direction {
    fn from(d: Direction) -> Self {
        match d {
            Direction::Outgoing => knowledge_graph::types::Direction::Outgoing,
            Direction::Incoming => knowledge_graph::types::Direction::Incoming,
            Direction::Both => knowledge_graph::types::Direction::Both,
        }
    }
}

impl From<knowledge_graph::types::NeighborInfo> for Neighbor {
    fn from(n: knowledge_graph::types::NeighborInfo) -> Self {
        Neighbor {
            entity_id: EntityId(n.entity.id),
            relationship_id: RelationshipId(n.relationship.id),
            relationship_type: n.relationship.relationship_type.as_str().to_string(),
            direction: n.direction.into(),
        }
    }
}

impl From<knowledge_graph::traversal::TraversalNode> for TraversalHit {
    fn from(n: knowledge_graph::traversal::TraversalNode) -> Self {
        TraversalHit {
            entity_id: EntityId(n.entity_id),
            hop: n.hop_distance as usize,
            path: n.path,
            mention_count: n.mention_count,
        }
    }
}

/// An entity that meets the orphan-archival heuristic: low confidence,
/// only seen once, old enough to be past the reinforcement grace period,
/// and with zero relationships in either direction. Returned by
/// `KnowledgeGraphStore::list_archivable_orphans` for the sleep-time
/// orphan archiver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivableEntity {
    pub entity_id: EntityId,
    pub agent_id: String,
    pub entity_type: String,
    pub name: String,
}
