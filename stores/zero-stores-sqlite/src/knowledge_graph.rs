use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_runtime::llm::EmbeddingClient;
use async_trait::async_trait;
use knowledge_graph::types::{
    Direction as KgDirection, Entity, EntityType, GraphStats, NeighborInfo, Relationship, Subgraph,
};

use crate::kg::storage::{ArchivableEntityRow, GraphStorage};
use zero_stores::error::StoreError;
use zero_stores::extracted::ExtractedKnowledge;
use zero_stores::types::{
    ArchivableEntity, Direction, EntityId, KgStats, Neighbor, ReindexReport, RelationshipId,
    ResolveOutcome, StoreOutcome, TraversalHit, VecIndexHealth,
};
use zero_stores::KnowledgeGraphStore;
use zero_stores::StoreResult;

use crate::blocking::{block, map_graph_err};
use crate::reindex;

/// SQLite implementation of `KnowledgeGraphStore`. Wraps the existing
/// `crate::kg::storage::GraphStorage` and bridges its synchronous
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

    async fn get_entity_by_name(&self, agent_id: &str, name: &str) -> StoreResult<Option<Entity>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let name = name.to_string();
        block(move || {
            storage
                .get_entity_by_name(&agent_id, &name)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn search_entities_view(
        &self,
        agent_id: &str,
        query: &str,
        view: zero_stores::GraphView,
        limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        let service = crate::kg::service::GraphService::new(self.storage.clone());
        service
            .search_entities_view(agent_id, query, view, limit)
            .await
            .map_err(map_graph_err)
    }

    async fn search_entities_by_name_embedding(
        &self,
        agent_id: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> StoreResult<Vec<zero_stores::EntityNameEmbeddingHit>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let query_embedding = query_embedding.to_vec();
        block(move || {
            storage
                .search_entities_by_name_embedding(&query_embedding, top_k, &agent_id)
                .map(|rows| {
                    rows.into_iter()
                        .map(
                            |(name, entity_type, distance)| zero_stores::EntityNameEmbeddingHit {
                                name,
                                entity_type,
                                distance,
                            },
                        )
                        .collect()
                })
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

    async fn decay_entity_confidence(
        &self,
        agent_id: &str,
        half_life_days: f64,
        min_confidence: f64,
        skip_recent_hours: i64,
    ) -> StoreResult<u64> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        block(move || {
            decay_kg_table(
                &storage,
                "kg_entities",
                &agent_id,
                half_life_days,
                min_confidence,
                skip_recent_hours,
            )
        })
        .await
    }

    async fn decay_relationship_confidence(
        &self,
        agent_id: &str,
        half_life_days: f64,
        min_confidence: f64,
        skip_recent_hours: i64,
    ) -> StoreResult<u64> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        block(move || {
            decay_kg_table(
                &storage,
                "kg_relationships",
                &agent_id,
                half_life_days,
                min_confidence,
                skip_recent_hours,
            )
        })
        .await
    }

    // -------- HTTP read paths -----------------------------------------------

    async fn graph_stats(&self, agent_id: &str) -> StoreResult<GraphStats> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        block(move || compute_graph_stats(&storage, &agent_id)).await
    }

    async fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Entity>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let entity_type = entity_type.map(|s| s.to_string());
        block(move || {
            storage
                .list_entities(&agent_id, entity_type.as_deref(), limit, offset)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Relationship>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let relationship_type = relationship_type.map(|s| s.to_string());
        block(move || {
            storage
                .list_relationships(&agent_id, relationship_type.as_deref(), limit, offset)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn get_neighbors_full(
        &self,
        agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> StoreResult<Vec<NeighborInfo>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let entity_id = entity_id.to_string();
        block(move || {
            storage
                .get_neighbors(&agent_id, &entity_id, direction.into(), limit)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn get_subgraph(
        &self,
        agent_id: &str,
        center_entity_id: &str,
        max_hops: usize,
    ) -> StoreResult<Subgraph> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let center = center_entity_id.to_string();
        block(move || compute_subgraph(&storage, &agent_id, &center, max_hops)).await
    }

    async fn count_all_entities(&self) -> StoreResult<usize> {
        let storage = self.storage.clone();
        block(move || storage.count_all_entities().map_err(map_graph_err)).await
    }

    async fn count_all_relationships(&self) -> StoreResult<usize> {
        let storage = self.storage.clone();
        block(move || storage.count_all_relationships().map_err(map_graph_err)).await
    }

    async fn list_all_entities(
        &self,
        ward_id: Option<&str>,
        entity_type: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<Entity>> {
        let storage = self.storage.clone();
        let ward_id = ward_id.map(|s| s.to_string());
        let entity_type = entity_type.map(|s| s.to_string());
        block(move || {
            storage
                .list_all_entities(ward_id.as_deref(), entity_type.as_deref(), limit)
                .map_err(map_graph_err)
        })
        .await
    }

    async fn list_all_relationships(&self, limit: usize) -> StoreResult<Vec<Relationship>> {
        let storage = self.storage.clone();
        block(move || storage.list_all_relationships(limit).map_err(map_graph_err)).await
    }

    async fn vec_index_health(&self) -> StoreResult<VecIndexHealth> {
        // SQLite-vec maintains an aux `<table>_rowids` table per index;
        // counting its rows is the faithful "indexed row count" used by
        // the embeddings health endpoint. This SQL is intentionally
        // SQLite-specific and stays inside the impl crate — the trait
        // surface stays portable.
        let db = self.storage.knowledge_db().clone();
        block(move || {
            const ROWID_TABLES: &[&str] = &[
                "memory_facts_index_rowids",
                "kg_name_index_rowids",
                "session_episodes_index_rowids",
            ];
            let (tables_present, tables_missing) = db
                .with_connection(|conn| Ok(crate::list_vec_table_presence(conn)))
                .map_err(|e| StoreError::Backend(format!("vec_index_health: {e}")))?;
            let indexed_rows = db
                .with_connection(|conn| {
                    let mut total = 0usize;
                    for tbl in ROWID_TABLES {
                        let n: i64 = conn
                            .query_row(&format!("SELECT count(*) FROM {tbl}"), [], |r| r.get(0))
                            .unwrap_or(0);
                        total = total.saturating_add(n.max(0) as usize);
                    }
                    Ok(total)
                })
                .unwrap_or(0);
            Ok(VecIndexHealth {
                tables_present,
                tables_missing,
                indexed_rows,
            })
        })
        .await
    }

    // ---- Sleep-time maintenance (Phase D2) -------------------------------

    async fn find_duplicate_candidates(
        &self,
        agent_id: &str,
        entity_type: &knowledge_graph::EntityType,
        threshold: f32,
        limit: usize,
    ) -> StoreResult<Vec<zero_stores::DuplicateCandidate>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        let type_str = entity_type.as_str().to_string();
        block(move || {
            storage
                .find_duplicate_candidates(&agent_id, &type_str, threshold, limit)
                .map(|pairs| {
                    pairs
                        .into_iter()
                        .map(
                            |(loser_id, winner_id, score)| zero_stores::DuplicateCandidate {
                                loser_entity_id: loser_id,
                                winner_entity_id: winner_id,
                                cosine_similarity: score,
                            },
                        )
                        .collect()
                })
                .map_err(map_graph_err)
        })
        .await
    }

    async fn merge_entity_into(&self, loser: &EntityId, winner: &EntityId) -> StoreResult<()> {
        let storage = self.storage.clone();
        let loser_id = loser.0.clone();
        let winner_id = winner.0.clone();
        block(move || {
            storage
                .merge_entity_into(&loser_id, &winner_id)
                .map(|_| ())
                .map_err(map_graph_err)
        })
        .await
    }

    async fn list_orphan_old_candidates(
        &self,
        agent_id: &str,
        min_age_days: i64,
        limit: usize,
    ) -> StoreResult<Vec<zero_stores::DecayCandidate>> {
        let storage = self.storage.clone();
        let agent_id = agent_id.to_string();
        block(move || {
            storage
                .list_orphan_old_candidates(&agent_id, min_age_days, limit)
                .map(|rows| {
                    rows.into_iter()
                        .map(|c| zero_stores::DecayCandidate {
                            id: c.id,
                            name: c.name,
                            entity_type: c.entity_type,
                            mention_count: c.mention_count,
                        })
                        .collect()
                })
                .map_err(map_graph_err)
        })
        .await
    }

    async fn mark_entity_pruned(&self, id: &EntityId) -> StoreResult<()> {
        let storage = self.storage.clone();
        let entity_id = id.0.clone();
        block(move || storage.mark_pruned(&entity_id).map_err(map_graph_err)).await
    }

    // ---- Sleep-time synthesis (Phase D4) -------------------------------

    async fn list_strategy_candidates(
        &self,
        min_sessions: i64,
        lookback_days: i64,
        limit: usize,
    ) -> StoreResult<Vec<zero_stores::StrategyCandidate>> {
        let db = self.storage.knowledge_db().clone();
        let limit_i64 = limit as i64;
        block(move || {
            db.with_connection(|conn| {
                // SQLite `datetime()` doesn't take bound days directly, so format
                // the modifier into the SQL literal. Both `min_sessions` and
                // `limit` stay parameterised.
                let date_modifier = format!("-{lookback_days} days");
                let sql = format!(
                    "SELECT e.id, e.agent_id, e.name, e.entity_type, COUNT(DISTINCT ep.session_id) AS n_sessions
                     FROM kg_entities e
                     JOIN kg_relationships r ON r.source_entity_id = e.id OR r.target_entity_id = e.id
                     JOIN kg_episodes ep    ON instr(COALESCE(r.source_episode_ids, ''), ep.id) > 0
                     WHERE e.epistemic_class != 'archival'
                       AND e.compressed_into IS NULL
                       AND ep.created_at > datetime('now', '{date_modifier}')
                       AND ep.session_id IS NOT NULL
                     GROUP BY e.id
                     HAVING n_sessions >= ?1
                     ORDER BY n_sessions DESC, e.mention_count DESC
                     LIMIT ?2"
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(rusqlite::params![min_sessions, limit_i64], |row| {
                        Ok(zero_stores::StrategyCandidate {
                            entity_id: row.get(0)?,
                            agent_id: row.get(1)?,
                            name: row.get(2)?,
                            entity_type: row.get(3)?,
                            n_sessions: row.get::<_, i64>(4)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .map_err(StoreError::Backend)
        })
        .await
    }

    async fn relationship_context_for_entity(
        &self,
        entity_id: &str,
        lookback_days: i64,
        edge_limit: usize,
    ) -> StoreResult<zero_stores::RelationshipContext> {
        let db = self.storage.knowledge_db().clone();
        let entity_id = entity_id.to_string();
        let edge_limit_i64 = edge_limit as i64;
        block(move || {
            db.with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT r.relationship_type, r.source_entity_id, r.target_entity_id,
                            r.source_episode_ids
                     FROM kg_relationships r
                     WHERE r.source_entity_id = ?1 OR r.target_entity_id = ?1
                     LIMIT ?2",
                )?;
                let rows: Vec<(String, String, String, Option<String>)> = stmt
                    .query_map(rusqlite::params![entity_id, edge_limit_i64], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;

                let summaries: Vec<String> = rows
                    .iter()
                    .map(|(ty, src, tgt, _)| format!("{src} --[{ty}]--> {tgt}"))
                    .collect();

                let episode_id_blob: String = rows
                    .iter()
                    .filter_map(|(_, _, _, eids)| eids.clone())
                    .collect::<Vec<_>>()
                    .join(",");

                let mut session_ids: Vec<String> = Vec::new();
                if !episode_id_blob.is_empty() {
                    let date_modifier = format!("-{lookback_days} days");
                    let sql = format!(
                        "SELECT DISTINCT session_id FROM kg_episodes
                         WHERE session_id IS NOT NULL
                           AND instr(?1, id) > 0
                           AND created_at > datetime('now', '{date_modifier}')"
                    );
                    let mut q = conn.prepare(&sql)?;
                    session_ids = q
                        .query_map(rusqlite::params![episode_id_blob], |row| {
                            row.get::<_, String>(0)
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                }
                Ok(zero_stores::RelationshipContext {
                    summaries,
                    session_ids,
                })
            })
            .map_err(StoreError::Backend)
        })
        .await
    }

    async fn episode_ids_for_entity(
        &self,
        entity_id: &str,
        lookback_days: i64,
    ) -> StoreResult<Vec<String>> {
        let db = self.storage.knowledge_db().clone();
        let entity_id = entity_id.to_string();
        block(move || {
            db.with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT r.source_episode_ids FROM kg_relationships r
                     WHERE (r.source_entity_id = ?1 OR r.target_entity_id = ?1)
                       AND r.source_episode_ids IS NOT NULL",
                )?;
                let blobs = stmt
                    .query_map(rusqlite::params![entity_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                let mut ids: Vec<String> = blobs
                    .iter()
                    .flat_map(|b| b.split(',').map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .collect();
                ids.sort();
                ids.dedup();
                if ids.is_empty() {
                    return Ok(ids);
                }
                let placeholders = vec!["?"; ids.len()].join(",");
                let date_modifier = format!("-{lookback_days} days");
                let sql = format!(
                    "SELECT id FROM kg_episodes
                     WHERE id IN ({placeholders})
                       AND session_id IS NOT NULL
                       AND created_at > datetime('now', '{date_modifier}')"
                );
                let mut q = conn.prepare(&sql)?;
                let params_vec: Vec<&dyn rusqlite::types::ToSql> = ids
                    .iter()
                    .map(|s| s as &dyn rusqlite::types::ToSql)
                    .collect();
                let filtered: Vec<String> = q
                    .query_map(params_vec.as_slice(), |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(filtered)
            })
            .map_err(StoreError::Backend)
        })
        .await
    }
}

// ============================================================================
// Helpers — extracted from the trait body so each impl method stays small.
// ============================================================================

/// Compute the rich stats view used by `GET /api/graph/:agent_id/stats`.
/// Mirrors the historical `GraphService::get_stats` — port lives here so
/// the handler can call through the trait without depending on
/// `GraphService`. Synchronous; runs inside a `spawn_blocking`.
fn compute_graph_stats(storage: &GraphStorage, agent_id: &str) -> StoreResult<GraphStats> {
    let entity_count = storage.count_entities(agent_id).map_err(map_graph_err)?;
    let relationship_count = storage
        .count_relationships(agent_id)
        .map_err(map_graph_err)?;

    let entities = storage
        .list_entities(agent_id, None, 10_000, 0)
        .map_err(map_graph_err)?;
    let mut entity_types: HashMap<String, usize> = HashMap::new();
    for entity in &entities {
        *entity_types
            .entry(entity.entity_type.as_str().to_string())
            .or_default() += 1;
    }

    let relationships = storage
        .list_relationships(agent_id, None, 10_000, 0)
        .map_err(map_graph_err)?;
    let mut relationship_types: HashMap<String, usize> = HashMap::new();
    let mut entity_connections: HashMap<String, usize> = HashMap::new();
    for rel in &relationships {
        *relationship_types
            .entry(rel.relationship_type.as_str().to_string())
            .or_default() += 1;
        *entity_connections
            .entry(rel.source_entity_id.clone())
            .or_default() += 1;
        *entity_connections
            .entry(rel.target_entity_id.clone())
            .or_default() += 1;
    }

    let entity_id_to_name: HashMap<&str, &str> = entities
        .iter()
        .map(|e| (e.id.as_str(), e.name.as_str()))
        .collect();

    let mut connection_vec: Vec<(String, usize)> = entity_connections
        .into_iter()
        .filter_map(|(id, count)| {
            entity_id_to_name
                .get(id.as_str())
                .map(|name| (name.to_string(), count))
        })
        .collect();
    connection_vec.sort_by(|a, b| b.1.cmp(&a.1));
    connection_vec.truncate(10);

    Ok(GraphStats {
        entity_count,
        relationship_count,
        entity_types,
        relationship_types,
        most_connected_entities: connection_vec,
    })
}

/// BFS subgraph centered on `center_entity_id` out to `max_hops`.
/// Mirrors the historical `GraphService::get_subgraph`.
fn compute_subgraph(
    storage: &GraphStorage,
    agent_id: &str,
    center_entity_id: &str,
    max_hops: usize,
) -> StoreResult<Subgraph> {
    let mut visited_entities: HashSet<String> = HashSet::new();
    let mut visited_relationships: HashSet<String> = HashSet::new();
    let mut entities: Vec<Entity> = Vec::new();
    let mut relationships: Vec<Relationship> = Vec::new();

    let mut current_hop: Vec<String> = vec![center_entity_id.to_string()];
    visited_entities.insert(center_entity_id.to_string());

    for _hop in 0..max_hops {
        if current_hop.is_empty() {
            break;
        }
        let mut next_hop: Vec<String> = Vec::new();
        for entity_id in &current_hop {
            let neighbors = storage
                .get_neighbors(agent_id, entity_id, KgDirection::Both, 1_000)
                .map_err(map_graph_err)?;
            collect_subgraph_neighbors(
                neighbors,
                &mut visited_entities,
                &mut visited_relationships,
                &mut entities,
                &mut relationships,
                &mut next_hop,
            );
        }
        current_hop = next_hop;
    }

    // Insert the center entity at the front, matching the historical shape.
    let all_entities = storage
        .list_entities(agent_id, None, 10_000, 0)
        .map_err(map_graph_err)?;
    if let Some(center) = all_entities.into_iter().find(|e| e.id == center_entity_id) {
        entities.insert(0, center);
    }

    Ok(Subgraph {
        entities,
        relationships,
        center: center_entity_id.to_string(),
        max_hops,
    })
}

/// Batch-apply temporal confidence decay to a KG table.
///
/// Both `kg_entities` and `kg_relationships` share the same schema for
/// `id`, `agent_id`, `confidence`, `last_seen_at`, and `epistemic_class`,
/// so one helper covers both. We compute the per-row multiplier in Rust
/// (`new = max(min_confidence, old * 0.5^(days / half_life))`) and apply
/// via individual UPDATEs inside a single connection — this avoids
/// depending on SQLite's optional `exp()` math extension and keeps the
/// math portable.
fn decay_kg_table(
    storage: &Arc<crate::kg::storage::GraphStorage>,
    table: &str,
    agent_id: &str,
    half_life_days: f64,
    min_confidence: f64,
    skip_recent_hours: i64,
) -> StoreResult<u64> {
    if half_life_days <= 0.0 {
        return Err(StoreError::Invalid(
            "half_life_days must be > 0".to_string(),
        ));
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(skip_recent_hours);
    let cutoff_rfc = cutoff.to_rfc3339();
    let decay_constant = std::f64::consts::LN_2 / half_life_days;
    let now = chrono::Utc::now();

    let table = table.to_string();
    let agent_id = agent_id.to_string();

    storage
        .knowledge_db()
        .with_connection(move |conn| {
            let tx = conn.unchecked_transaction()?;

            let select_sql = format!(
                "SELECT id, confidence, last_seen_at FROM {table}
                 WHERE agent_id = ?1
                   AND epistemic_class != 'archival'
                   AND last_seen_at < ?2"
            );
            let mut stmt = tx.prepare(&select_sql)?;
            let rows: Vec<(String, f64, String)> = stmt
                .query_map(rusqlite::params![agent_id, cutoff_rfc], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);

            let update_sql = format!("UPDATE {table} SET confidence = ?1 WHERE id = ?2");
            let mut update = tx.prepare(&update_sql)?;
            let mut total_updated: u64 = 0;
            // TODO: Replace this O(N) per-row UPDATE loop with a single bulk
            // UPDATE using `exp()` once we confirm SQLite's math extension is
            // always built into our rusqlite. For now we compute in Rust for
            // portability.
            for (id, old_conf, last_seen) in rows {
                let last_seen_dt = match chrono::DateTime::parse_from_rfc3339(&last_seen) {
                    Ok(dt) => dt.with_timezone(&chrono::Utc),
                    Err(_) => continue,
                };
                let days = (now - last_seen_dt).num_seconds() as f64 / 86_400.0;
                if days <= 0.0 {
                    continue;
                }
                let new_conf = (old_conf * (-decay_constant * days).exp()).max(min_confidence);
                if (new_conf - old_conf).abs() < 1e-6 {
                    continue;
                }
                update.execute(rusqlite::params![new_conf, id])?;
                total_updated += 1;
            }
            drop(update);
            tx.commit()?;
            Ok(total_updated)
        })
        .map_err(StoreError::Backend)
}

/// Process one hop's worth of neighbors into the BFS accumulators.
fn collect_subgraph_neighbors(
    neighbors: Vec<NeighborInfo>,
    visited_entities: &mut HashSet<String>,
    visited_relationships: &mut HashSet<String>,
    entities: &mut Vec<Entity>,
    relationships: &mut Vec<Relationship>,
    next_hop: &mut Vec<String>,
) {
    for neighbor in neighbors {
        if !visited_relationships.contains(&neighbor.relationship.id) {
            visited_relationships.insert(neighbor.relationship.id.clone());
            relationships.push(neighbor.relationship);
        }
        let neighbor_id = neighbor.entity.id.clone();
        if !visited_entities.contains(&neighbor_id) {
            visited_entities.insert(neighbor_id.clone());
            entities.push(neighbor.entity);
            next_hop.push(neighbor_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_db::KnowledgeDatabase;
    use gateway_services::VaultPaths;

    #[tokio::test]
    async fn decay_entity_confidence_reduces_old_entities() {
        use chrono::Duration as ChronoDuration;
        let tmp = tempfile::tempdir().unwrap();
        let paths = std::sync::Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        let db = std::sync::Arc::new(KnowledgeDatabase::new(paths).expect("db"));
        let graph = std::sync::Arc::new(crate::kg::storage::GraphStorage::new(db).expect("graph"));
        let store = SqliteKgStore::new(graph.clone());

        let agent_id = "agent-decay-test";
        let stale_time = chrono::Utc::now() - ChronoDuration::days(180); // 2 half-lives at 90d
        let fresh_time = chrono::Utc::now();

        // Insert one stale + one fresh entity directly via SQL for full control.
        graph
            .knowledge_db()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO kg_entities
                        (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                         epistemic_class, confidence, mention_count, access_count,
                         first_seen_at, last_seen_at)
                     VALUES ('stale', ?1, 'Concept', 'Stale', 'stale', 'h1',
                             'current', 0.8, 1, 0, ?2, ?2)",
                    rusqlite::params![agent_id, stale_time.to_rfc3339()],
                )?;
                conn.execute(
                    "INSERT INTO kg_entities
                        (id, agent_id, entity_type, name, normalized_name, normalized_hash,
                         epistemic_class, confidence, mention_count, access_count,
                         first_seen_at, last_seen_at)
                     VALUES ('fresh', ?1, 'Concept', 'Fresh', 'fresh', 'h2',
                             'current', 0.8, 1, 0, ?2, ?2)",
                    rusqlite::params![agent_id, fresh_time.to_rfc3339()],
                )?;
                Ok(())
            })
            .unwrap();

        let updated = store
            .decay_entity_confidence(agent_id, 90.0, 0.01, 24)
            .await
            .expect("decay");
        assert_eq!(updated, 1, "exactly the stale entity should be decayed");

        // Verify stale confidence approximately halved twice
        // (2 half-lives → 0.25× of 0.8 = 0.2).
        let new_stale_conf: f64 = graph
            .knowledge_db()
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT confidence FROM kg_entities WHERE id = 'stale'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert!(
            (new_stale_conf - 0.2).abs() < 0.02,
            "stale conf {new_stale_conf} should be ~0.2"
        );

        // Fresh entity unchanged.
        let fresh_conf: f64 = graph
            .knowledge_db()
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT confidence FROM kg_entities WHERE id = 'fresh'",
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap();
        assert!(
            (fresh_conf - 0.8).abs() < 1e-6,
            "fresh conf should be unchanged"
        );
    }
}
