use std::sync::Arc;

use agent_runtime::llm::EmbeddingClient;
use async_trait::async_trait;
use knowledge_graph::storage::{ArchivableEntityRow, GraphStorage};
use knowledge_graph::types::{Entity, EntityType, Relationship};
use zero_stores::error::StoreError;
use zero_stores::extracted::ExtractedKnowledge;
use zero_stores::types::{
    ArchivableEntity, Direction, EntityId, KgStats, Neighbor, ReindexReport, RelationshipId,
    ResolveOutcome, StoreOutcome, TraversalHit,
};
use zero_stores::KnowledgeGraphStore;
use zero_stores::StoreResult;

use crate::blocking::{block, map_graph_err};
use crate::reindex;

/// SQLite implementation of `KnowledgeGraphStore`. Wraps the existing
/// `knowledge_graph::storage::GraphStorage` and bridges its synchronous
/// rusqlite API into the async trait via `spawn_blocking`.
#[derive(Clone)]
pub struct SqliteKgStore {
    storage: Arc<GraphStorage>,
    /// Active embedding client used by `reindex_embeddings`. Optional so
    /// integration tests that don't exercise the reindex path can construct
    /// the store without wiring an embedding backend. Production wiring
    /// goes through [`SqliteKgStore::with_embedding_client`].
    embedding_client: Option<Arc<dyn EmbeddingClient>>,
}

impl SqliteKgStore {
    /// Construct a store without an embedding client. Calls to
    /// `reindex_embeddings` on a store built this way return
    /// `StoreError::Backend("no embedding client configured ...")`.
    pub fn new(storage: Arc<GraphStorage>) -> Self {
        Self {
            storage,
            embedding_client: None,
        }
    }

    /// Construct a store that supports `reindex_embeddings`.
    ///
    /// The supplied client must produce vectors of the dimension passed to
    /// [`KnowledgeGraphStore::reindex_embeddings`]; per-row mismatches are
    /// logged and skipped (the index for that row stays empty until the next
    /// reindex).
    pub fn with_embedding_client(
        storage: Arc<GraphStorage>,
        embedding_client: Arc<dyn EmbeddingClient>,
    ) -> Self {
        Self {
            storage,
            embedding_client: Some(embedding_client),
        }
    }
}

#[async_trait]
impl KnowledgeGraphStore for SqliteKgStore {
    // Methods filled in by Tasks 4-9.

    async fn upsert_entity(&self, agent_id: &str, entity: Entity) -> StoreResult<EntityId> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        block(move || {
            storage
                .upsert_entity(&agent_id, entity)
                .map(EntityId::from)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn get_entity(&self, id: &EntityId) -> StoreResult<Option<Entity>> {
        let storage = self.storage.clone();
        let id = id.0.clone();
        block(move || storage.get_entity_by_id(&id).map_err(map_graph_err)).await
    }

    async fn delete_entity(&self, id: &EntityId) -> StoreResult<()> {
        let storage = self.storage.clone();
        let id = id.0.clone();
        block(move || storage.delete_entity_by_id(&id).map_err(map_graph_err)).await
    }

    async fn bump_entity_mention(&self, id: &EntityId) -> StoreResult<()> {
        let storage = self.storage.clone();
        let id = id.0.clone();
        block(move || storage.bump_entity_mention(&id).map_err(map_graph_err)).await
    }

    async fn add_alias(&self, entity_id: &EntityId, surface: &str) -> StoreResult<()> {
        let storage = self.storage.clone();
        let entity_id = entity_id.0.clone();
        let surface = surface.to_string();
        block(move || {
            storage
                .add_alias(&entity_id, &surface)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn resolve_entity(
        &self,
        agent_id: &str,
        entity_type: &EntityType,
        name: &str,
        embedding: Option<&[f32]>,
    ) -> StoreResult<ResolveOutcome> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let entity_type = entity_type.clone();
        let name = name.to_string();
        let embedding = embedding.map(|e| e.to_vec());
        block(move || {
            let result = storage
                .resolve_entity(&agent_id, &entity_type, &name, embedding.as_deref())
                .map_err(map_graph_err)?;
            Ok(match result {
                Some(matched_id) => ResolveOutcome::Match(EntityId::from(matched_id)),
                None => ResolveOutcome::NoMatch,
            })
        })
        .await
    }

    async fn upsert_relationship(
        &self,
        agent_id: &str,
        rel: Relationship,
    ) -> StoreResult<RelationshipId> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        block(move || {
            storage
                .upsert_relationship(&agent_id, rel)
                .map(RelationshipId::from)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn delete_relationship(&self, id: &RelationshipId) -> StoreResult<()> {
        let storage = self.storage.clone();
        let id = id.0.clone();
        block(move || storage.delete_relationship(&id).map_err(map_graph_err)).await
    }

    async fn store_knowledge(
        &self,
        agent_id: &str,
        knowledge: ExtractedKnowledge,
    ) -> StoreResult<StoreOutcome> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        // Capture counts before consuming the value via `.into()`.
        // GraphStorage::store_knowledge returns `()` with no outcome breakdown,
        // so we derive counts from the input. merged vs inserted is unknown at
        // this layer for Phase 1, so all entities count as inserted.
        let entity_count = knowledge.entities.len() as u64;
        let rel_count = knowledge.relationships.len() as u64;
        block(move || {
            storage
                .store_knowledge(&agent_id, knowledge.into())
                .map_err(map_graph_err)?;
            Ok(StoreOutcome {
                entities_inserted: entity_count,
                entities_merged: 0,
                relationships_inserted: rel_count,
            })
        })
        .await
    }

    async fn get_neighbors(
        &self,
        id: &EntityId,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<Neighbor>> {
        let storage = self.storage.clone();
        let id = id.0.clone();
        block(move || {
            storage
                .get_neighbors("", &id, direction.into(), limit)
                .map(|rows| rows.into_iter().map(Into::into).collect())
                .map_err(map_graph_err)
        })
        .await
    }

    async fn traverse(
        &self,
        seed: &EntityId,
        max_hops: usize,
        limit: usize,
    ) -> StoreResult<Vec<TraversalHit>> {
        let storage = self.storage.clone();
        let seed = seed.0.clone();
        block(move || {
            storage
                .traverse_sync(&seed, max_hops.min(255) as u8, limit)
                .map(|rows| rows.into_iter().map(Into::into).collect())
                .map_err(map_graph_err)
        })
        .await
    }

    async fn search_entities_by_name(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let query = query.to_string();
        block(move || {
            storage
                .search_by_name(&agent_id, &query, limit)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn reindex_embeddings(&self, new_dim: usize) -> StoreResult<ReindexReport> {
        // The trait contract is "rebuild embedding indexes for new dim and
        // return a final report" — there is intentionally no progress
        // callback in the trait surface (a SurrealDB impl rebuilds in one
        // shot; UX progress is impl-specific). Callers that want per-table
        // progress events drive `crate::reindex::reindex_all` directly via
        // the gateway-side wrapper at
        // `gateway-execution/src/sleep/embedding_reindex.rs`.
        let client = self.embedding_client.clone().ok_or_else(|| {
            StoreError::Backend(
                "no embedding client configured — use SqliteKgStore::with_embedding_client".into(),
            )
        })?;

        let db = self.storage.knowledge_db().clone();
        let summaries = reindex::reindex_all(&db, client, new_dim, &|_, _, _| {})
            .await
            .map_err(StoreError::Backend)?;

        let tables_rebuilt: Vec<String> = summaries
            .iter()
            .map(|(target, _)| target.table.to_string())
            .collect();
        let rows_indexed: u64 = summaries
            .iter()
            .map(|(_, summary)| summary.indexed as u64)
            .sum();

        Ok(ReindexReport {
            tables_rebuilt,
            rows_indexed,
        })
    }

    async fn stats(&self) -> StoreResult<KgStats> {
        let storage = self.storage.clone();
        block(move || {
            let (entity_count, relationship_count, alias_count) =
                storage.stats().map_err(map_graph_err)?;
            Ok(KgStats {
                entity_count,
                relationship_count,
                alias_count,
            })
        })
        .await
    }

    async fn list_archivable_orphans(
        &self,
        min_age_hours: u32,
        limit: usize,
    ) -> StoreResult<Vec<ArchivableEntity>> {
        let storage = self.storage.clone();
        block(move || {
            storage
                .find_archivable_orphans(min_age_hours, limit)
                .map(|rows| {
                    rows.into_iter()
                        .map(|r: ArchivableEntityRow| ArchivableEntity {
                            entity_id: EntityId::from(r.id),
                            agent_id: r.agent_id,
                            entity_type: r.entity_type,
                            name: r.name,
                        })
                        .collect()
                })
                .map_err(map_graph_err)
        })
        .await
    }

    async fn mark_entity_archival(&self, id: &EntityId, reason: &str) -> StoreResult<()> {
        let storage = self.storage.clone();
        let id = id.0.clone();
        let reason = reason.to_string();
        block(move || {
            storage
                .mark_entity_archival(&id, &reason)
                .map_err(map_graph_err)
        })
        .await
    }
}
