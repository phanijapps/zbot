//! SQLite implementation of `MemoryFactStore`.
//!
//! `gateway_database::GatewayMemoryFactStore` already implements
//! `zero_stores_traits::MemoryFactStore` directly, with the SQLite-specific
//! persistence wired through `MemoryRepository` and the optional
//! `EmbeddingClient`. This crate re-exports it under the canonical
//! `SqliteMemoryStore` name so that the persistence factory in
//! `gateway/src/state/persistence_factory.rs` can construct the store via
//! the same `zero-stores-sqlite::Sqlite*` naming convention used for
//! `SqliteKgStore`.
//!
//! When SurrealDB lands, its impl will live in
//! `stores/zero-stores-surreal/src/memory_facts.rs` as
//! `SurrealMemoryStore`.

pub use gateway_database::GatewayMemoryFactStore as SqliteMemoryStore;
