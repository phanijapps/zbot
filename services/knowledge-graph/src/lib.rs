#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
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
pub mod extractor;
pub mod service;
pub mod storage;
pub mod traversal;
pub mod types;

pub use error::{GraphError, GraphResult};
pub use extractor::EntityExtractor;
pub use service::GraphService;
pub use storage::GraphStorage;
pub use traversal::{GraphTraversal, SqliteGraphTraversal, TraversalNode};
pub use types::{
    Direction, Entity, EntityType, EntityWithConnections, ExtractedKnowledge, GraphStats,
    NeighborInfo, Relationship, RelationshipType, Subgraph,
};
