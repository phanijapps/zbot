//! SurrealDB 3.0 implementation of the `zero-stores` traits.
//!
//! Exposes [`SurrealKgStore`] (`KnowledgeGraphStore`) and
//! [`SurrealMemoryStore`] (`MemoryFactStore`). Both wrap a shared
//! `Arc<Surreal<Any>>` handle constructed by [`connect`].
//!
//! See `AGENTS.md` for the locked design decisions.

pub mod config;
pub mod connection;
pub mod error;
pub mod kg;
pub mod memory;
pub mod schema;
pub mod types;

pub use config::{SurrealConfig, SurrealCredentials};
pub use connection::connect;
pub use kg::SurrealKgStore;
pub use memory::SurrealMemoryStore;
