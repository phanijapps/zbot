use crate::error::StoreResult;
use crate::extracted::ExtractedKnowledge;
use crate::types::*;
use async_trait::async_trait;
use knowledge_graph::types::{
    Entity, EntityType, GraphStats, NeighborInfo, Relationship, Subgraph,
};

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

    /// Find entities that satisfy the orphan-archival heuristic:
    /// `mention_count = 1`, `confidence < 0.5`, `first_seen_at` older
    /// than `min_age_hours`, zero incoming AND zero outgoing
    /// relationships, and not already archived (`epistemic_class !=
    /// 'archival'`). Used by the sleep-time orphan archiver. Hard-cap
    /// the result at `limit` rows for runaway protection.
    async fn list_archivable_orphans(
        &self,
        min_age_hours: u32,
        limit: usize,
    ) -> StoreResult<Vec<ArchivableEntity>>;

    /// Soft-delete an entity by marking it archival. Sets the entity's
    /// `epistemic_class` to `'archival'`, records `reason` in
    /// `compressed_into`, and removes the entity's name-index row (so
    /// future searches don't surface it). Used by the sleep-time orphan
    /// archiver. Atomically applies all writes via a single transaction.
    async fn mark_entity_archival(&self, id: &EntityId, reason: &str) -> StoreResult<()>;

    // ---- HTTP read paths (graph.rs handlers) ------------------------------

    /// Aggregate stats view used by `GET /api/graph/:agent_id/stats`.
    /// Computes entity/relationship counts, type breakdowns, and the
    /// top-10 most-connected entities for a given agent. The richer
    /// shape (vs. [`KnowledgeGraphStore::stats`]) is intentional —
    /// `stats` returns global counts only and is used by maintenance
    /// jobs; this is the per-agent UI view.
    async fn graph_stats(&self, agent_id: &str) -> StoreResult<GraphStats>;

    /// List entities for an agent with optional `entity_type` filter and
    /// LIMIT/OFFSET pagination. Order is `mention_count DESC`.
    async fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Entity>>;

    /// List relationships for an agent with optional `relationship_type`
    /// filter and LIMIT/OFFSET pagination. Order is `mention_count DESC`.
    async fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Relationship>>;

    /// Get full neighbor info (entity + relationship + direction) for an
    /// entity. Unlike [`KnowledgeGraphStore::get_neighbors`] which
    /// returns the lightweight [`Neighbor`] type, this preserves the
    /// hydrated entity and relationship payloads for HTTP responses.
    async fn get_neighbors_full(
        &self,
        agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<NeighborInfo>>;

    /// BFS subgraph centered on `center_entity_id` out to `max_hops`.
    /// Used by `GET /api/graph/:agent_id/entities/:entity_id/subgraph`.
    async fn get_subgraph(
        &self,
        agent_id: &str,
        center_entity_id: &str,
        max_hops: usize,
    ) -> StoreResult<Subgraph>;

    /// Count all entities across all agents. Used by the Observatory
    /// aggregate stats endpoint.
    async fn count_all_entities(&self) -> StoreResult<usize>;

    /// Count all relationships across all agents.
    async fn count_all_relationships(&self) -> StoreResult<usize>;

    /// List entities across all agents with optional ward/type filters.
    /// Used by `GET /api/graph/all/entities`.
    async fn list_all_entities(
        &self,
        ward_id: Option<&str>,
        entity_type: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<Entity>>;

    /// List all relationships across all agents.
    /// Used by `GET /api/graph/all/relationships`.
    async fn list_all_relationships(&self, limit: usize) -> StoreResult<Vec<Relationship>>;

    /// Vec0-index health snapshot: which of the expected vector tables
    /// exist in the backing store and how many rows are indexed in
    /// total. Backend-specific in implementation (SQLite-vec aux tables,
    /// SurrealDB index counts) but the trait surface stays the same.
    /// Used by `GET /api/embeddings/health`.
    async fn vec_index_health(&self) -> StoreResult<VecIndexHealth>;
}
