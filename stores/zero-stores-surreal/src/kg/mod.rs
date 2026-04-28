//! `SurrealKgStore` — `KnowledgeGraphStore` impl over `Arc<Surreal<Any>>`.

use std::sync::Arc;

use async_trait::async_trait;
use knowledge_graph::types::{Entity, EntityType, GraphStats, NeighborInfo, Relationship, Subgraph};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use zero_stores::KnowledgeGraphStore;
use zero_stores::error::{StoreError, StoreResult};
use zero_stores::extracted::ExtractedKnowledge;
use zero_stores::types::{
    ArchivableEntity, Direction, EntityId, KgStats, Neighbor, ReindexReport, RelationshipId,
    ResolveOutcome, StoreOutcome, TraversalHit, VecIndexHealth,
};

mod alias;
mod entity;
mod relationship;
mod search;
mod traverse;

#[derive(Clone)]
pub struct SurrealKgStore {
    db: Arc<Surreal<Any>>,
}

impl SurrealKgStore {
    pub fn new(db: Arc<Surreal<Any>>) -> Self {
        Self { db }
    }

    pub(crate) fn db(&self) -> &Arc<Surreal<Any>> {
        &self.db
    }
}

fn unimplemented_err(task: &str) -> StoreError {
    StoreError::Backend(format!("SurrealKgStore: {task} pending"))
}

#[async_trait]
impl KnowledgeGraphStore for SurrealKgStore {
    // === entity (Task 5) ===
    async fn upsert_entity(&self, agent_id: &str, e: Entity) -> StoreResult<EntityId> {
        entity::upsert(self.db(), agent_id, e).await
    }

    async fn get_entity(&self, id: &EntityId) -> StoreResult<Option<Entity>> {
        entity::get(self.db(), id).await
    }

    async fn delete_entity(&self, id: &EntityId) -> StoreResult<()> {
        entity::delete(self.db(), id).await
    }

    async fn bump_entity_mention(&self, id: &EntityId) -> StoreResult<()> {
        entity::bump_mention(self.db(), id).await
    }

    // === alias / resolve (Task 6) ===
    async fn add_alias(&self, entity_id: &EntityId, surface: &str) -> StoreResult<()> {
        alias::add_alias(self.db(), entity_id, surface).await
    }

    async fn resolve_entity(
        &self,
        agent_id: &str,
        entity_type: &EntityType,
        name: &str,
        embedding: Option<&[f32]>,
    ) -> StoreResult<ResolveOutcome> {
        alias::resolve_entity(self.db(), agent_id, entity_type, name, embedding).await
    }

    // === relationships (Task 7) ===
    async fn upsert_relationship(
        &self,
        agent_id: &str,
        rel: Relationship,
    ) -> StoreResult<RelationshipId> {
        relationship::upsert_relationship(self.db(), agent_id, rel).await
    }

    async fn delete_relationship(&self, id: &RelationshipId) -> StoreResult<()> {
        relationship::delete_relationship(self.db(), id).await
    }

    async fn store_knowledge(
        &self,
        agent_id: &str,
        knowledge: ExtractedKnowledge,
    ) -> StoreResult<StoreOutcome> {
        relationship::store_knowledge(self.db(), agent_id, knowledge).await
    }

    // === traverse (Task 8) ===
    async fn get_neighbors(
        &self,
        id: &EntityId,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<Neighbor>> {
        traverse::get_neighbors(self.db(), id, direction, limit).await
    }

    async fn traverse(
        &self,
        seed: &EntityId,
        max_hops: usize,
        limit: usize,
    ) -> StoreResult<Vec<TraversalHit>> {
        traverse::traverse(self.db(), seed, max_hops, limit).await
    }

    async fn get_neighbors_full(
        &self,
        agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<NeighborInfo>> {
        traverse::get_neighbors_full(self.db(), agent_id, entity_id, direction, limit).await
    }

    async fn get_subgraph(
        &self,
        agent_id: &str,
        center_entity_id: &str,
        max_hops: usize,
    ) -> StoreResult<Subgraph> {
        traverse::get_subgraph(self.db(), agent_id, center_entity_id, max_hops).await
    }

    // === search (Task 9) ===
    async fn search_entities_by_name(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        search::search_entities_by_name(self.db(), agent_id, query, limit).await
    }

    // === reindex (Task 10) ===
    async fn reindex_embeddings(&self, _new_dim: usize) -> StoreResult<ReindexReport> {
        Err(unimplemented_err("reindex_embeddings (Task 10)"))
    }

    // === archival (Task 11) ===
    async fn list_archivable_orphans(
        &self,
        _min_age_hours: u32,
        _limit: usize,
    ) -> StoreResult<Vec<ArchivableEntity>> {
        Err(unimplemented_err("list_archivable_orphans (Task 11)"))
    }

    async fn mark_entity_archival(&self, _id: &EntityId, _reason: &str) -> StoreResult<()> {
        Err(unimplemented_err("mark_entity_archival (Task 11)"))
    }

    // === stats / list / health (Task 12) ===
    async fn stats(&self) -> StoreResult<KgStats> {
        Err(unimplemented_err("stats (Task 12)"))
    }

    async fn graph_stats(&self, _agent_id: &str) -> StoreResult<GraphStats> {
        Err(unimplemented_err("graph_stats (Task 12)"))
    }

    async fn list_entities(
        &self,
        _agent_id: &str,
        _entity_type: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> StoreResult<Vec<Entity>> {
        Err(unimplemented_err("list_entities (Task 12)"))
    }

    async fn list_relationships(
        &self,
        _agent_id: &str,
        _relationship_type: Option<&str>,
        _limit: usize,
        _offset: usize,
    ) -> StoreResult<Vec<Relationship>> {
        Err(unimplemented_err("list_relationships (Task 12)"))
    }

    async fn count_all_entities(&self) -> StoreResult<usize> {
        Err(unimplemented_err("count_all_entities (Task 12)"))
    }

    async fn count_all_relationships(&self) -> StoreResult<usize> {
        Err(unimplemented_err("count_all_relationships (Task 12)"))
    }

    async fn list_all_entities(
        &self,
        _ward_id: Option<&str>,
        _entity_type: Option<&str>,
        _limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        Err(unimplemented_err("list_all_entities (Task 12)"))
    }

    async fn list_all_relationships(&self, _limit: usize) -> StoreResult<Vec<Relationship>> {
        Err(unimplemented_err("list_all_relationships (Task 12)"))
    }

    async fn vec_index_health(&self) -> StoreResult<VecIndexHealth> {
        Err(unimplemented_err("vec_index_health (Task 12)"))
    }
}
