use std::sync::Arc;

use async_trait::async_trait;
use knowledge_graph::storage::GraphStorage;
use knowledge_graph::types::{Entity, EntityType, Relationship};
use zero_stores::error::StoreError;
use zero_stores::extracted::ExtractedKnowledge;
use zero_stores::types::{
    Direction, EntityId, KgStats, Neighbor, ReindexReport, RelationshipId, ResolveOutcome,
    StoreOutcome, TraversalHit,
};
use zero_stores::KnowledgeGraphStore;
use zero_stores::StoreResult;

use crate::blocking::{block, map_graph_err};

/// SQLite implementation of `KnowledgeGraphStore`. Wraps the existing
/// `knowledge_graph::storage::GraphStorage` and bridges its synchronous
/// rusqlite API into the async trait via `spawn_blocking`.
#[derive(Clone)]
pub struct SqliteKgStore {
    storage: Arc<GraphStorage>,
}

impl SqliteKgStore {
    pub fn new(storage: Arc<GraphStorage>) -> Self {
        Self { storage }
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

    async fn add_alias(&self, _entity_id: &EntityId, _surface: &str) -> StoreResult<()> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn resolve_entity(
        &self,
        _agent_id: &str,
        _entity_type: &EntityType,
        _name: &str,
        _embedding: Option<&[f32]>,
    ) -> StoreResult<ResolveOutcome> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn upsert_relationship(
        &self,
        _agent_id: &str,
        _rel: Relationship,
    ) -> StoreResult<RelationshipId> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn delete_relationship(&self, _id: &RelationshipId) -> StoreResult<()> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn store_knowledge(
        &self,
        _agent_id: &str,
        _knowledge: ExtractedKnowledge,
    ) -> StoreResult<StoreOutcome> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn get_neighbors(
        &self,
        _id: &EntityId,
        _direction: Direction,
        _limit: usize,
    ) -> StoreResult<Vec<Neighbor>> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn traverse(
        &self,
        _seed: &EntityId,
        _max_hops: usize,
        _limit: usize,
    ) -> StoreResult<Vec<TraversalHit>> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn search_entities_by_name(
        &self,
        _agent_id: &str,
        _query: &str,
        _limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn reindex_embeddings(&self, _new_dim: usize) -> StoreResult<ReindexReport> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn stats(&self) -> StoreResult<KgStats> {
        Err(StoreError::Backend("not implemented".into()))
    }
}
