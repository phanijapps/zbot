//! `SurrealKgStore` — `KnowledgeGraphStore` impl over `Arc<Surreal<Any>>`.

use std::sync::Arc;

use async_trait::async_trait;
use knowledge_graph::types::{
    Entity, EntityType, GraphStats, NeighborInfo, Relationship, Subgraph,
};
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use zero_stores::error::StoreResult;
use zero_stores::extracted::ExtractedKnowledge;
use zero_stores::types::{
    ArchivableEntity, Direction, EntityId, KgStats, Neighbor, ReindexReport, RelationshipId,
    ResolveOutcome, StoreOutcome, TraversalHit, VecIndexHealth,
};
use zero_stores::KnowledgeGraphStore;

mod alias;
mod archival;
mod entity;
mod maintenance;
mod reindex;
mod relationship;
mod search;
mod stats;
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
    async fn reindex_embeddings(&self, new_dim: usize) -> StoreResult<ReindexReport> {
        reindex::reindex_embeddings(self.db(), new_dim).await
    }

    // === archival (Task 11) ===
    async fn list_archivable_orphans(
        &self,
        min_age_hours: u32,
        limit: usize,
    ) -> StoreResult<Vec<ArchivableEntity>> {
        archival::list_archivable_orphans(self.db(), min_age_hours, limit).await
    }

    async fn mark_entity_archival(&self, id: &EntityId, reason: &str) -> StoreResult<()> {
        archival::mark_entity_archival(self.db(), id, reason).await
    }

    // === stats / list / health (Task 12) ===
    async fn stats(&self) -> StoreResult<KgStats> {
        stats::stats(self.db()).await
    }

    async fn graph_stats(&self, agent_id: &str) -> StoreResult<GraphStats> {
        stats::graph_stats(self.db(), agent_id).await
    }

    async fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Entity>> {
        stats::list_entities(self.db(), agent_id, entity_type, limit, offset).await
    }

    async fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Relationship>> {
        stats::list_relationships(self.db(), agent_id, relationship_type, limit, offset).await
    }

    async fn count_all_entities(&self) -> StoreResult<usize> {
        stats::count_all_entities(self.db()).await
    }

    async fn count_all_relationships(&self) -> StoreResult<usize> {
        stats::count_all_relationships(self.db()).await
    }

    async fn list_all_entities(
        &self,
        ward_id: Option<&str>,
        entity_type: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        stats::list_all_entities(self.db(), ward_id, entity_type, limit).await
    }

    async fn list_all_relationships(&self, limit: usize) -> StoreResult<Vec<Relationship>> {
        stats::list_all_relationships(self.db(), limit).await
    }

    async fn vec_index_health(&self) -> StoreResult<VecIndexHealth> {
        stats::vec_index_health(self.db()).await
    }

    // ---- Sleep-time maintenance (Phase D2) -------------------------------

    async fn find_duplicate_candidates(
        &self,
        agent_id: &str,
        entity_type: &EntityType,
        threshold: f32,
        limit: usize,
    ) -> StoreResult<Vec<zero_stores::DuplicateCandidate>> {
        maintenance::find_duplicate_candidates(self.db(), agent_id, entity_type, threshold, limit)
            .await
    }

    async fn merge_entity_into(
        &self,
        loser: &EntityId,
        winner: &EntityId,
    ) -> StoreResult<()> {
        maintenance::merge_entity_into(self.db(), loser, winner).await
    }

    async fn list_orphan_old_candidates(
        &self,
        agent_id: &str,
        min_age_days: i64,
        limit: usize,
    ) -> StoreResult<Vec<zero_stores::DecayCandidate>> {
        maintenance::list_orphan_old_candidates(self.db(), agent_id, min_age_days, limit).await
    }

    async fn mark_entity_pruned(&self, id: &EntityId) -> StoreResult<()> {
        maintenance::mark_entity_pruned(self.db(), id).await
    }
}
