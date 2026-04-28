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
