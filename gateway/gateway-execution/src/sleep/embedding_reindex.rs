//! # Embedding Reindex (gateway-side wrapper)
//!
//! The vec0-rebuild orchestration moved into
//! [`zero_stores_sqlite::reindex`] in Phase 3d (TD-012). The trait method
//! [`zero_stores::KnowledgeGraphStore::reindex_embeddings`] is the
//! backend-agnostic surface that future SurrealDB / other impls will
//! implement.
//!
//! This thin wrapper module stays alive because two gateway-side callers
//! ([`crate::state`] boot reconcile and the `/api/embeddings/configure` SSE
//! handler) drive the reindex with a per-table progress callback that
//! publishes [`gateway_services::Health::Reindexing`] events to the UI's
//! `EmbeddingProgressModal`. The trait signature is intentionally
//! "fire and report" (one final `ReindexReport`, no progress) so the
//! abstraction stays portable; the progress-aware variant is exposed here
//! as a SQLite-impl-specific helper.
//!
//! When the SurrealDB impl lands, it will own its own progress-reporting
//! shape (or rebuild atomically without progress, if that's natural for
//! the backend); this module stays SQLite-only.

// Re-export the orchestration primitives so existing callers
// (`gateway::state::reconcile_embeddings_at_boot`, the `/api/embeddings`
// handler) keep their import paths working with no churn beyond a single
// dep addition.
pub use zero_stores_sqlite::reindex::{
    reindex_all, reindex_table, ProgressFn, ReindexSummary, ReindexTarget, REINDEX_TARGETS,
};
