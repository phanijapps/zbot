//! # gateway-database (deprecated shim)
//!
//! Slice D8 (2026-04) merged this crate's contents into `zero-stores-sqlite`.
//! This file remains for one transitional commit so the workspace stays
//! green while every consumer's `use gateway_database::...` is rewritten to
//! `use zero_stores_sqlite::...`. The next commit removes the crate
//! entirely.

pub use zero_stores_sqlite::*;

// Modules that consumers reach through `gateway_database::<module>::Type` need
// to be re-exposed as named modules — `pub use ::*` only re-exports names, not
// the module hierarchy itself.
pub use zero_stores_sqlite::knowledge_db;
pub use zero_stores_sqlite::knowledge_schema;
pub use zero_stores_sqlite::memory_repository;
pub use zero_stores_sqlite::episode_repository;
pub use zero_stores_sqlite::wiki_repository;
pub use zero_stores_sqlite::procedure_repository;
pub use zero_stores_sqlite::goal_repository;
pub use zero_stores_sqlite::recall_log_repository;
pub use zero_stores_sqlite::distillation_repository;
pub use zero_stores_sqlite::compaction_repository;
pub use zero_stores_sqlite::kg_episode_repository;
pub use zero_stores_sqlite::repository;
pub use zero_stores_sqlite::vector_index;
pub use zero_stores_sqlite::sqlite_vec_loader;
pub use zero_stores_sqlite::episode_store;
pub use zero_stores_sqlite::wiki_store;
pub use zero_stores_sqlite::procedure_store;
pub use zero_stores_sqlite::memory_fact_store;
pub use zero_stores_sqlite::auxiliary_stores;
pub use zero_stores_sqlite::age_bucket;
pub use zero_stores_sqlite::bootstrap;
