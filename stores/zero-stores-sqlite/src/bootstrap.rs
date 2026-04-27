//! Schema bootstrap for the SQLite KG impl.
//!
//! This module is the canonical place where schema-evolution code lives
//! for the SQLite backend. Today it delegates to `gateway-database`'s
//! [`KnowledgeDatabase::new`] (which runs `initialize_knowledge_database`)
//! — those routines own the full schema DDL plus the historical inline
//! migrations (v1-v22) and the `migrations/v23, v24.sql` files.
//!
//! Future: the schema DDL can be moved here verbatim; for now we keep
//! it in `gateway-database` to avoid churn. The function below exists
//! so the bootstrap pattern is symmetric across impls — when SurrealDB
//! arrives, its bootstrap goes in `stores/zero-stores-surreal/src/
//! bootstrap.rs`, called from its impl crate's constructor analogously.
//!
//! TD-032 progress: pattern established; full schema relocation deferred
//! until proven necessary (e.g. when the schema needs to differ between
//! impls in shape, not just storage layer).

pub use gateway_database::knowledge_db::KnowledgeDatabase;

/// Idempotent schema bootstrap. Called by `SqliteKgStore` constructors
/// when running against a fresh data directory. Today this is a no-op
/// because [`KnowledgeDatabase::new`] already runs the bootstrap as a
/// side effect; kept here as the canonical hook point for the SQLite
/// backend.
pub fn bootstrap_schema(_db: &KnowledgeDatabase) -> Result<(), String> {
    // No-op: KnowledgeDatabase::new already runs the bootstrap.
    // Future SurrealDB impl will have non-trivial logic here.
    Ok(())
}
