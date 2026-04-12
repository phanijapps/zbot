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
            });
        }

        Ok(entities)
                })()
                .map_err(graph_to_rusqlite)
            })
            .map_err(GraphError::Other)
    }
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
    match crate::resolver::resolve(conn, agent_id, entity, None).map_err(GraphError::Other)? {
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
fn merge_into_existing(
    conn: &Connection,
    existing_id: &str,
    candidate: &Entity,
) -> GraphResult<()> {
    let current_aliases: Option<String> = conn
        .query_row(
            "SELECT aliases FROM kg_entities WHERE id = ?1",
            params![existing_id],
            |r| r.get(0),
        )
        .ok();
    let new_aliases = crate::resolver::merge_alias(current_aliases.as_deref(), &candidate.name);
    conn.execute(
        "UPDATE kg_entities SET aliases = ?1, mention_count = mention_count + 1, last_seen_at = ?2 WHERE id = ?3",
        params![new_aliases, chrono::Utc::now().to_rfc3339(), existing_id],
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
            properties_json,
            entity.first_seen_at.to_rfc3339(),
            entity.last_seen_at.to_rfc3339(),
            entity.mention_count,
        ],
    ).map_err(GraphError::Database)?;

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
            properties_json,
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
}
