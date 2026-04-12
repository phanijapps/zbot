//! # Knowledge Graph
//!
//! Extracts and stores entities and relationships from conversations.
//!
//! Features:
//! - Entity extraction (people, places, organizations, concepts)
//! - Relationship extraction (works for, located in, related to, etc.)
//! - SQLite storage with full-text search
//! - LLM-powered smart extraction

pub mod causal;
pub mod error;
pub mod extractor;
pub mod service;
pub mod storage;
pub mod traversal;
pub mod types;

pub use causal::{CausalEdge, CausalEdgeStore};
pub use error::{GraphError, GraphResult};
pub use extractor::EntityExtractor;
pub use service::GraphService;
pub use storage::GraphStorage;
pub use traversal::{GraphTraversal, SqliteGraphTraversal, TraversalNode};
pub use types::{
    Direction, Entity, EntityType, EntityWithConnections, ExtractedKnowledge, GraphStats,
    NeighborInfo, Relationship, RelationshipType, Subgraph,
};
