//! Pure-data domain types for the AgentZero persistence layer.
//!
//! This crate is dep-light by design — only `serde`. It is the home for
//! every value type that crosses the persistence boundary (in either
//! direction) and that any backend impl needs to round-trip.
//!
//! Adding a new domain type? It belongs here when:
//! - It has no methods that touch a database, file, or network
//! - Multiple crates need to construct or read it
//! - Backend impls (SQLite, SurrealDB, Postgres, …) all serialize the same shape
//!
//! Things that do NOT belong here:
//! - Repository structs (`MemoryRepository`, `KnowledgeDatabase`) — those are
//!   storage logic, they live in backend impl crates.
//! - Trait surfaces (`MemoryFactStore`, `KnowledgeGraphStore`) — those live in
//!   `zero-stores-traits`.
//! - HTTP request/response shapes — those live in the gateway HTTP layer
//!   (they can derive From/Into the domain types here).

pub mod memory_fact;

pub use memory_fact::{MemoryFact, ScoredFact};
