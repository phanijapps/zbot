//! SQLite implementation of the `zero-stores` traits. Wraps the
//! `kg::storage::GraphStorage` (relocated here from
//! `services/knowledge-graph` in Slice D6b) and bridges its synchronous
//! rusqlite API into async via `tokio::task::spawn_blocking`.

mod blocking;
pub mod bootstrap;
pub mod kg;
mod knowledge_graph;
pub mod memory_facts;
pub mod reindex;

pub use knowledge_graph::SqliteKgStore;
pub use memory_facts::SqliteMemoryStore;
