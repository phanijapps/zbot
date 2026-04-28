//! # Persistence factory
//!
//! Centralized construction of persistence-layer trait objects.
//!
//! Today only the SQLite backend exists. When SurrealDB lands, this is
//! where the config-driven branch goes — `match config.knowledge_backend
//! { Sqlite => SqliteKgStore::with_embedding_client(…), Surreal => …}`.
//! The HTTP handlers and sleep jobs that consume `Arc<dyn KnowledgeGraphStore>`
//! don't need to know which backend they got.
//!
//! TD-023 progress: factory pattern established. Retirement of the
//! parallel `graph_service: Option<Arc<GraphService>>` field on AppState
//! is deferred to a follow-up multi-PR workstream — that affects dozens
//! of consumer sites and warrants its own phasing.

use std::sync::Arc;

use agent_runtime::llm::EmbeddingClient;
use gateway_database::KnowledgeDatabase;
use knowledge_graph::storage::GraphStorage;
use zero_stores::KnowledgeGraphStore;
use zero_stores_sqlite::SqliteKgStore;

/// Build the `Arc<dyn KnowledgeGraphStore>` used by `AppState`.
///
/// Today this always returns a `SqliteKgStore`. When SurrealDB support
/// lands, this function branches on a `PersistenceConfig` enum.
///
/// Constructs a fresh `GraphStorage` over the supplied `KnowledgeDatabase`.
/// Callers that already hold an `Arc<GraphStorage>` (e.g. for sharing with
/// the legacy `GraphService`) should use [`build_kg_store_from_storage`]
/// instead so the same storage handle is reused.
///
/// Currently no callsite uses this entrypoint — `AppState::new` shares
/// one `Arc<GraphStorage>` between the trait-object `kg_store` and the
/// legacy concrete `graph_service`, so it routes through
/// [`build_kg_store_from_storage`]. This canonical API exists for the
/// future state where `graph_service` has retired (TD-023's deferred
/// half) and the factory becomes the only construction site.
#[allow(dead_code)] // Canonical factory API; gated until graph_service retires (TD-023).
pub fn build_kg_store(
    knowledge_db: Arc<KnowledgeDatabase>,
    embedding_client: Arc<dyn EmbeddingClient>,
) -> Result<Arc<dyn KnowledgeGraphStore>, String> {
    let storage =
        Arc::new(GraphStorage::new(knowledge_db).map_err(|e| format!("GraphStorage::new: {e}"))?);
    Ok(build_kg_store_from_storage(storage, embedding_client))
}

/// Build the `Arc<dyn KnowledgeGraphStore>` from an existing `GraphStorage`.
///
/// Phase 5 helper: AppState today shares a single `Arc<GraphStorage>` between
/// the trait-object `kg_store` and the legacy concrete `graph_service`.
/// Until `graph_service` retires (deferred multi-PR workstream), the factory
/// has to accept the pre-built storage rather than recreating it. When
/// `graph_service` finally retires, callers migrate to
/// [`build_kg_store`] and this helper goes away.
pub fn build_kg_store_from_storage(
    storage: Arc<GraphStorage>,
    embedding_client: Arc<dyn EmbeddingClient>,
) -> Arc<dyn KnowledgeGraphStore> {
    Arc::new(SqliteKgStore::with_embedding_client(
        storage,
        embedding_client,
    ))
}
