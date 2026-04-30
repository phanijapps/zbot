//! SurrealDB 3.0 implementation of the `zero-stores` traits.
//!
//! Exposes the full set of trait impls — [`SurrealKgStore`] /
//! [`SurrealMemoryStore`] / [`SurrealEpisodeStore`] / [`SurrealWikiStore`] /
//! [`SurrealProcedureStore`] / [`SurrealGoalStore`] /
//! [`SurrealRecallLogStore`] / [`SurrealDistillationStore`]. All wrap a
//! shared `Arc<Surreal<Any>>` handle constructed by [`connect`].
//!
//! See `AGENTS.md` for the locked design decisions.

pub mod compaction;
pub mod config;
pub mod connection;
pub mod distillation;
pub mod episodes;
pub mod error;
pub mod goals;
pub mod kg;
pub mod kg_ingestion;
pub mod memory;
pub mod procedures;
pub mod recall_log;
pub mod row_value;
pub mod schema;
pub mod similarity;
pub mod types;
pub mod wiki;

pub use compaction::SurrealCompactionStore;
pub use config::{SurrealConfig, SurrealCredentials};
pub use connection::connect;
pub use distillation::SurrealDistillationStore;
pub use episodes::SurrealEpisodeStore;
pub use error::{map_surreal_error, MapSurreal};
pub use goals::SurrealGoalStore;
pub use kg::SurrealKgStore;
pub use kg_ingestion::SurrealKgEpisodeStore;
pub use memory::SurrealMemoryStore;
pub use procedures::SurrealProcedureStore;
pub use recall_log::SurrealRecallLogStore;
pub use types::{embedding_to_value, value_to_embedding, EntityIdExt, ThingExt};
pub use wiki::SurrealWikiStore;
