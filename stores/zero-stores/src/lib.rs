//! `zero-stores` — backend-agnostic persistence interfaces for AgentZero.
//!
//! This crate defines store traits and shared types only. It pulls in NO
//! database drivers. Concrete implementations live in sibling crates
//! (`zero-stores-sqlite`, future `zero-stores-surreal`).

pub mod error;
pub mod extracted;
pub mod knowledge_graph;
pub mod memory_facts;
pub mod types;

pub use error::{StoreError, StoreResult};
pub use extracted::ExtractedKnowledge;
pub use knowledge_graph::KnowledgeGraphStore;
pub use memory_facts::{MemoryFactStore, SkillIndexRow};
pub use types::*;
