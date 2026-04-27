use crate::error::StoreResult;
use crate::extracted::ExtractedKnowledge;
use crate::types::*;
use async_trait::async_trait;
use knowledge_graph::types::{Entity, EntityType, Relationship};

/// Backend-agnostic persistence for the knowledge graph subsystem.
#[async_trait]
pub trait KnowledgeGraphStore: Send + Sync {
    // ---- Entities ---------------------------------------------------------
    async fn upsert_entity(&self, agent_id: &str, entity: Entity) -> StoreResult<EntityId>;
    async fn get_entity(&self, id: &EntityId) -> StoreResult<Option<Entity>>;
    async fn delete_entity(&self, id: &EntityId) -> StoreResult<()>;
    async fn bump_entity_mention(&self, id: &EntityId) -> StoreResult<()>;

    // ---- Aliases & resolution --------------------------------------------
    async fn add_alias(&self, entity_id: &EntityId, surface: &str) -> StoreResult<()>;
    async fn resolve_entity(
        &self,
        agent_id: &str,
        entity_type: &EntityType,
        name: &str,
        embedding: Option<&[f32]>,
    ) -> StoreResult<ResolveOutcome>;

    // ---- Relationships ---------------------------------------------------
    async fn upsert_relationship(
        &self,
        agent_id: &str,
        rel: Relationship,
    ) -> StoreResult<RelationshipId>;
    async fn delete_relationship(&self, id: &RelationshipId) -> StoreResult<()>;

    // ---- Bulk ingest -----------------------------------------------------
    async fn store_knowledge(
        &self,
        agent_id: &str,
        knowledge: ExtractedKnowledge,
    ) -> StoreResult<StoreOutcome>;

    // ---- Read paths ------------------------------------------------------
    async fn get_neighbors(
        &self,
        id: &EntityId,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<Neighbor>>;

    async fn traverse(
        &self,
        seed: &EntityId,
        max_hops: usize,
        limit: usize,
    ) -> StoreResult<Vec<TraversalHit>>;

    async fn search_entities_by_name(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> StoreResult<Vec<Entity>>;

    // ---- Maintenance -----------------------------------------------------
    async fn reindex_embeddings(&self, new_dim: usize) -> StoreResult<ReindexReport>;
    async fn stats(&self) -> StoreResult<KgStats>;
}
