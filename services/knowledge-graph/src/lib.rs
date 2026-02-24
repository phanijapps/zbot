//! # Knowledge Graph
//!
//! Extracts and stores entities and relationships from conversations.
//!
//! Features:
//! - Entity extraction (people, places, organizations, concepts)
//! - Relationship extraction (works for, located in, related to, etc.)
//! - SQLite storage with full-text search
//! - LLM-powered smart extraction

pub mod error;
pub mod types;
pub mod extractor;
pub mod storage;
pub mod service;

pub use error::{GraphError, GraphResult};
pub use types::{Entity, Relationship, EntityType, RelationshipType, Direction, NeighborInfo, EntityWithConnections, ExtractedKnowledge, GraphStats, Subgraph};
pub use extractor::EntityExtractor;
pub use storage::GraphStorage;
pub use service::GraphService;
