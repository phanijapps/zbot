//! SQLite implementation of the `zero-stores` traits. Wraps the existing
//! `knowledge_graph::storage::GraphStorage` and bridges its sync rusqlite
//! API into async via `tokio::task::spawn_blocking`.

mod blocking;
mod knowledge_graph;

pub use knowledge_graph::SqliteKgStore;
