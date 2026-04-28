//! SQLite implementation of the `zero-stores` traits. Wraps the existing
//! `knowledge_graph::storage::GraphStorage` and bridges its sync rusqlite
//! API into async via `tokio::task::spawn_blocking`.

mod blocking;
pub mod bootstrap;
mod knowledge_graph;
pub mod memory_facts;
pub mod reindex;

pub use knowledge_graph::SqliteKgStore;
pub use memory_facts::SqliteMemoryStore;

// Phase D6 (partial): re-export the KG storage types from this crate so
// callers can use the "right" import path (`zero_stores_sqlite::*`)
// regardless of where the implementation file currently sits. The
// physical relocation of `services/knowledge-graph/src/storage.rs` (3k+
// LoC) into this crate is deferred to a dedicated mechanical pass —
// scoped in `memory-bank/future-state/db-provider-portability.md`. Until
// then the re-exports preserve the architectural contract: SQLite-coupled
// storage logic is reachable through `zero-stores-sqlite`, and consumers
// who import here will not need to update once the file moves.
pub mod kg {
    pub use ::knowledge_graph::storage::{GraphStorage, OrphanCandidate};
    pub use ::knowledge_graph::traversal::{
        GraphTraversal, SqliteGraphTraversal, TraversalNode,
    };
    pub use ::knowledge_graph::causal::{CausalEdge, CausalEdgeStore};
}
