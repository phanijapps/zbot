use crate::error::StoreResult;
use crate::extracted::ExtractedKnowledge;
use crate::types::*;
use async_trait::async_trait;
use knowledge_graph::types::{
    Entity, EntityType, GraphStats, NeighborInfo, Relationship, Subgraph,
};
// Port request/response shapes live in `zero-stores-domain`; re-export
// at this crate's surface so existing imports of
// `zero_stores::{DuplicateCandidate, DecayCandidate, StrategyCandidate,
// RelationshipContext, GraphView}` keep compiling.
pub use zero_stores_domain::{
    DecayCandidate, DuplicateCandidate, EntityNameEmbeddingHit, GraphView, RelationshipContext,
    StrategyCandidate,
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

    /// Exact-name lookup for an entity. Used by tools that resolve a
    /// human-typed name to an entity id before traversing neighbours.
    /// Default falls back to filtering `search_entities_by_name` so
    /// backends without an indexed lookup still work.
    async fn get_entity_by_name(&self, agent_id: &str, name: &str) -> StoreResult<Option<Entity>> {
        let matches = self.search_entities_by_name(agent_id, name, 16).await?;
        Ok(matches.into_iter().find(|e| e.name == name))
    }

    /// Search entities through a specific [`GraphView`] lens. Backends
    /// implement at least `Semantic` (mention_count DESC); other views
    /// may degrade to `Semantic` with a tracing warn. Default routes
    /// to `search_entities_by_name` so backends without view support
    /// still return ranked results.
    async fn search_entities_view(
        &self,
        agent_id: &str,
        query: &str,
        _view: GraphView,
        limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        self.search_entities_by_name(agent_id, query, limit).await
    }

    /// ANN search over the name-embedding index. Returns hits ordered by
    /// ascending L2-squared distance (callers convert to cosine via
    /// `1 - distance / 2`). The query embedding MUST be L2-normalized.
    /// Used by the unified-recall graph pool. Default returns empty so
    /// backends without a vector index over entity names degrade
    /// gracefully (the recall path just contributes no graph items).
    async fn search_entities_by_name_embedding(
        &self,
        _agent_id: &str,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> StoreResult<Vec<EntityNameEmbeddingHit>> {
        Ok(Vec::new())
    }

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

    // ---- Sleep-time maintenance (Phase D2) -------------------------------
    //
    // Operation-oriented surface for the maintenance worker. Each method
    // describes WHAT the consumer needs (find dupes, merge, decay-prune)
    // — backends implement HOW with whatever native primitive fits. Each
    // has a default returning empty/no-op so backends that haven't
    // implemented yet degrade gracefully (the sleep cycle just does
    // less work, doesn't crash).

    /// Find pairs of entities of the same `entity_type` whose name
    /// embeddings have cosine similarity >= `threshold`. Used by the
    /// Compactor to surface merge candidates. Returns up to `limit`
    /// pairs. Default: no candidates.
    async fn find_duplicate_candidates(
        &self,
        _agent_id: &str,
        _entity_type: &knowledge_graph::EntityType,
        _threshold: f32,
        _limit: usize,
    ) -> StoreResult<Vec<DuplicateCandidate>> {
        Ok(Vec::new())
    }

    /// Atomically merge `loser` into `winner`: re-target every
    /// relationship that pointed to `loser` so it points to `winner`,
    /// then mark `loser` as merged. Backend chooses the atomicity
    /// primitive (SQLite transaction, SurrealDB BEGIN/COMMIT block).
    /// Default: no-op error so misuse is loud.
    async fn merge_entity_into(&self, _loser: &EntityId, _winner: &EntityId) -> StoreResult<()> {
        Err(crate::StoreError::Backend(
            "merge_entity_into not implemented for this store".to_string(),
        ))
    }

    /// Find entities that are orphans (zero in/out edges) AND old
    /// (`last_seen_at` older than `min_age_days`). Used by the
    /// DecayEngine to surface prune candidates. Excludes already-
    /// archived entities. Default: no candidates.
    async fn list_orphan_old_candidates(
        &self,
        _agent_id: &str,
        _min_age_days: i64,
        _limit: usize,
    ) -> StoreResult<Vec<DecayCandidate>> {
        Ok(Vec::new())
    }

    /// Soft-delete an entity by marking it pruned. Distinct sentinel
    /// from `mark_entity_archival` so operators can tell decay-driven
    /// prunes apart from orphan archival. Used by the Pruner.
    /// Default: no-op error.
    async fn mark_entity_pruned(&self, _id: &EntityId) -> StoreResult<()> {
        Err(crate::StoreError::Backend(
            "mark_entity_pruned not implemented for this store".to_string(),
        ))
    }

    // ---- Sleep-time synthesis (Phase D4) ---------------------------------
    //
    // Reads needed by `Synthesizer` to surface cross-session strategy
    // candidates and load their context. Each method captures one
    // semantic operation; backends implement with whatever primitive
    // fits (SQLite uses JOINs over kg_entities × kg_relationships ×
    // kg_episodes, Surreal uses graph traversals over the `relationship`
    // edge table). Default: empty results so the synthesis cycle is a
    // no-op on backends that haven't implemented yet.

    /// Entities seen in at least `min_sessions` distinct sessions over
    /// the last `lookback_days`. Excludes archival/compressed rows.
    /// Used by the Synthesizer to find cross-session strategy
    /// candidates. Default: no candidates.
    async fn list_strategy_candidates(
        &self,
        _min_sessions: i64,
        _lookback_days: i64,
        _limit: usize,
    ) -> StoreResult<Vec<StrategyCandidate>> {
        Ok(Vec::new())
    }

    /// Relationship summaries (`src --[type]--> tgt` strings) for an
    /// entity, plus the distinct `session_id`s that referenced any of
    /// those relationships within the last `lookback_days`. Used by the
    /// Synthesizer to build per-candidate LLM context. Default: empty.
    async fn relationship_context_for_entity(
        &self,
        _entity_id: &str,
        _lookback_days: i64,
        _edge_limit: usize,
    ) -> StoreResult<RelationshipContext> {
        Ok(RelationshipContext::default())
    }

    /// Distinct, deduped episode ids that touched the entity within
    /// the last `lookback_days`. Used by the Synthesizer to attribute
    /// a synthesized fact to its contributing episodes. Default: empty.
    async fn episode_ids_for_entity(
        &self,
        _entity_id: &str,
        _lookback_days: i64,
    ) -> StoreResult<Vec<String>> {
        Ok(Vec::new())
    }
}
