//! # Persistence factory
//!
//! Centralized construction of persistence-layer trait objects.
//!
//! AppState consumes `Arc<dyn KnowledgeGraphStore>` and `Arc<dyn
//! MemoryFactStore>` rather than the concrete SQLite repos so HTTP
//! handlers and sleep jobs don't need to know which backend they got.
//! The trait surfaces in `zero-stores-traits` keep the door open for
//! future backends; today there is one impl, SQLite.

use std::sync::Arc;

use agent_runtime::llm::EmbeddingClient;
use zero_stores::{KnowledgeGraphStore, MemoryFactStore};
use zero_stores_sqlite::kg::storage::GraphStorage;
use zero_stores_sqlite::{KnowledgeDatabase, MemoryRepository};
use zero_stores_sqlite::{SqliteKgStore, SqliteMemoryStore};

/// Build the `Arc<dyn KnowledgeGraphStore>` used by `AppState`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use gateway_services::VaultPaths;
    use tempfile::TempDir;
    use zero_stores_sqlite::vector_index::{SqliteVecIndex, VectorIndex};

    struct NoEmbed;
    #[async_trait::async_trait]
    impl EmbeddingClient for NoEmbed {
        async fn embed(
            &self,
            texts: &[&str],
        ) -> Result<Vec<Vec<f32>>, agent_runtime::llm::EmbeddingError> {
            Ok(vec![vec![0.0_f32; 4]; texts.len()])
        }
        fn dimensions(&self) -> usize {
            4
        }
        fn model_name(&self) -> String {
            "test-no-embed".to_string()
        }
    }

    fn fixture() -> (TempDir, Arc<KnowledgeDatabase>, Arc<MemoryRepository>) {
        let dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(dir.path().to_path_buf()));
        std::fs::create_dir_all(paths.data_dir()).unwrap();
        let kdb = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let vec: Arc<dyn VectorIndex> = Arc::new(
            SqliteVecIndex::new(kdb.clone(), "memory_facts_index", "fact_id").expect("vec index"),
        );
        let repo = Arc::new(MemoryRepository::new(kdb.clone(), vec));
        (dir, kdb, repo)
    }

    #[test]
    fn build_memory_store_returns_trait_object_without_embedder() {
        let (_dir, _kdb, repo) = fixture();
        let store = build_memory_store(repo, None);
        assert!(Arc::strong_count(&store) >= 1);
    }

    #[test]
    fn build_kg_store_from_storage_yields_arc_dyn() {
        let (_dir, kdb, _repo) = fixture();
        let storage = Arc::new(GraphStorage::new(kdb.clone()).expect("graph storage"));
        let embedder: Arc<dyn EmbeddingClient> = Arc::new(NoEmbed);
        let store = build_kg_store_from_storage(storage, embedder);
        assert!(Arc::strong_count(&store) >= 1);
    }

    #[test]
    fn build_kg_store_constructs_storage_then_delegates() {
        let (_dir, kdb, _repo) = fixture();
        let embedder: Arc<dyn EmbeddingClient> = Arc::new(NoEmbed);
        let store = build_kg_store(kdb, embedder).expect("build_kg_store");
        assert!(Arc::strong_count(&store) >= 1);
    }
}
