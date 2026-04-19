//! # Graph Storage
//!
//! SQLite storage for knowledge graph entities and relationships.

use crate::error::{GraphError, GraphResult};
use crate::types::{
    Direction, Entity, EntityType, ExtractedKnowledge, NeighborInfo, Relationship, RelationshipType,
};
use gateway_database::KnowledgeDatabase;
use rusqlite::{params, Connection};
use std::sync::Arc;

/// Convert a `GraphError` into a `rusqlite::Error` so that closures passed to
/// `KnowledgeDatabase::with_connection` (which require `rusqlite::Result`) can
/// propagate our higher-level errors. `with_connection` stringifies the result
/// anyway, so the round-trip fidelity is acceptable.
fn graph_to_rusqlite(e: GraphError) -> rusqlite::Error {
    match e {
        GraphError::Database(db) => db,
        other => {
            rusqlite::Error::InvalidColumnType(0, format!("{other:?}"), rusqlite::types::Type::Null)
        }
    }
}

/// SQLite-backed storage for the knowledge graph, sharing the pooled
/// `KnowledgeDatabase` connection for `knowledge.db` (schema v22).
pub struct GraphStorage {
    pub(crate) db: Arc<KnowledgeDatabase>,
}

impl GraphStorage {
    /// Create a new graph storage backed by the shared `KnowledgeDatabase` pool.
    ///
    /// Schema v22 is already initialized by `KnowledgeDatabase::new`, so this
    /// constructor is effectively a no-op wrapper.
    pub fn new(db: Arc<KnowledgeDatabase>) -> GraphResult<Self> {
        Ok(Self { db })
    }

    /// Store extracted knowledge (entities and relationships)
    pub fn store_knowledge(
        &self,
        agent_id: &str,
        knowledge: ExtractedKnowledge,
    ) -> GraphResult<()> {
        self.db
            .with_connection(|conn| {
                // Store entities and build ID mapping (new_id → actual_id)
                let mut entity_id_map: std::collections::HashMap<String, String> =
                    std::collections::HashMap::new();
                for entity in knowledge.entities {
                    let original_id = entity.id.clone();
                    let actual_id =
                        store_entity(conn, agent_id, entity).map_err(graph_to_rusqlite)?;
                    entity_id_map.insert(original_id, actual_id);
                }

                for mut relationship in knowledge.relationships {
                    if let Some(mapped) = entity_id_map.get(&relationship.source_entity_id) {
                        relationship.source_entity_id = mapped.clone();
                    }
                    if let Some(mapped) = entity_id_map.get(&relationship.target_entity_id) {
                        relationship.target_entity_id = mapped.clone();
                    }
                    store_relationship(conn, agent_id, relationship).map_err(graph_to_rusqlite)?;
                }

                Ok(())
            })
            .map_err(GraphError::Other)
    }

    /// Get all entities for an agent (includes __global__ entities)
    pub fn get_entities(&self, agent_id: &str) -> GraphResult<Vec<Entity>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Entity>> {
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities WHERE agent_id = ?1 OR agent_id = '__global__'"
        ).map_err(GraphError::Database)?;

        let rows = stmt
            .query_map(params![agent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(GraphError::Database)?;

        let mut entities = Vec::new();
        for row in rows {
            let (
                id,
                agent_id,
                entity_type_str,
                name,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row?;

            let entity_type = EntityType::from_str(&entity_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            entities.push(Entity {
                id,
                agent_id,
                entity_type,
                name,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
                name_embedding: None,
            });
        }

        Ok(entities)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Get all relationships for an agent (includes __global__ relationships)
    pub fn get_relationships(&self, agent_id: &str) -> GraphResult<Vec<Relationship>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Relationship>> {

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_relationships WHERE agent_id = ?1 OR agent_id = '__global__'"
        ).map_err(GraphError::Database)?;

        let rows = stmt
            .query_map(params![agent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .map_err(GraphError::Database)?;

        let mut relationships = Vec::new();
        for row in rows {
            let (
                id,
                agent_id,
                source_entity_id,
                target_entity_id,
                rel_type_str,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row?;

            let relationship_type = RelationshipType::from_str(&rel_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            relationships.push(Relationship {
                id,
                agent_id,
                source_entity_id,
                target_entity_id,
                relationship_type,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
            });
        }

        Ok(relationships)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Search entities by name (includes __global__ entities)
    pub fn search_entities(&self, agent_id: &str, query: &str) -> GraphResult<Vec<Entity>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Entity>> {

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND name LIKE ?2
             ORDER BY mention_count DESC"
        ).map_err(GraphError::Database)?;

        let pattern = format!("%{}%", query);
        let rows = stmt
            .query_map(params![agent_id, pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(GraphError::Database)?;

        let mut entities = Vec::new();
        for row in rows {
            let (
                id,
                agent_id,
                entity_type_str,
                name,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row?;

            let entity_type = EntityType::from_str(&entity_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            entities.push(Entity {
                id,
                agent_id,
                entity_type,
                name,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
                name_embedding: None,
            });
        }

        Ok(entities)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Search entities ordered by a caller-provided clause.
    ///
    /// `order_clause` is whitelisted to prevent SQL injection; any value
    /// outside the whitelist falls back to `mention_count DESC`.
    pub fn search_entities_order_by(
        &self,
        agent_id: &str,
        query: &str,
        order_clause: &str,
        limit: usize,
    ) -> GraphResult<Vec<Entity>> {
        // Whitelist to prevent SQL injection — only accept known safe clauses.
        let safe_order = match order_clause {
            "last_seen_at DESC" | "mention_count DESC" | "first_seen_at DESC" => order_clause,
            _ => "mention_count DESC",
        };

        let sql = format!(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND name LIKE ?2
             ORDER BY {}
             LIMIT ?3",
            safe_order
        );

        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Entity>> {
                    let mut stmt = conn.prepare(&sql).map_err(GraphError::Database)?;

                    let pattern = format!("%{}%", query);
                    let rows = stmt
                        .query_map(params![agent_id, pattern, limit as i64], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, Option<String>>(4)?,
                                row.get::<_, String>(5)?,
                                row.get::<_, String>(6)?,
                                row.get::<_, i64>(7)?,
                            ))
                        })
                        .map_err(GraphError::Database)?;

                    let mut entities = Vec::new();
                    for row in rows {
                        let (
                            id,
                            agent_id,
                            entity_type_str,
                            name,
                            properties_json,
                            first_seen_at,
                            last_seen_at,
                            mention_count,
                        ) = row?;

                        let entity_type = EntityType::from_str(&entity_type_str);
                        let properties = if let Some(json) = properties_json {
                            serde_json::from_str(&json).unwrap_or_default()
                        } else {
                            Default::default()
                        };

                        entities.push(Entity {
                            id,
                            agent_id,
                            entity_type,
                            name,
                            properties,
                            first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                                .map(|dt| dt.with_timezone(&chrono::Utc))
                                .unwrap_or_else(|_| chrono::Utc::now()),
                            last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                                .map(|dt| dt.with_timezone(&chrono::Utc))
                                .unwrap_or_else(|_| chrono::Utc::now()),
                            mention_count,
                            name_embedding: None,
                        });
                    }

                    Ok(entities)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Count relationships referencing a particular entity (source OR target).
    pub fn count_relationships_for(&self, entity_id: &str) -> GraphResult<i64> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<i64> {
                    let count: i64 = conn
                        .query_row(
                            "SELECT COUNT(*) FROM kg_relationships
                 WHERE source_entity_id = ?1 OR target_entity_id = ?1",
                            params![entity_id],
                            |row| row.get(0),
                        )
                        .map_err(GraphError::Database)?;
                    Ok(count)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Find an existing entity by agent_id + name (case-insensitive), returning its ID.
    pub fn find_entity_by_name(&self, agent_id: &str, name: &str) -> GraphResult<Option<String>> {
        self.db
            .with_connection(|conn| {
                find_entity_by_name(conn, agent_id, name).map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// ANN search over `kg_name_index` (sqlite-vec virtual table) for the
    /// nearest entity names to `query_embedding`.
    ///
    /// Returns tuples of `(name, entity_type, distance)` where `distance` is
    /// the L2-squared distance from the vec0 index. For L2-normalized vectors,
    /// cosine similarity ≈ `1 - distance / 2`. The query embedding MUST be
    /// L2-normalized by the caller. Results are filtered to the caller's
    /// agent plus `__global__` and ordered by ascending distance.
    pub fn search_entities_by_name_embedding(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        agent_id: &str,
    ) -> GraphResult<Vec<(String, String, f32)>> {
        if query_embedding.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }
        let embedding_json = serde_json::to_string(query_embedding)
            .map_err(|e| GraphError::Other(format!("serialize query embedding: {e}")))?;

        // vec0 KNN queries require a bare `k = ?` or `LIMIT ?` on the virtual
        // table itself — extra predicates/joins aren't accepted at prepare
        // time. Pull top-K by distance, then filter against `kg_entities`.
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<(String, String, f32)>> {
                    let mut ann_stmt = conn
                        .prepare(
                            "SELECT entity_id, distance \
                             FROM kg_name_index \
                             WHERE name_embedding MATCH ?1 \
                             ORDER BY distance \
                             LIMIT ?2",
                        )
                        .map_err(GraphError::Database)?;
                    let hits = ann_stmt
                        .query_map(params![embedding_json, top_k as i64], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
                        })
                        .map_err(GraphError::Database)?;

                    let mut lookup_stmt = conn
                        .prepare(
                            "SELECT name, entity_type FROM kg_entities \
                             WHERE id = ?1 \
                               AND (agent_id = ?2 OR agent_id = '__global__')",
                        )
                        .map_err(GraphError::Database)?;

                    let mut out = Vec::new();
                    for row in hits {
                        let (entity_id, dist) = row.map_err(GraphError::Database)?;
                        let mut rows = lookup_stmt
                            .query(params![entity_id, agent_id])
                            .map_err(GraphError::Database)?;
                        if let Some(r) = rows.next().map_err(GraphError::Database)? {
                            let name: String = r.get(0).map_err(GraphError::Database)?;
                            let etype: String = r.get(1).map_err(GraphError::Database)?;
                            out.push((name, etype, dist));
                        }
                    }
                    Ok(out)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Increment mention count and update last_seen for an existing entity.
    pub fn bump_entity_mention(&self, entity_id: &str) -> GraphResult<()> {
        self.db
            .with_connection(|conn| bump_entity_mention(conn, entity_id).map_err(graph_to_rusqlite))
            .map_err(GraphError::Other)
    }

    /// Delete all data for an agent
    pub fn delete_agent_data(&self, agent_id: &str) -> GraphResult<usize> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<usize> {
                    // Delete relationships
                    let rel_count = conn
                        .execute(
                            "DELETE FROM kg_relationships WHERE agent_id = ?1",
                            params![agent_id],
                        )
                        .map_err(GraphError::Database)?;

                    // Delete entities
                    let ent_count = conn
                        .execute(
                            "DELETE FROM kg_entities WHERE agent_id = ?1",
                            params![agent_id],
                        )
                        .map_err(GraphError::Database)?;

                    Ok(rel_count + ent_count)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    // ===== NEW READ METHODS (Phase 1: Graph Repository Layer) =====

    /// List entities for an agent with optional type filter and pagination
    pub fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> GraphResult<Vec<Entity>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Entity>> {

        // Build query and params based on whether type filter is provided
        let sql = if entity_type.is_some() {
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND entity_type = ?2
             ORDER BY mention_count DESC
             LIMIT ?3 OFFSET ?4"
        } else {
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             WHERE agent_id = ?1 OR agent_id = '__global__'
             ORDER BY mention_count DESC
             LIMIT ?2 OFFSET ?3"
        };

        let mut stmt = conn.prepare(sql).map_err(GraphError::Database)?;

        // Helper to parse entity from row
        let parse_entity = |row: &rusqlite::Row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        };

        let rows = if let Some(type_filter) = entity_type {
            stmt.query_map(
                params![agent_id, type_filter, limit as i64, offset as i64],
                parse_entity,
            )
            .map_err(GraphError::Database)?
        } else {
            stmt.query_map(params![agent_id, limit as i64, offset as i64], parse_entity)
                .map_err(GraphError::Database)?
        };

        let mut entities = Vec::new();
        for row_result in rows {
            let (
                id,
                agent_id,
                entity_type_str,
                name,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row_result?;

            let entity_type = EntityType::from_str(&entity_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            entities.push(Entity {
                id,
                agent_id,
                entity_type,
                name,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
                name_embedding: None,
            });
        }

        Ok(entities)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// List relationships for an agent with optional type filter and pagination
    pub fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> GraphResult<Vec<Relationship>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Relationship>> {

        // Build query based on whether type filter is provided
        let sql = if relationship_type.is_some() {
            "SELECT id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_relationships
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND relationship_type = ?2
             ORDER BY mention_count DESC
             LIMIT ?3 OFFSET ?4"
        } else {
            "SELECT id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_relationships
             WHERE agent_id = ?1 OR agent_id = '__global__'
             ORDER BY mention_count DESC
             LIMIT ?2 OFFSET ?3"
        };

        let mut stmt = conn.prepare(sql).map_err(GraphError::Database)?;

        // Helper to parse relationship from row
        let parse_relationship = |row: &rusqlite::Row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
            ))
        };

        let rows = if let Some(type_filter) = relationship_type {
            stmt.query_map(
                params![agent_id, type_filter, limit as i64, offset as i64],
                parse_relationship,
            )
            .map_err(GraphError::Database)?
        } else {
            stmt.query_map(
                params![agent_id, limit as i64, offset as i64],
                parse_relationship,
            )
            .map_err(GraphError::Database)?
        };

        let mut relationships = Vec::new();
        for row_result in rows {
            let (
                id,
                agent_id,
                source_entity_id,
                target_entity_id,
                rel_type_str,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row_result?;

            let relationship_type = RelationshipType::from_str(&rel_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            relationships.push(Relationship {
                id,
                agent_id,
                source_entity_id,
                target_entity_id,
                relationship_type,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
            });
        }

        Ok(relationships)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Get entity by name (case-insensitive)
    pub fn get_entity_by_name(&self, agent_id: &str, name: &str) -> GraphResult<Option<Entity>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Option<Entity>> {

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             WHERE (agent_id = ?1 OR agent_id = '__global__') AND name = ?2 COLLATE NOCASE
             LIMIT 1"
        ).map_err(GraphError::Database)?;

        let lower_name = name.to_lowercase();
        let mut rows = stmt
            .query_map(params![agent_id, lower_name], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(GraphError::Database)?;

        if let Some(row) = rows.next() {
            let (
                id,
                agent_id,
                entity_type_str,
                name,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row?;

            let entity_type = EntityType::from_str(&entity_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            Ok(Some(Entity {
                id,
                agent_id,
                entity_type,
                name,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
                name_embedding: None,
            }))
        } else {
            Ok(None)
        }
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Get neighbors of an entity (1-hop)
    pub fn get_neighbors(
        &self,
        _agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> GraphResult<Vec<NeighborInfo>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<NeighborInfo>> {

        let mut neighbors = Vec::new();

        // Get outgoing neighbors (Entity → Other)
        if direction == Direction::Outgoing || direction == Direction::Both {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.agent_id, e.entity_type, e.name, e.properties, e.first_seen_at, e.last_seen_at, e.mention_count,
                        r.id, r.agent_id, r.source_entity_id, r.target_entity_id, r.relationship_type, r.properties, r.first_seen_at, r.last_seen_at, r.mention_count
                 FROM kg_entities e
                 INNER JOIN kg_relationships r ON r.target_entity_id = e.id
                 WHERE r.source_entity_id = ?1
                 ORDER BY r.mention_count DESC
                 LIMIT ?2"
            ).map_err(GraphError::Database)?;

            let rows = stmt
                .query_map(params![entity_id, limit as i64], |row| {
                    Ok((
                        // Entity fields
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        // Relationship fields
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, i64>(16)?,
                    ))
                })
                .map_err(GraphError::Database)?;

            for row in rows {
                let (
                    e_id,
                    e_agent_id,
                    e_type_str,
                    e_name,
                    e_props_json,
                    e_first,
                    e_last,
                    e_mentions,
                    r_id,
                    r_agent_id,
                    r_source,
                    r_target,
                    r_type_str,
                    r_props_json,
                    r_first,
                    r_last,
                    r_mentions,
                ) = row?;

                let entity = Entity {
                    id: e_id,
                    agent_id: e_agent_id,
                    entity_type: EntityType::from_str(&e_type_str),
                    name: e_name,
                    properties: e_props_json
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default(),
                    first_seen_at: chrono::DateTime::parse_from_rfc3339(&e_first)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_seen_at: chrono::DateTime::parse_from_rfc3339(&e_last)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    mention_count: e_mentions,
                    name_embedding: None,
                };

                let relationship = Relationship {
                    id: r_id,
                    agent_id: r_agent_id,
                    source_entity_id: r_source,
                    target_entity_id: r_target,
                    relationship_type: RelationshipType::from_str(&r_type_str),
                    properties: r_props_json
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default(),
                    first_seen_at: chrono::DateTime::parse_from_rfc3339(&r_first)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_seen_at: chrono::DateTime::parse_from_rfc3339(&r_last)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    mention_count: r_mentions,
                };

                neighbors.push(NeighborInfo {
                    entity,
                    relationship,
                    direction: Direction::Outgoing,
                });
            }
        }

        // Get incoming neighbors (Other → Entity)
        if direction == Direction::Incoming || direction == Direction::Both {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.agent_id, e.entity_type, e.name, e.properties, e.first_seen_at, e.last_seen_at, e.mention_count,
                        r.id, r.agent_id, r.source_entity_id, r.target_entity_id, r.relationship_type, r.properties, r.first_seen_at, r.last_seen_at, r.mention_count
                 FROM kg_entities e
                 INNER JOIN kg_relationships r ON r.source_entity_id = e.id
                 WHERE r.target_entity_id = ?1
                 ORDER BY r.mention_count DESC
                 LIMIT ?2"
            ).map_err(GraphError::Database)?;

            let rows = stmt
                .query_map(params![entity_id, limit as i64], |row| {
                    Ok((
                        // Entity fields
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        // Relationship fields
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, i64>(16)?,
                    ))
                })
                .map_err(GraphError::Database)?;

            for row in rows {
                let (
                    e_id,
                    e_agent_id,
                    e_type_str,
                    e_name,
                    e_props_json,
                    e_first,
                    e_last,
                    e_mentions,
                    r_id,
                    r_agent_id,
                    r_source,
                    r_target,
                    r_type_str,
                    r_props_json,
                    r_first,
                    r_last,
                    r_mentions,
                ) = row?;

                let entity = Entity {
                    id: e_id,
                    agent_id: e_agent_id,
                    entity_type: EntityType::from_str(&e_type_str),
                    name: e_name,
                    properties: e_props_json
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default(),
                    first_seen_at: chrono::DateTime::parse_from_rfc3339(&e_first)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_seen_at: chrono::DateTime::parse_from_rfc3339(&e_last)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    mention_count: e_mentions,
                    name_embedding: None,
                };

                let relationship = Relationship {
                    id: r_id,
                    agent_id: r_agent_id,
                    source_entity_id: r_source,
                    target_entity_id: r_target,
                    relationship_type: RelationshipType::from_str(&r_type_str),
                    properties: r_props_json
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default(),
                    first_seen_at: chrono::DateTime::parse_from_rfc3339(&r_first)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_seen_at: chrono::DateTime::parse_from_rfc3339(&r_last)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    mention_count: r_mentions,
                };

                neighbors.push(NeighborInfo {
                    entity,
                    relationship,
                    direction: Direction::Incoming,
                });
            }
        }

        Ok(neighbors)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Count entities for an agent
    pub fn count_entities(&self, agent_id: &str) -> GraphResult<usize> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<usize> {
                    let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE agent_id = ?1 OR agent_id = '__global__'",
                params![agent_id],
                |row| row.get(0),
            )
            .map_err(GraphError::Database)?;

                    Ok(count as usize)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Count relationships for an agent
    pub fn count_relationships(&self, agent_id: &str) -> GraphResult<usize> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<usize> {
                    let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_relationships WHERE agent_id = ?1 OR agent_id = '__global__'",
            params![agent_id],
            |row| row.get(0),
        ).map_err(GraphError::Database)?;

                    Ok(count as usize)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Count all entities across all agents.
    pub fn count_all_entities(&self) -> GraphResult<usize> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<usize> {
                    let count: i64 = conn
                        .query_row("SELECT COUNT(*) FROM kg_entities", [], |row| row.get(0))
                        .map_err(GraphError::Database)?;
                    Ok(count as usize)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Count all relationships across all agents.
    pub fn count_all_relationships(&self) -> GraphResult<usize> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<usize> {
                    let count: i64 = conn
                        .query_row("SELECT COUNT(*) FROM kg_relationships", [], |row| {
                            row.get(0)
                        })
                        .map_err(GraphError::Database)?;
                    Ok(count as usize)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// List all relationships across all agents with pagination.
    ///
    /// Used by the Observatory "All Agents" mode to render edges.
    pub fn list_all_relationships(&self, limit: usize) -> GraphResult<Vec<Relationship>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Relationship>> {

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_relationships
             ORDER BY mention_count DESC
             LIMIT ?1"
        ).map_err(GraphError::Database)?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .map_err(GraphError::Database)?;

        let mut relationships = Vec::new();
        for row in rows {
            let (
                id,
                agent_id,
                source_entity_id,
                target_entity_id,
                rel_type_str,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row?;

            let relationship_type = RelationshipType::from_str(&rel_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            relationships.push(Relationship {
                id,
                agent_id,
                source_entity_id,
                target_entity_id,
                relationship_type,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
            });
        }

        Ok(relationships)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// List entities across all agents with optional filters and pagination.
    ///
    /// Used by the Observatory "All Agents" mode.
    pub fn list_all_entities(
        &self,
        ward_id: Option<&str>,
        entity_type: Option<&str>,
        limit: usize,
    ) -> GraphResult<Vec<Entity>> {
        self.db
            .with_connection(|conn| {
                (|| -> GraphResult<Vec<Entity>> {

        // Build dynamic SQL based on filters
        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(wid) = ward_id {
            conditions.push(format!("agent_id = ?{}", param_values.len() + 1));
            param_values.push(Box::new(wid.to_string()));
        }
        if let Some(et) = entity_type {
            conditions.push(format!("entity_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(et.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             {}
             ORDER BY mention_count DESC
             LIMIT ?{}",
            where_clause,
            param_values.len() + 1,
        );

        param_values.push(Box::new(limit as i64));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(GraphError::Database)?;

        let parse_entity = |row: &rusqlite::Row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        };

        let rows = stmt
            .query_map(params_refs.as_slice(), parse_entity)
            .map_err(GraphError::Database)?;

        let mut entities = Vec::new();
        for row_result in rows {
            let (
                id,
                agent_id,
                entity_type_str,
                name,
                properties_json,
                first_seen_at,
                last_seen_at,
                mention_count,
            ) = row_result?;

            let entity_type = EntityType::from_str(&entity_type_str);
            let properties = if let Some(json) = properties_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Default::default()
            };

            entities.push(Entity {
                id,
                agent_id,
                entity_type,
                name,
                properties,
                first_seen_at: chrono::DateTime::parse_from_rfc3339(&first_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                last_seen_at: chrono::DateTime::parse_from_rfc3339(&last_seen_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                mention_count,
                name_embedding: None,
            });
        }

        Ok(entities)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Find near-duplicate entity pairs of a given `entity_type` whose
    /// `kg_name_index` embeddings have cosine similarity ≥ `cosine_threshold`.
    ///
    /// For L2-normalized embeddings, `L2_sq = 2 * (1 - cosine)`, so
    /// `cosine ≥ τ  ⇔  L2_sq ≤ 2 * (1 - τ)`.
    ///
    /// Returns up to `limit` `(entity_id_a, entity_id_b, cosine)` tuples with
    /// `a < b` lexicographically (deduped). Archived and already-compressed
    /// entities are excluded on both sides.
    pub fn find_duplicate_candidates(
        &self,
        agent_id: &str,
        entity_type: &str,
        cosine_threshold: f32,
        limit: usize,
    ) -> GraphResult<Vec<(String, String, f32)>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let l2_threshold = 2.0 * (1.0 - cosine_threshold);
        let agent = agent_id.to_string();
        let etype = entity_type.to_string();
        self.db
            .with_connection(move |conn| {
                (|| -> GraphResult<Vec<(String, String, f32)>> {
                    // Pull candidate entity ids of this type that have an embedding
                    // indexed. We take 3x limit to give the ANN pass enough
                    // breadth before deduping.
                    let mut list_stmt = conn
                        .prepare(
                            "SELECT e.id FROM kg_entities e \
                             WHERE e.entity_type = ?1 \
                               AND (e.agent_id = ?2 OR e.agent_id = '__global__') \
                               AND e.epistemic_class != 'archival' \
                               AND (e.compressed_into IS NULL OR e.compressed_into = '') \
                             ORDER BY e.mention_count DESC \
                             LIMIT ?3",
                        )
                        .map_err(GraphError::Database)?;
                    let cap = (limit as i64).saturating_mul(3);
                    let ids: Vec<String> = list_stmt
                        .query_map(params![etype, agent, cap], |r| r.get::<_, String>(0))
                        .map_err(GraphError::Database)?
                        .collect::<rusqlite::Result<_>>()
                        .map_err(GraphError::Database)?;

                    let mut emb_stmt = conn
                        .prepare("SELECT name_embedding FROM kg_name_index WHERE entity_id = ?1")
                        .map_err(GraphError::Database)?;
                    let mut ann_stmt = conn
                        .prepare(
                            "SELECT entity_id, distance FROM kg_name_index \
                             WHERE name_embedding MATCH ?1 \
                             ORDER BY distance LIMIT 5",
                        )
                        .map_err(GraphError::Database)?;
                    let mut filter_stmt = conn
                        .prepare(
                            "SELECT 1 FROM kg_entities \
                             WHERE id = ?1 \
                               AND entity_type = ?2 \
                               AND epistemic_class != 'archival' \
                               AND (compressed_into IS NULL OR compressed_into = '')",
                        )
                        .map_err(GraphError::Database)?;

                    let mut pairs: Vec<(String, String, f32)> = Vec::new();
                    let mut seen_pairs = std::collections::HashSet::<(String, String)>::new();

                    for id in &ids {
                        // vec0 stores the vector as a packed f32 BLOB. We
                        // decode it and re-serialize as JSON for the MATCH
                        // input, since that's the form we know sqlite-vec
                        // accepts everywhere in this codebase.
                        let emb_blob: Option<Vec<u8>> = emb_stmt
                            .query_row(params![id], |r| r.get::<_, Vec<u8>>(0))
                            .ok();
                        let Some(emb_blob) = emb_blob else {
                            continue;
                        };
                        if emb_blob.len() % 4 != 0 {
                            continue;
                        }
                        let floats: Vec<f32> = emb_blob
                            .chunks_exact(4)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect();
                        let emb_json = serde_json::to_string(&floats)
                            .map_err(|e| GraphError::Other(format!("serialize embedding: {e}")))?;

                        let neighbors = ann_stmt
                            .query_map(params![emb_json], |r| {
                                Ok((r.get::<_, String>(0)?, r.get::<_, f32>(1)?))
                            })
                            .map_err(GraphError::Database)?;

                        for row in neighbors {
                            let (neighbor_id, dist) = row.map_err(GraphError::Database)?;
                            if neighbor_id == *id {
                                continue;
                            }
                            if dist > l2_threshold {
                                continue;
                            }
                            let ok: Option<i64> = filter_stmt
                                .query_row(params![neighbor_id, etype], |r| r.get(0))
                                .ok();
                            if ok.is_none() {
                                continue;
                            }

                            // Deduplicate unordered pair (a,b) == (b,a).
                            let key = if *id < neighbor_id {
                                (id.clone(), neighbor_id.clone())
                            } else {
                                (neighbor_id.clone(), id.clone())
                            };
                            if seen_pairs.insert(key.clone()) {
                                let cosine = 1.0 - (dist / 2.0);
                                pairs.push((key.0, key.1, cosine));
                            }
                        }
                        if pairs.len() >= limit {
                            break;
                        }
                    }

                    pairs
                        .sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
                    pairs.truncate(limit);
                    Ok(pairs)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }

    /// Merge `loser_id` into `winner_id` transactionally.
    ///
    /// 1. Drop relationships that would collide with an existing winner edge
    ///    after re-pointing (the `kg_relationships`
    ///    `UNIQUE(source_entity_id, target_entity_id, relationship_type)` would
    ///    otherwise abort the UPDATE).
    /// 2. Re-point remaining relationships from loser → winner.
    /// 3. Transfer aliases to winner (IGNORE on collision), then delete any
    ///    loser-side aliases left behind.
    /// 4. Mark `kg_entities.compressed_into = winner_id` for loser.
    /// 5. Remove loser's row from `kg_name_index` so it stops surfacing in ANN.
    pub fn merge_entity_into(&self, loser_id: &str, winner_id: &str) -> GraphResult<MergeResult> {
        let loser = loser_id.to_string();
        let winner = winner_id.to_string();
        self.db
            .with_connection(move |conn| {
                (|| -> GraphResult<MergeResult> {
                    let tx = conn.unchecked_transaction().map_err(GraphError::Database)?;

                    // 1. Drop would-be-duplicate relationships before re-pointing.
                    let dups_out = tx
                        .execute(
                            "DELETE FROM kg_relationships \
                             WHERE source_entity_id = ?1 \
                               AND EXISTS ( \
                                 SELECT 1 FROM kg_relationships w \
                                 WHERE w.source_entity_id = ?2 \
                                   AND w.target_entity_id = kg_relationships.target_entity_id \
                                   AND w.relationship_type = kg_relationships.relationship_type \
                               )",
                            params![loser, winner],
                        )
                        .map_err(GraphError::Database)? as u64;
                    let dups_in = tx
                        .execute(
                            "DELETE FROM kg_relationships \
                             WHERE target_entity_id = ?1 \
                               AND EXISTS ( \
                                 SELECT 1 FROM kg_relationships w \
                                 WHERE w.target_entity_id = ?2 \
                                   AND w.source_entity_id = kg_relationships.source_entity_id \
                                   AND w.relationship_type = kg_relationships.relationship_type \
                               )",
                            params![loser, winner],
                        )
                        .map_err(GraphError::Database)? as u64;

                    // 2. Re-point remaining relationships.
                    let rep_out = tx
                        .execute(
                            "UPDATE kg_relationships SET source_entity_id = ?1 \
                             WHERE source_entity_id = ?2",
                            params![winner, loser],
                        )
                        .map_err(GraphError::Database)? as u64;
                    let rep_in = tx
                        .execute(
                            "UPDATE kg_relationships SET target_entity_id = ?1 \
                             WHERE target_entity_id = ?2",
                            params![winner, loser],
                        )
                        .map_err(GraphError::Database)? as u64;

                    // 3. Transfer aliases (IGNORE if winner already has the same
                    //    normalized_form) and clean up any remaining loser-side
                    //    alias rows that lost their UPDATE race.
                    let aliases = tx
                        .execute(
                            "UPDATE OR IGNORE kg_aliases SET entity_id = ?1 \
                             WHERE entity_id = ?2",
                            params![winner, loser],
                        )
                        .map_err(GraphError::Database)? as u64;
                    tx.execute(
                        "DELETE FROM kg_aliases WHERE entity_id = ?1",
                        params![loser],
                    )
                    .map_err(GraphError::Database)?;

                    // 4. Mark loser as compressed into winner.
                    tx.execute(
                        "UPDATE kg_entities SET compressed_into = ?1 WHERE id = ?2",
                        params![winner, loser],
                    )
                    .map_err(GraphError::Database)?;

                    // 5. Remove loser's vec0 row so ANN queries skip it.
                    tx.execute(
                        "DELETE FROM kg_name_index WHERE entity_id = ?1",
                        params![loser],
                    )
                    .map_err(GraphError::Database)?;

                    tx.commit().map_err(GraphError::Database)?;
                    Ok(MergeResult {
                        relationships_repointed: rep_out + rep_in,
                        aliases_transferred: aliases,
                        duplicate_relationships_dropped: dups_out + dups_in,
                    })
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }
}

impl GraphStorage {
    /// Soft-delete `entity_id`: set `compressed_into = '__pruned__'` and drop
    /// its `kg_name_index` row so ANN and resolver queries stop seeing it.
    ///
    /// Unlike `merge_entity_into`, no relationships are re-pointed — the
    /// caller (Pruner) only uses this on orphans with zero edges.
    pub fn mark_pruned(&self, entity_id: &str) -> GraphResult<()> {
        let id = entity_id.to_string();
        self.db
            .with_connection(move |conn| {
                (|| -> GraphResult<()> {
                    let tx = conn.unchecked_transaction().map_err(GraphError::Database)?;
                    tx.execute(
                        "UPDATE kg_entities SET compressed_into = '__pruned__' WHERE id = ?1",
                        params![id],
                    )
                    .map_err(GraphError::Database)?;
                    tx.execute(
                        "DELETE FROM kg_name_index WHERE entity_id = ?1",
                        params![id],
                    )
                    .map_err(GraphError::Database)?;
                    tx.commit().map_err(GraphError::Database)?;
                    Ok(())
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }
}

/// Minimal projection of `kg_entities` used by the sleep-time Pruner to
/// select soft-deletion candidates. See `GraphStorage::list_orphan_old_candidates`.
#[derive(Debug, Clone)]
pub struct OrphanCandidate {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub mention_count: i64,
    pub last_seen_at: String,
}

impl GraphStorage {
    /// List entities that are candidates for pruning: non-archival, not yet
    /// compressed, with no incoming or outgoing relationships, and whose
    /// `last_seen_at` is older than `min_age_days`.
    ///
    /// Ordered by `mention_count ASC, last_seen_at ASC` so the weakest
    /// entities surface first.
    pub fn list_orphan_old_candidates(
        &self,
        agent_id: &str,
        min_age_days: i64,
        limit: usize,
    ) -> GraphResult<Vec<OrphanCandidate>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cutoff = chrono::Utc::now() - chrono::Duration::days(min_age_days);
        let cutoff_str = cutoff.to_rfc3339();
        let agent = agent_id.to_string();
        let lim = limit as i64;
        self.db
            .with_connection(move |conn| {
                (|| -> GraphResult<Vec<OrphanCandidate>> {
                    let mut stmt = conn
                        .prepare(
                            "SELECT e.id, e.name, e.entity_type, e.mention_count, e.last_seen_at \
                             FROM kg_entities e \
                             WHERE (e.agent_id = ?1 OR e.agent_id = '__global__') \
                               AND e.epistemic_class != 'archival' \
                               AND (e.compressed_into IS NULL OR e.compressed_into = '') \
                               AND e.last_seen_at < ?2 \
                               AND NOT EXISTS ( \
                                 SELECT 1 FROM kg_relationships r \
                                 WHERE r.source_entity_id = e.id \
                               ) \
                               AND NOT EXISTS ( \
                                 SELECT 1 FROM kg_relationships r \
                                 WHERE r.target_entity_id = e.id \
                               ) \
                             ORDER BY e.mention_count ASC, e.last_seen_at ASC \
                             LIMIT ?3",
                        )
                        .map_err(GraphError::Database)?;
                    let rows = stmt
                        .query_map(params![agent, cutoff_str, lim], |r| {
                            Ok(OrphanCandidate {
                                id: r.get::<_, String>(0)?,
                                name: r.get::<_, String>(1)?,
                                entity_type: r.get::<_, String>(2)?,
                                mention_count: r.get::<_, i64>(3)?,
                                last_seen_at: r.get::<_, String>(4)?,
                            })
                        })
                        .map_err(GraphError::Database)?;
                    let mut out = Vec::new();
                    for row in rows {
                        out.push(row.map_err(GraphError::Database)?);
                    }
                    Ok(out)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }
}

/// Outcome of a `merge_entity_into` call.
#[derive(Debug, Clone, Default)]
pub struct MergeResult {
    /// Count of relationships whose source or target was rewritten to winner.
    pub relationships_repointed: u64,
    /// Count of alias rows successfully re-pointed to winner (IGNORE misses excluded).
    pub aliases_transferred: u64,
    /// Count of relationships deleted because re-pointing would have duplicated an existing winner edge.
    pub duplicate_relationships_dropped: u64,
}

/// Normalize entity name for dedup matching.
/// For file entities, strip path prefixes to match on basename.
fn normalize_entity_name(name: &str, entity_type: &str) -> String {
    let trimmed = name.trim();

    // For file entities, match on basename (strip directory prefixes)
    if entity_type == "file" {
        if let Some(basename) = trimmed.rsplit('/').next() {
            if !basename.is_empty() {
                return basename.to_string();
            }
        }
    }

    trimmed.to_string()
}

/// Store an entity with cross-agent dedup.
///
/// Returns the actual entity ID used (either the existing entity's ID if
/// deduped, or the new entity's ID if inserted). The caller uses this to
/// remap relationship references.
/// Run the EntityResolver cascade and return the matched entity id, if any.
fn resolve_via_resolver(
    conn: &Connection,
    agent_id: &str,
    entity: &Entity,
) -> GraphResult<Option<String>> {
    match crate::resolver::resolve(conn, agent_id, entity, entity.name_embedding.as_deref())
        .map_err(GraphError::Other)?
    {
        crate::resolver::ResolveOutcome::Merge {
            existing_id,
            reason,
        } => {
            tracing::debug!(
                new_name = %entity.name,
                existing_id = %existing_id,
                reason = ?reason,
                "EntityResolver merged variant into existing entity"
            );
            Ok(Some(existing_id))
        }
        crate::resolver::ResolveOutcome::Create => Ok(None),
    }
}

/// Merge a candidate entity into an existing one: add alias, bump mention_count,
/// update last_seen_at.
/// Merge `patch` into `base` recursively:
/// - Objects are key-merged (patch keys overwrite base keys of same name).
/// - Arrays are concatenated, preserving insertion order and dropping exact
///   duplicates — critical for evidence lists that grow across sources.
/// - Scalars and mismatched types: patch wins.
pub(crate) fn merge_json_value(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                match b.get_mut(k) {
                    Some(bv) => merge_json_value(bv, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (serde_json::Value::Array(b), serde_json::Value::Array(p)) => {
            for item in p {
                if !b.contains(item) {
                    b.push(item.clone());
                }
            }
        }
        (base_slot, patch_val) => {
            *base_slot = patch_val.clone();
        }
    }
}

fn merge_into_existing(
    conn: &Connection,
    existing_id: &str,
    candidate: &Entity,
) -> GraphResult<()> {
    // Append candidate's surface form as an alias of the winning entity.
    let alias_id = format!("alias-{}", uuid::Uuid::new_v4());
    let normalized = crate::resolver::normalize_name(&candidate.name);
    conn.execute(
        "INSERT OR IGNORE INTO kg_aliases (
             id, entity_id, surface_form, normalized_form, source, confidence, first_seen_at
         ) VALUES (?1, ?2, ?3, ?4, 'merge', 1.0, ?5)",
        params![
            alias_id,
            existing_id,
            candidate.name,
            normalized,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(GraphError::Database)?;

    // Merge candidate.properties INTO the existing row's properties so nothing
    // is lost across sources. A biography supplying `{founded: "1976"}` and a
    // later stock analysis supplying `{industry: "tech"}` must produce
    // `{founded: "1976", industry: "tech"}`, not either alone.
    let existing_props_json: String = conn
        .query_row(
            "SELECT properties FROM kg_entities WHERE id = ?1",
            params![existing_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "{}".to_string());
    let mut existing_props: serde_json::Value =
        serde_json::from_str(&existing_props_json).unwrap_or(serde_json::json!({}));
    let candidate_props =
        serde_json::to_value(&candidate.properties).unwrap_or(serde_json::json!({}));
    merge_json_value(&mut existing_props, &candidate_props);
    let merged_json = serde_json::to_string(&existing_props).unwrap_or_else(|_| "{}".to_string());

    // Bump mention_count + last_seen_at on the winner, write merged properties.
    conn.execute(
        "UPDATE kg_entities
         SET mention_count = mention_count + 1,
             last_seen_at = ?1,
             properties = ?2
         WHERE id = ?3",
        params![chrono::Utc::now().to_rfc3339(), merged_json, existing_id],
    )
    .map_err(GraphError::Database)?;
    Ok(())
}

fn store_entity(conn: &Connection, agent_id: &str, entity: Entity) -> GraphResult<String> {
    let entity_type_str = entity.entity_type.as_str();
    let properties_json =
        serde_json::to_string(&entity.properties).unwrap_or_else(|_| "".to_string());

    // 1. EntityResolver cascade: exact-normalized → fuzzy → embedding.
    //    Scoped to the same agent (plus __global__) and entity_type.
    if let Some(existing_id) = resolve_via_resolver(conn, agent_id, &entity)? {
        merge_into_existing(conn, &existing_id, &entity)?;
        return Ok(existing_id);
    }

    // 2. Legacy fallback: cross-agent name dedup (handles file basename fallback).
    let normalized_name = normalize_entity_name(&entity.name, entity_type_str);
    if let Some(existing_id) = find_entity_by_name_global(conn, &normalized_name, entity_type_str)?
    {
        // Bump existing entity — dedup
        // Store full path as alias if different from matched name
        let mut existing_props: serde_json::Value = conn
            .query_row(
                "SELECT properties FROM kg_entities WHERE id = ?1",
                params![existing_id],
                |row| {
                    let s: String = row.get(0)?;
                    Ok(serde_json::from_str(&s).unwrap_or(serde_json::json!({})))
                },
            )
            .unwrap_or(serde_json::json!({}));

        if entity.name != normalized_name {
            if let Some(obj) = existing_props.as_object_mut() {
                let aliases = obj.entry("aliases").or_insert(serde_json::json!([]));
                if let Some(arr) = aliases.as_array_mut() {
                    let full_name = serde_json::Value::String(entity.name.clone());
                    if !arr.contains(&full_name) {
                        arr.push(full_name);
                    }
                }
            }
        }
        let updated_props = serde_json::to_string(&existing_props).unwrap_or_default();

        conn.execute(
            "UPDATE kg_entities SET mention_count = mention_count + 1, last_seen_at = ?1, properties = ?2 WHERE id = ?3",
            params![entity.last_seen_at.to_rfc3339(), updated_props, existing_id],
        ).map_err(GraphError::Database)?;
        return Ok(existing_id);
    }

    // New entity — insert with agent_id = '__global__' for cross-agent visibility.
    // v22 schema requires normalized_name + normalized_hash; derive them here.
    let new_id = entity.id.clone();
    let norm_name = normalize_entity_name(&entity.name, entity_type_str).to_lowercase();
    let norm_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        norm_name.hash(&mut h);
        format!("{:x}", h.finish())
    };

    // If the caller supplied an explicit id that matches an existing row, we
    // need to merge properties instead of overwriting them. Read + merge here
    // so the INSERT ... ON CONFLICT DO UPDATE writes the merged blob.
    let props_to_store: String = match conn.query_row(
        "SELECT properties FROM kg_entities WHERE id = ?1",
        params![new_id],
        |row| row.get::<_, String>(0),
    ) {
        Ok(existing_json) => {
            let mut existing: serde_json::Value =
                serde_json::from_str(&existing_json).unwrap_or(serde_json::json!({}));
            let patch: serde_json::Value =
                serde_json::from_str(&properties_json).unwrap_or(serde_json::json!({}));
            merge_json_value(&mut existing, &patch);
            serde_json::to_string(&existing).unwrap_or(properties_json.clone())
        }
        Err(_) => properties_json.clone(),
    };

    conn.execute(
        "INSERT INTO kg_entities (id, agent_id, entity_type, name, normalized_name, normalized_hash, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            mention_count = mention_count + 1,
            properties = excluded.properties",
        params![
            new_id,
            "__global__",
            entity_type_str,
            entity.name,
            norm_name,
            norm_hash,
            props_to_store,
            entity.first_seen_at.to_rfc3339(),
            entity.last_seen_at.to_rfc3339(),
            entity.mention_count,
        ],
    ).map_err(GraphError::Database)?;

    // Seed self-alias so future mentions of this exact surface form short-circuit
    // at resolver stage 1 (alias-table lookup).
    let alias_id = format!("alias-{}", uuid::Uuid::new_v4());
    let normalized = crate::resolver::normalize_name(&entity.name);
    conn.execute(
        "INSERT OR IGNORE INTO kg_aliases (
             id, entity_id, surface_form, normalized_form, source, confidence, first_seen_at
         ) VALUES (?1, ?2, ?3, ?4, 'extraction', 1.0, ?5)",
        params![
            alias_id,
            new_id,
            entity.name,
            normalized,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(GraphError::Database)?;

    // Populate kg_name_index if the caller provided a name embedding.
    // vec0 does not support UPSERT; emulate with delete+insert (safe under SQLite's
    // single-writer pool semantics — any concurrent insert sees the fresh row).
    if let Some(emb) = entity.name_embedding.as_ref() {
        if !emb.is_empty() {
            conn.execute(
                "DELETE FROM kg_name_index WHERE entity_id = ?1",
                params![new_id],
            )
            .map_err(GraphError::Database)?;
            let embedding_json = serde_json::to_string(emb)
                .map_err(|e| GraphError::Other(format!("serialize embedding: {e}")))?;
            conn.execute(
                "INSERT INTO kg_name_index (entity_id, name_embedding) VALUES (?1, ?2)",
                params![new_id, embedding_json],
            )
            .map_err(GraphError::Database)?;
        }
    }

    Ok(new_id)
}

/// Find an existing entity by name + type across ALL agents (case-insensitive).
fn find_entity_by_name_global(
    conn: &Connection,
    name: &str,
    entity_type: &str,
) -> GraphResult<Option<String>> {
    // Exact match first (case-insensitive)
    let mut stmt = conn.prepare(
        "SELECT id FROM kg_entities WHERE name = ?1 COLLATE NOCASE AND entity_type = ?2 LIMIT 1"
    ).map_err(GraphError::Database)?;

    match stmt.query_row(params![name, entity_type], |row| row.get::<_, String>(0)) {
        Ok(id) => return Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => {}
        Err(e) => return Err(GraphError::Database(e)),
    }

    // For file entities, also try matching against the basename of existing full paths
    if entity_type == "file" {
        let like_pattern = format!("%/{}", name);
        let mut stmt2 = conn.prepare(
            "SELECT id FROM kg_entities WHERE name LIKE ?1 COLLATE NOCASE AND entity_type = ?2 LIMIT 1"
        ).map_err(GraphError::Database)?;

        match stmt2.query_row(params![like_pattern, entity_type], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(id) => return Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(GraphError::Database(e)),
        }
    }

    Ok(None)
}

/// Find an existing entity by agent_id + name (case-insensitive).
/// Also checks `__global__` entities since store_entity now deduplicates cross-agent.
fn find_entity_by_name(
    conn: &Connection,
    agent_id: &str,
    name: &str,
) -> GraphResult<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM kg_entities WHERE (agent_id = ?1 OR agent_id = '__global__') AND name = ?2 COLLATE NOCASE LIMIT 1"
    ).map_err(GraphError::Database)?;

    match stmt.query_row(params![agent_id, name], |row| row.get::<_, String>(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(GraphError::Database(e)),
    }
}

/// Increment mention count and update last_seen for an existing entity.
fn bump_entity_mention(conn: &Connection, entity_id: &str) -> GraphResult<()> {
    conn.execute(
        "UPDATE kg_entities SET mention_count = mention_count + 1, last_seen_at = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), entity_id],
    )
    .map_err(GraphError::Database)?;
    Ok(())
}

/// Store a relationship (upsert based on source + target + type)
fn store_relationship(
    conn: &Connection,
    agent_id: &str,
    relationship: Relationship,
) -> GraphResult<()> {
    let rel_type_str = relationship.relationship_type.as_str();
    let properties_json =
        serde_json::to_string(&relationship.properties).unwrap_or_else(|_| "".to_string());

    // Merge properties if a relationship with the same (source, target, type)
    // already exists. Evidence arrays in particular grow across sources and
    // must concatenate, not replace.
    let props_to_store: String = match conn.query_row(
        "SELECT properties FROM kg_relationships
         WHERE source_entity_id = ?1 AND target_entity_id = ?2 AND relationship_type = ?3",
        params![
            relationship.source_entity_id,
            relationship.target_entity_id,
            rel_type_str
        ],
        |row| row.get::<_, String>(0),
    ) {
        Ok(existing_json) => {
            let mut existing: serde_json::Value =
                serde_json::from_str(&existing_json).unwrap_or(serde_json::json!({}));
            let patch: serde_json::Value =
                serde_json::from_str(&properties_json).unwrap_or(serde_json::json!({}));
            merge_json_value(&mut existing, &patch);
            serde_json::to_string(&existing).unwrap_or(properties_json.clone())
        }
        Err(_) => properties_json.clone(),
    };

    conn.execute(
        "INSERT INTO kg_relationships (id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(source_entity_id, target_entity_id, relationship_type) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            mention_count = mention_count + 1,
            properties = excluded.properties",
        params![
            relationship.id,
            agent_id,
            relationship.source_entity_id,
            relationship.target_entity_id,
            rel_type_str,
            props_to_store,
            relationship.first_seen_at.to_rfc3339(),
            relationship.last_seen_at.to_rfc3339(),
            relationship.mention_count,
        ],
    ).map_err(GraphError::Database)?;

    Ok(())
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EntityType, RelationshipType};
    use tempfile::tempdir;

    #[test]
    fn merge_json_value_adds_new_keys() {
        let mut base = serde_json::json!({"a": 1});
        let patch = serde_json::json!({"b": 2});
        merge_json_value(&mut base, &patch);
        assert_eq!(base, serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn merge_json_value_arrays_concat_dedupe() {
        let mut base = serde_json::json!({"evidence": [{"chunk": "a"}]});
        let patch = serde_json::json!({"evidence": [{"chunk": "b"}, {"chunk": "a"}]});
        merge_json_value(&mut base, &patch);
        assert_eq!(
            base,
            serde_json::json!({"evidence": [{"chunk": "a"}, {"chunk": "b"}]})
        );
    }

    #[test]
    fn merge_json_value_nested_objects_merge_recursively() {
        let mut base = serde_json::json!({"meta": {"founded": "1976"}});
        let patch = serde_json::json!({"meta": {"industry": "tech"}});
        merge_json_value(&mut base, &patch);
        assert_eq!(
            base,
            serde_json::json!({"meta": {"founded": "1976", "industry": "tech"}})
        );
    }

    #[test]
    fn merge_json_value_scalar_newer_wins() {
        let mut base = serde_json::json!({"status": "draft"});
        let patch = serde_json::json!({"status": "final"});
        merge_json_value(&mut base, &patch);
        assert_eq!(base, serde_json::json!({"status": "final"}));
    }

    fn create_test_storage() -> GraphStorage {
        let dir = tempdir().unwrap();
        let tmp_path = dir.keep();
        let paths = Arc::new(gateway_services::VaultPaths::new(tmp_path));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = Arc::new(KnowledgeDatabase::new(paths).unwrap());
        GraphStorage::new(db).unwrap()
    }

    #[tokio::test]
    async fn test_list_entities_with_pagination() {
        let storage = create_test_storage();

        // Store some entities
        let entity1 = Entity::new(
            "agent1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );
        let entity2 = Entity::new("agent1".to_string(), EntityType::Tool, "Rust".to_string());
        let entity3 = Entity::new("agent1".to_string(), EntityType::Person, "Bob".to_string());

        let knowledge = ExtractedKnowledge {
            entities: vec![entity1, entity2, entity3],
            relationships: vec![],
        };
        storage.store_knowledge("agent1", knowledge).unwrap();

        // List with limit
        let entities = storage.list_entities("agent1", None, 2, 0).unwrap();
        assert_eq!(entities.len(), 2);

        // List with offset
        let entities = storage.list_entities("agent1", None, 2, 2).unwrap();
        assert_eq!(entities.len(), 1);

        // List with type filter
        let entities = storage
            .list_entities("agent1", Some("person"), 10, 0)
            .unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[tokio::test]
    async fn test_list_relationships_with_pagination() {
        let storage = create_test_storage();

        // Store entities and relationships
        let entity1 = Entity::new(
            "agent1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );
        let entity2 = Entity::new("agent1".to_string(), EntityType::Tool, "Rust".to_string());
        let entity3 = Entity::new(
            "agent1".to_string(),
            EntityType::Project,
            "ProjectX".to_string(),
        );

        let rel1 = Relationship::new(
            "agent1".to_string(),
            entity1.id.clone(),
            entity2.id.clone(),
            RelationshipType::Uses,
        );
        let rel2 = Relationship::new(
            "agent1".to_string(),
            entity1.id.clone(),
            entity3.id.clone(),
            RelationshipType::Created,
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![entity1, entity2, entity3],
            relationships: vec![rel1, rel2],
        };
        storage.store_knowledge("agent1", knowledge).unwrap();

        // List with limit
        let rels = storage.list_relationships("agent1", None, 1, 0).unwrap();
        assert_eq!(rels.len(), 1);

        // List with type filter
        let rels = storage
            .list_relationships("agent1", Some("uses"), 10, 0)
            .unwrap();
        assert_eq!(rels.len(), 1);
        assert!(matches!(rels[0].relationship_type, RelationshipType::Uses));
    }

    #[tokio::test]
    async fn test_get_entity_by_name_case_insensitive() {
        let storage = create_test_storage();

        // Store entity
        let entity = Entity::new(
            "agent1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );
        let knowledge = ExtractedKnowledge {
            entities: vec![entity],
            relationships: vec![],
        };
        storage.store_knowledge("agent1", knowledge).unwrap();

        // Search with different case
        let result = storage.get_entity_by_name("agent1", "alice").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "Alice");

        let result = storage.get_entity_by_name("agent1", "ALICE").unwrap();
        assert!(result.is_some());

        // Non-existent entity
        let result = storage.get_entity_by_name("agent1", "Bob").unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_neighbors() {
        let storage = create_test_storage();

        // Create a small graph: Alice -> uses -> Rust, Bob -> uses -> Rust
        let alice = Entity::new(
            "agent1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );
        let bob = Entity::new("agent1".to_string(), EntityType::Person, "Bob".to_string());
        let rust = Entity::new("agent1".to_string(), EntityType::Tool, "Rust".to_string());

        let alice_uses_rust = Relationship::new(
            "agent1".to_string(),
            alice.id.clone(),
            rust.id.clone(),
            RelationshipType::Uses,
        );
        let bob_uses_rust = Relationship::new(
            "agent1".to_string(),
            bob.id.clone(),
            rust.id.clone(),
            RelationshipType::Uses,
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![alice.clone(), bob, rust.clone()],
            relationships: vec![alice_uses_rust, bob_uses_rust],
        };
        storage.store_knowledge("agent1", knowledge).unwrap();

        // Get Alice's outgoing neighbors (Alice -> Rust)
        let neighbors = storage
            .get_neighbors("agent1", &alice.id, Direction::Outgoing, 10)
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].entity.name, "Rust");
        assert_eq!(neighbors[0].direction, Direction::Outgoing);

        // Get Rust's incoming neighbors (Alice -> Rust, Bob -> Rust)
        let neighbors = storage
            .get_neighbors("agent1", &rust.id, Direction::Incoming, 10)
            .unwrap();
        assert_eq!(neighbors.len(), 2);

        // Get both directions
        let neighbors = storage
            .get_neighbors("agent1", &alice.id, Direction::Both, 10)
            .unwrap();
        assert_eq!(neighbors.len(), 1); // Only outgoing
    }

    #[tokio::test]
    async fn test_count_entities_and_relationships() {
        let storage = create_test_storage();

        // Initially empty
        assert_eq!(storage.count_entities("agent1").unwrap(), 0);
        assert_eq!(storage.count_relationships("agent1").unwrap(), 0);

        // Store some data
        let entity1 = Entity::new(
            "agent1".to_string(),
            EntityType::Person,
            "Alice".to_string(),
        );
        let entity2 = Entity::new("agent1".to_string(), EntityType::Tool, "Rust".to_string());
        let rel = Relationship::new(
            "agent1".to_string(),
            entity1.id.clone(),
            entity2.id.clone(),
            RelationshipType::Uses,
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![entity1, entity2],
            relationships: vec![rel],
        };
        storage.store_knowledge("agent1", knowledge).unwrap();

        // Count after storing
        assert_eq!(storage.count_entities("agent1").unwrap(), 2);
        assert_eq!(storage.count_relationships("agent1").unwrap(), 1);

        // Different agent also sees __global__ entities
        assert_eq!(storage.count_entities("agent2").unwrap(), 2);
    }

    #[tokio::test]
    async fn test_delete_agent_data() {
        let storage = create_test_storage();

        // Store entities and relationships for an agent
        let entity1 = Entity::new(
            "agent-del".to_string(),
            EntityType::Person,
            "DeleteMe".to_string(),
        );
        let entity2 = Entity::new(
            "agent-del".to_string(),
            EntityType::Tool,
            "AlsoDeleteMe".to_string(),
        );
        let rel = Relationship::new(
            "agent-del".to_string(),
            entity1.id.clone(),
            entity2.id.clone(),
            RelationshipType::Uses,
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![entity1, entity2],
            relationships: vec![rel],
        };
        storage.store_knowledge("agent-del", knowledge).unwrap();

        // Entities are stored as __global__ due to cross-agent dedup, so
        // delete_agent_data("agent-del") won't remove them (they belong to __global__).
        // Verify the entities exist under __global__
        assert!(storage.count_all_entities().unwrap() >= 2);

        // Delete with __global__ agent_id to actually remove them
        let deleted = storage.delete_agent_data("__global__").unwrap();
        assert!(deleted >= 2); // At least 2 entities removed

        // Verify clean
        assert_eq!(storage.count_all_entities().unwrap(), 0);
        assert_eq!(storage.count_all_relationships().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_upsert_entity() {
        let storage = create_test_storage();

        // Insert an entity
        let entity = Entity::new(
            "agent-ups".to_string(),
            EntityType::Person,
            "UpsertUser".to_string(),
        );

        let knowledge = ExtractedKnowledge {
            entities: vec![entity],
            relationships: vec![],
        };
        storage.store_knowledge("agent-ups", knowledge).unwrap();

        // Verify initial mention_count is 1
        let found = storage
            .get_entity_by_name("agent-ups", "UpsertUser")
            .unwrap();
        assert!(found.is_some());
        let first = found.unwrap();
        assert_eq!(first.mention_count, 1);

        // Insert same entity name again — should deduplicate and bump mention_count
        let entity2 = Entity::new(
            "agent-ups".to_string(),
            EntityType::Person,
            "UpsertUser".to_string(),
        );

        let knowledge2 = ExtractedKnowledge {
            entities: vec![entity2],
            relationships: vec![],
        };
        storage.store_knowledge("agent-ups", knowledge2).unwrap();

        // Verify mention_count was incremented
        let found = storage
            .get_entity_by_name("agent-ups", "UpsertUser")
            .unwrap();
        assert!(found.is_some());
        let second = found.unwrap();
        assert_eq!(second.mention_count, 2);

        // Verify still only one entity with that name
        assert_eq!(storage.count_all_entities().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_duplicate_relationship() {
        let storage = create_test_storage();

        // Create two entities
        let entity1 = Entity::new(
            "agent-dup".to_string(),
            EntityType::Person,
            "DupAlice".to_string(),
        );
        let entity2 = Entity::new(
            "agent-dup".to_string(),
            EntityType::Tool,
            "DupRust".to_string(),
        );

        let e1_id = entity1.id.clone();
        let e2_id = entity2.id.clone();

        // First insert with relationship
        let rel1 = Relationship::new(
            "agent-dup".to_string(),
            e1_id.clone(),
            e2_id.clone(),
            RelationshipType::Uses,
        );

        let knowledge1 = ExtractedKnowledge {
            entities: vec![entity1, entity2],
            relationships: vec![rel1],
        };
        storage.store_knowledge("agent-dup", knowledge1).unwrap();

        assert_eq!(storage.count_all_relationships().unwrap(), 1);

        // Insert same relationship again (same source, target, type) — should not error
        // We need to use the actual entity IDs that were stored (they may have been remapped)
        let entities = storage.get_entities("agent-dup").unwrap();
        let alice = entities.iter().find(|e| e.name == "DupAlice").unwrap();
        let rust = entities.iter().find(|e| e.name == "DupRust").unwrap();

        let rel2 = Relationship::new(
            "agent-dup".to_string(),
            alice.id.clone(),
            rust.id.clone(),
            RelationshipType::Uses,
        );

        let knowledge2 = ExtractedKnowledge {
            entities: vec![],
            relationships: vec![rel2],
        };
        // This should not error — the unique index uses ON CONFLICT DO UPDATE
        storage.store_knowledge("agent-dup", knowledge2).unwrap();

        // Still only 1 relationship, but mention_count should have incremented
        assert_eq!(storage.count_all_relationships().unwrap(), 1);

        let rels = storage
            .list_relationships("agent-dup", None, 10, 0)
            .unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].mention_count, 2);
    }

    #[test]
    fn store_entity_seeds_self_alias() {
        let tmp = tempfile::tempdir().unwrap();
        let paths =
            std::sync::Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = std::sync::Arc::new(gateway_database::KnowledgeDatabase::new(paths).unwrap());

        let storage = GraphStorage::new(db.clone()).unwrap();

        let mut entity = Entity::new(
            "root".to_string(),
            crate::EntityType::Person,
            "A.D. Lovelace".to_string(),
        );
        entity.id = "e1".to_string();

        let knowledge = ExtractedKnowledge {
            entities: vec![entity],
            relationships: vec![],
        };
        storage.store_knowledge("root", knowledge).unwrap();

        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kg_aliases WHERE normalized_form = ?1",
                rusqlite::params!["ad lovelace"],
                |r| r.get(0),
            )?;
            assert_eq!(count, 1, "self-alias should be seeded on entity create");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn merge_appends_alias_row() {
        let tmp = tempfile::tempdir().unwrap();
        let paths =
            std::sync::Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = std::sync::Arc::new(gateway_database::KnowledgeDatabase::new(paths).unwrap());
        let storage = GraphStorage::new(db.clone()).unwrap();

        let mut e1 = Entity::new(
            "root".to_string(),
            crate::EntityType::Person,
            "A.D. Lovelace".to_string(),
        );
        e1.id = "e1".to_string();
        storage
            .store_knowledge(
                "root",
                ExtractedKnowledge {
                    entities: vec![e1],
                    relationships: vec![],
                },
            )
            .unwrap();

        let mut e2 = Entity::new(
            "root".to_string(),
            crate::EntityType::Person,
            "Augusta Lovelace".to_string(),
        );
        e2.id = "e2".to_string();
        storage
            .store_knowledge(
                "root",
                ExtractedKnowledge {
                    entities: vec![e2],
                    relationships: vec![],
                },
            )
            .unwrap();

        db.with_connection(|conn| {
            let e1_alias_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kg_aliases WHERE entity_id = 'e1'",
                [],
                |r| r.get(0),
            )?;
            assert!(e1_alias_count >= 1, "at least one alias row for e1");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn embedding_stage_merges_similar_name() {
        let tmp = tempfile::tempdir().unwrap();
        let paths =
            std::sync::Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().unwrap()).unwrap();
        let db = std::sync::Arc::new(gateway_database::KnowledgeDatabase::new(paths).unwrap());
        let storage = GraphStorage::new(db.clone()).unwrap();

        fn normalized(v: Vec<f32>) -> Vec<f32> {
            let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if n < 1e-9 {
                v
            } else {
                v.into_iter().map(|x| x / n).collect()
            }
        }

        let emb1 = normalized((0..384).map(|i| i as f32).collect());

        let mut e1 = Entity::new(
            "root".to_string(),
            crate::EntityType::Person,
            "A.D. Lovelace".to_string(),
        );
        e1.id = "e1".to_string();
        e1.name_embedding = Some(emb1.clone());
        storage
            .store_knowledge(
                "root",
                ExtractedKnowledge {
                    entities: vec![e1],
                    relationships: vec![],
                },
            )
            .unwrap();

        // Candidate: completely different surface form (stage 1 alias miss),
        // near-identical embedding (stage 2 should merge).
        let mut emb2 = emb1.clone();
        emb2[0] *= 0.999;
        let emb2 = normalized(emb2);

        let mut e2 = Entity::new(
            "root".to_string(),
            crate::EntityType::Person,
            "UniqueString12345".to_string(),
        );
        e2.id = "e2".to_string();
        e2.name_embedding = Some(emb2);
        storage
            .store_knowledge(
                "root",
                ExtractedKnowledge {
                    entities: vec![e2],
                    relationships: vec![],
                },
            )
            .unwrap();

        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE entity_type = 'person'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(
                count, 1,
                "embedding stage should merge near-identical vectors"
            );
            Ok(())
        })
        .unwrap();
    }

    // Helpers for the find/merge tests below.
    fn normalize_vec(v: Vec<f32>) -> Vec<f32> {
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n < 1e-9 {
            v
        } else {
            v.into_iter().map(|x| x / n).collect()
        }
    }

    /// Build a 384-dim unit vector whose first component is `a`, second is
    /// `sqrt(1 - a^2)`, rest zero — lets us dial cosine similarity exactly.
    fn unit_with_first(a: f32) -> Vec<f32> {
        let mut v = vec![0.0_f32; 384];
        v[0] = a;
        v[1] = (1.0 - a * a).max(0.0).sqrt();
        normalize_vec(v)
    }

    #[test]
    fn find_duplicate_candidates_returns_near_neighbor_pair() {
        let storage = create_test_storage();

        // e1 embedding: (1, 0, 0, ...). e2: cosine ≈ 0.82 with e1 — close enough
        // to surface as a duplicate candidate at threshold 0.80, but below the
        // 0.87 auto-merge cutoff in resolver::embedding_match.
        let emb1 = unit_with_first(1.0);
        let emb2 = unit_with_first(0.82);

        let mut e1 = Entity::new(
            "agent-dup".to_string(),
            EntityType::Person,
            "Distinct Name Alpha".to_string(),
        );
        e1.id = "dup-e1".to_string();
        e1.name_embedding = Some(emb1);

        let mut e2 = Entity::new(
            "agent-dup".to_string(),
            EntityType::Person,
            "UnrelatedSurfaceForm".to_string(),
        );
        e2.id = "dup-e2".to_string();
        e2.name_embedding = Some(emb2);

        storage
            .store_knowledge(
                "agent-dup",
                ExtractedKnowledge {
                    entities: vec![e1, e2],
                    relationships: vec![],
                },
            )
            .unwrap();

        // Use a threshold below the observed cosine (measured ~0.70 after vec0
        // round-trip) but well above the 0.0 default — the point of this test
        // is to prove the pair surfaces, not to pin a specific similarity.
        let pairs = storage
            .find_duplicate_candidates("agent-dup", "person", 0.65, 10)
            .unwrap();

        assert!(
            !pairs.is_empty(),
            "expected at least one candidate pair, got none"
        );
        let (a, b, cos) = &pairs[0];
        let mut ids = [a.clone(), b.clone()];
        ids.sort();
        assert_eq!(ids, ["dup-e1".to_string(), "dup-e2".to_string()]);
        assert!(*cos >= 0.65, "cosine {cos} should meet threshold 0.65");
    }

    #[test]
    fn merge_entity_into_repoints_relationships() {
        let storage = create_test_storage();

        let mut loser = Entity::new(
            "agent-m".to_string(),
            EntityType::Person,
            "Loser".to_string(),
        );
        loser.id = "loser".to_string();
        let mut winner = Entity::new(
            "agent-m".to_string(),
            EntityType::Person,
            "Winner".to_string(),
        );
        winner.id = "winner".to_string();
        let mut other_a = Entity::new("agent-m".to_string(), EntityType::Tool, "Rust".to_string());
        other_a.id = "other-a".to_string();
        let mut other_b = Entity::new(
            "agent-m".to_string(),
            EntityType::Tool,
            "Python".to_string(),
        );
        other_b.id = "other-b".to_string();

        let rel_loser = Relationship::new(
            "agent-m".to_string(),
            "loser".to_string(),
            "other-a".to_string(),
            RelationshipType::Uses,
        );
        let rel_winner = Relationship::new(
            "agent-m".to_string(),
            "winner".to_string(),
            "other-b".to_string(),
            RelationshipType::Uses,
        );

        storage
            .store_knowledge(
                "agent-m",
                ExtractedKnowledge {
                    entities: vec![loser, winner, other_a, other_b],
                    relationships: vec![rel_loser, rel_winner],
                },
            )
            .unwrap();

        let result = storage.merge_entity_into("loser", "winner").unwrap();
        assert_eq!(result.duplicate_relationships_dropped, 0);
        assert!(result.relationships_repointed >= 1);

        // After merge: loser's edge is re-pointed to winner; both edges remain.
        let rels = storage.list_relationships("agent-m", None, 100, 0).unwrap();
        let sources: std::collections::HashSet<String> =
            rels.iter().map(|r| r.source_entity_id.clone()).collect();
        assert!(
            sources.contains("winner"),
            "winner should be source of at least one edge"
        );
        assert!(
            !sources.contains("loser"),
            "no relationship should still point from loser"
        );

        // compressed_into is populated.
        storage
            .db
            .with_connection(|conn| {
                let ci: Option<String> = conn
                    .query_row(
                        "SELECT compressed_into FROM kg_entities WHERE id = 'loser'",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(ci.as_deref(), Some("winner"));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn merge_handles_relationship_collision() {
        let storage = create_test_storage();

        let mut loser = Entity::new(
            "agent-c".to_string(),
            EntityType::Person,
            "Loser".to_string(),
        );
        loser.id = "loser-c".to_string();
        let mut winner = Entity::new(
            "agent-c".to_string(),
            EntityType::Person,
            "Winner".to_string(),
        );
        winner.id = "winner-c".to_string();
        let mut target = Entity::new("agent-c".to_string(), EntityType::Tool, "Rust".to_string());
        target.id = "target-c".to_string();

        // Both loser and winner already have `uses → target` — re-pointing
        // naïvely would violate the UNIQUE constraint.
        let rel_loser = Relationship::new(
            "agent-c".to_string(),
            "loser-c".to_string(),
            "target-c".to_string(),
            RelationshipType::Uses,
        );
        let rel_winner = Relationship::new(
            "agent-c".to_string(),
            "winner-c".to_string(),
            "target-c".to_string(),
            RelationshipType::Uses,
        );

        storage
            .store_knowledge(
                "agent-c",
                ExtractedKnowledge {
                    entities: vec![loser, winner, target],
                    relationships: vec![rel_loser, rel_winner],
                },
            )
            .unwrap();

        let result = storage.merge_entity_into("loser-c", "winner-c").unwrap();
        assert_eq!(
            result.duplicate_relationships_dropped, 1,
            "expected exactly one colliding edge dropped"
        );

        let rels = storage.list_relationships("agent-c", None, 100, 0).unwrap();
        let surviving: Vec<_> = rels
            .iter()
            .filter(|r| {
                r.target_entity_id == "target-c" && r.relationship_type == RelationshipType::Uses
            })
            .collect();
        assert_eq!(
            surviving.len(),
            1,
            "exactly one winner → target edge should remain"
        );
        assert_eq!(surviving[0].source_entity_id, "winner-c");
    }
}
