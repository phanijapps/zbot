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
use gateway_database::{KnowledgeDatabase, MemoryRepository};
use knowledge_graph::storage::GraphStorage;
use zero_stores::{KnowledgeGraphStore, MemoryFactStore};
use zero_stores_sqlite::{SqliteKgStore, SqliteMemoryStore};

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

/// Build the `Arc<dyn MemoryFactStore>` used by `AppState`.
///
/// Today this always returns a `SqliteMemoryStore` (a re-export of
/// `gateway_database::GatewayMemoryFactStore`). When SurrealDB support
/// lands, this is the branch point — `match config.knowledge_backend
/// { Sqlite => SqliteMemoryStore::new(…), Surreal => SurrealMemoryStore::new(…) }`.
///
/// AppState shares one `Arc<MemoryRepository>` between this factory and
/// the legacy `memory_repo` field that many existing consumers still
/// hold by concrete type. Until those consumers migrate to the trait
/// object (a separate workstream — TD-023's deferred half), the factory
/// accepts the pre-built repository rather than recreating it. The
/// `embedding_client` is the live `LiveEmbeddingClient` so embedding
/// generation follows ArcSwap backend changes.
pub fn build_memory_store(
    memory_repo: Arc<MemoryRepository>,
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
) -> Arc<dyn MemoryFactStore> {
    Arc::new(SqliteMemoryStore::new(memory_repo, embedding_client))
}

// ============================================================================
// SurrealDB backend dispatch (Cargo feature `surreal-backend`).
//
// These functions construct an `Arc<dyn KnowledgeGraphStore>` /
// `Arc<dyn MemoryFactStore>` backed by a shared `Surreal<Any>` handle.
// Wiring into `AppState::new` is a follow-up — for now, callers can
// invoke these helpers directly when constructing a SurrealDB-backed
// daemon. The feature gate keeps the SurrealDB SDK out of default
// builds.
// ============================================================================

/// Configuration parameters for the SurrealDB backend.
///
/// Mirrors the `[persistence.surreal]` section of `settings.json`.
/// `vault_root` is used to expand the `$VAULT` placeholder in the URL.
#[cfg(feature = "surreal-backend")]
#[derive(Clone, Debug)]
pub struct SurrealBackendConfig {
    pub url: String,
    pub namespace: String,
    pub database: String,
    pub credentials: Option<(String, String)>,
    pub vault_root: std::path::PathBuf,
}

/// Read `config/settings.json` and return `Some(config)` if the user has
/// opted into SurrealDB (`persistence.knowledge_backend = "surreal"`).
/// Returns `None` for any other state — missing file, missing key,
/// "sqlite", parse error. Failing closed keeps SQLite as the safe default.
#[cfg(feature = "surreal-backend")]
fn read_surreal_opt_in(paths: &gateway_services::paths::VaultPaths) -> Option<SurrealBackendConfig> {
    let raw = std::fs::read_to_string(paths.settings()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let backend = value
        .get("persistence")
        .and_then(|p| p.get("knowledge_backend"))
        .and_then(|b| b.as_str())?;
    if backend != "surreal" {
        return None;
    }
    let surreal_obj = value.get("persistence").and_then(|p| p.get("surreal"));
    let url = surreal_obj
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(String::from)
        .unwrap_or_else(|| "rocksdb://$VAULT/data/knowledge.surreal".to_string());
    let namespace = surreal_obj
        .and_then(|s| s.get("namespace"))
        .and_then(|n| n.as_str())
        .map(String::from)
        .unwrap_or_else(|| "memory_kg".to_string());
    let database = surreal_obj
        .and_then(|s| s.get("database"))
        .and_then(|d| d.as_str())
        .map(String::from)
        .unwrap_or_else(|| "main".to_string());
    Some(SurrealBackendConfig {
        url,
        namespace,
        database,
        credentials: None,
        vault_root: paths.vault_dir().clone(),
    })
}

/// Construct the SurrealDB-backed store pair when the user has opted in
/// via settings.json, otherwise return `None`. Failures are fatal: a
/// configured-but-broken Surreal connection means the daemon refuses to
/// start (logged + panicked) — falling back to SQLite would silently
/// hide the problem.
#[cfg(feature = "surreal-backend")]
pub fn maybe_build_surreal_pair(
    paths: &gateway_services::paths::VaultPaths,
) -> Option<(Arc<dyn KnowledgeGraphStore>, Arc<dyn MemoryFactStore>)> {
    let cfg = read_surreal_opt_in(paths)?;
    tracing::info!(
        url = %cfg.url,
        namespace = %cfg.namespace,
        database = %cfg.database,
        "SurrealDB backend enabled via settings.json"
    );
    // Bridge to async via the ambient tokio runtime — AppState::new is
    // called from a tokio context (daemon main).
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(build_surreal_pair(&cfg))
    });
    match result {
        Ok(pair) => Some(pair),
        Err(e) => {
            tracing::error!(error = %e, "SurrealDB init failed — daemon refusing to start");
            panic!(
                "SurrealDB persistence init failed: {e}. \
                 If the database appears corrupted, run the recovery CLI \
                 backed by zero-stores-surreal-recovery."
            );
        }
    }
}

/// Build both SurrealDB-backed stores sharing one connection handle.
/// Schema is applied idempotently as part of construction. Returns the
/// (KG, Memory) pair. Errors fail fast — the daemon is expected to
/// refuse to start on persistence init failure.
#[cfg(feature = "surreal-backend")]
pub async fn build_surreal_pair(
    cfg: &SurrealBackendConfig,
) -> Result<(Arc<dyn KnowledgeGraphStore>, Arc<dyn MemoryFactStore>), String> {
    let surreal_cfg = zero_stores_surreal::SurrealConfig {
        url: cfg.url.clone(),
        namespace: cfg.namespace.clone(),
        database: cfg.database.clone(),
        credentials: cfg.credentials.as_ref().map(|(u, p)| {
            zero_stores_surreal::SurrealCredentials {
                username: u.clone(),
                password: p.clone(),
            }
        }),
    };
    let db = zero_stores_surreal::connect(&surreal_cfg, Some(&cfg.vault_root))
        .await
        .map_err(|e| format!("surreal connect: {e}"))?;
    zero_stores_surreal::schema::apply_schema(&db)
        .await
        .map_err(|e| format!("surreal schema: {e}"))?;
    let kg: Arc<dyn KnowledgeGraphStore> =
        Arc::new(zero_stores_surreal::SurrealKgStore::new(db.clone()));
    let mem: Arc<dyn MemoryFactStore> = Arc::new(zero_stores_surreal::SurrealMemoryStore::new(db));
    Ok((kg, mem))
}
