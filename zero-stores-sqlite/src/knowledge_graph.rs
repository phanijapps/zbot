use std::sync::Arc;

use async_trait::async_trait;
use knowledge_graph::storage::GraphStorage;
use knowledge_graph::types::{Entity, EntityType, Relationship};
use zero_stores::error::StoreError;
use zero_stores::extracted::ExtractedKnowledge;
use zero_stores::knowledge_graph::KnowledgeGraphStore;
use zero_stores::types::{
    Direction, EntityId, KgStats, Neighbor, ReindexReport, RelationshipId, ResolveOutcome,
    StoreOutcome, TraversalHit,
};
use zero_stores::StoreResult;

/// SQLite implementation of `KnowledgeGraphStore`. Wraps the existing
/// `knowledge_graph::storage::GraphStorage` and bridges its synchronous
/// rusqlite API into the async trait via `spawn_blocking`.
#[derive(Clone)]
pub struct SqliteKgStore {
    // Used by Tasks 4+
    #[allow(dead_code)]
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

    async fn upsert_entity(&self, _agent_id: &str, _entity: Entity) -> StoreResult<EntityId> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn get_entity(&self, _id: &EntityId) -> StoreResult<Option<Entity>> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn delete_entity(&self, _id: &EntityId) -> StoreResult<()> {
        Err(StoreError::Backend("not implemented".into()))
    }

    async fn bump_entity_mention(&self, _id: &EntityId) -> StoreResult<()> {
        Err(StoreError::Backend("not implemented".into()))
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
