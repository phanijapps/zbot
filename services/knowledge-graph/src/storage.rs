//! # Graph Storage
//!
//! SQLite storage for knowledge graph entities and relationships.

use crate::error::{GraphError, GraphResult};
use crate::types::{
    Direction, Entity, EntityType, ExtractedKnowledge, NeighborInfo, Relationship, RelationshipType,
};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// SQLite storage for knowledge graph
pub struct GraphStorage {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl GraphStorage {
    /// Create a new graph storage
    pub fn new(db_path: PathBuf) -> GraphResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| GraphError::Config(format!("Failed to create directory: {}", e)))?;
        }

        let conn = Connection::open(&db_path).map_err(GraphError::Database)?;

        // Initialize schema
        initialize_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Store extracted knowledge (entities and relationships)
    pub async fn store_knowledge(
        &self,
        agent_id: &str,
        knowledge: ExtractedKnowledge,
    ) -> GraphResult<()> {
        let conn = self.conn.lock().await;

        // Store entities and build ID mapping (new_id → actual_id)
        // When an entity deduplicates, the actual_id is the existing entity's ID
        let mut entity_id_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for entity in knowledge.entities {
            let original_id = entity.id.clone();
            let actual_id = store_entity(&conn, agent_id, entity)?;
            entity_id_map.insert(original_id, actual_id);
        }

        // Store relationships, remapping entity IDs through the dedup map
        for mut relationship in knowledge.relationships {
            if let Some(mapped) = entity_id_map.get(&relationship.source_entity_id) {
                relationship.source_entity_id = mapped.clone();
            }
            if let Some(mapped) = entity_id_map.get(&relationship.target_entity_id) {
                relationship.target_entity_id = mapped.clone();
            }
            store_relationship(&conn, agent_id, relationship)?;
        }

        Ok(())
    }

    /// Get all entities for an agent (includes __global__ entities)
    pub async fn get_entities(&self, agent_id: &str) -> GraphResult<Vec<Entity>> {
        let conn = self.conn.lock().await;

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
    }

    /// Get all relationships for an agent (includes __global__ relationships)
    pub async fn get_relationships(&self, agent_id: &str) -> GraphResult<Vec<Relationship>> {
        let conn = self.conn.lock().await;

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
    }

    /// Search entities by name (includes __global__ entities)
    pub async fn search_entities(&self, agent_id: &str, query: &str) -> GraphResult<Vec<Entity>> {
        let conn = self.conn.lock().await;

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
    }

    /// Find an existing entity by agent_id + name (case-insensitive), returning its ID.
    pub async fn find_entity_by_name(
        &self,
        agent_id: &str,
        name: &str,
    ) -> GraphResult<Option<String>> {
        let conn = self.conn.lock().await;
        find_entity_by_name(&conn, agent_id, name)
    }

    /// Increment mention count and update last_seen for an existing entity.
    pub async fn bump_entity_mention(&self, entity_id: &str) -> GraphResult<()> {
        let conn = self.conn.lock().await;
        bump_entity_mention(&conn, entity_id)
    }

    /// Delete all data for an agent
    pub async fn delete_agent_data(&self, agent_id: &str) -> GraphResult<usize> {
        let conn = self.conn.lock().await;

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
    }

    // ===== NEW READ METHODS (Phase 1: Graph Repository Layer) =====

    /// List entities for an agent with optional type filter and pagination
    pub async fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> GraphResult<Vec<Entity>> {
        let conn = self.conn.lock().await;

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
    }

    /// List relationships for an agent with optional type filter and pagination
    pub async fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> GraphResult<Vec<Relationship>> {
        let conn = self.conn.lock().await;

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
    }

    /// Get entity by name (case-insensitive)
    pub async fn get_entity_by_name(
        &self,
        agent_id: &str,
        name: &str,
    ) -> GraphResult<Option<Entity>> {
        let conn = self.conn.lock().await;

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
    }

    /// Get neighbors of an entity (1-hop)
    pub async fn get_neighbors(
        &self,
        _agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> GraphResult<Vec<NeighborInfo>> {
        let conn = self.conn.lock().await;

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
    }

    /// Count entities for an agent
    pub async fn count_entities(&self, agent_id: &str) -> GraphResult<usize> {
        let conn = self.conn.lock().await;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE agent_id = ?1 OR agent_id = '__global__'",
                params![agent_id],
                |row| row.get(0),
            )
            .map_err(GraphError::Database)?;

        Ok(count as usize)
    }

    /// Count relationships for an agent
    pub async fn count_relationships(&self, agent_id: &str) -> GraphResult<usize> {
        let conn = self.conn.lock().await;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM kg_relationships WHERE agent_id = ?1 OR agent_id = '__global__'",
            params![agent_id],
            |row| row.get(0),
        ).map_err(GraphError::Database)?;

        Ok(count as usize)
    }

    /// Count all entities across all agents.
    pub async fn count_all_entities(&self) -> GraphResult<usize> {
        let conn = self.conn.lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM kg_entities", [], |row| row.get(0))
            .map_err(GraphError::Database)?;
        Ok(count as usize)
    }

    /// Count all relationships across all agents.
    pub async fn count_all_relationships(&self) -> GraphResult<usize> {
        let conn = self.conn.lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM kg_relationships", [], |row| {
                row.get(0)
            })
            .map_err(GraphError::Database)?;
        Ok(count as usize)
    }

    /// List all relationships across all agents with pagination.
    ///
    /// Used by the Observatory "All Agents" mode to render edges.
    pub async fn list_all_relationships(&self, limit: usize) -> GraphResult<Vec<Relationship>> {
        let conn = self.conn.lock().await;

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
    }

    /// List entities across all agents with optional filters and pagination.
    ///
    /// Used by the Observatory "All Agents" mode.
    pub async fn list_all_entities(
        &self,
        ward_id: Option<&str>,
        entity_type: Option<&str>,
        limit: usize,
    ) -> GraphResult<Vec<Entity>> {
        let conn = self.conn.lock().await;

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
    }
}

/// Initialize the knowledge graph database schema
fn initialize_schema(conn: &Connection) -> GraphResult<()> {
    // Create entities table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS kg_entities (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            name TEXT NOT NULL,
            properties TEXT,
            first_seen_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            mention_count INTEGER DEFAULT 1
        )",
        [],
    )
    .map_err(GraphError::Database)?;

    // Create indexes for entities
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entities_agent ON kg_entities(agent_id)",
        [],
    )
    .map_err(GraphError::Database)?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entities_name ON kg_entities(name)",
        [],
    )
    .map_err(GraphError::Database)?;

    // Create relationships table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS kg_relationships (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            source_entity_id TEXT NOT NULL,
            target_entity_id TEXT NOT NULL,
            relationship_type TEXT NOT NULL,
            properties TEXT,
            first_seen_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            mention_count INTEGER DEFAULT 1,
            FOREIGN KEY (source_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
            FOREIGN KEY (target_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(GraphError::Database)?;

    // Create indexes for relationships
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relationships_agent ON kg_relationships(agent_id)",
        [],
    )
    .map_err(GraphError::Database)?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relationships_source ON kg_relationships(source_entity_id)",
        [],
    )
    .map_err(GraphError::Database)?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relationships_target ON kg_relationships(target_entity_id)",
        [],
    )
    .map_err(GraphError::Database)?;

    // Migration: Add mention_count column if it doesn't exist (for databases created before this feature)
    // Check if mention_count column exists in kg_entities
    let has_entities_mention_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('kg_entities') WHERE name='mention_count'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if has_entities_mention_count == 0 {
        tracing::info!("Migrating kg_entities: adding mention_count column");
        conn.execute(
            "ALTER TABLE kg_entities ADD COLUMN mention_count INTEGER DEFAULT 1",
            [],
        )
        .map_err(GraphError::Database)?;
    }

    // Check if mention_count column exists in kg_relationships
    let has_relationships_mention_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('kg_relationships') WHERE name='mention_count'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if has_relationships_mention_count == 0 {
        tracing::info!("Migrating kg_relationships: adding mention_count column");
        conn.execute(
            "ALTER TABLE kg_relationships ADD COLUMN mention_count INTEGER DEFAULT 1",
            [],
        )
        .map_err(GraphError::Database)?;
    }

    // One-time migration: dedup existing relationships before adding unique index
    // Keep the row with lowest rowid per (source, target, type) triple
    conn.execute_batch(
        "
        DELETE FROM kg_relationships WHERE rowid NOT IN (
            SELECT MIN(rowid) FROM kg_relationships
            GROUP BY source_entity_id, target_entity_id, relationship_type
        );
    ",
    )
    .map_err(GraphError::Database)?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_kg_rel_unique
         ON kg_relationships(source_entity_id, target_entity_id, relationship_type)",
        [],
    )
    .map_err(GraphError::Database)?;

    Ok(())
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
fn store_entity(conn: &Connection, _agent_id: &str, entity: Entity) -> GraphResult<String> {
    let entity_type_str = entity.entity_type.as_str();
    let properties_json =
        serde_json::to_string(&entity.properties).unwrap_or_else(|_| "".to_string());

    // Check for existing entity with same name + type across ALL agents
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

    // New entity — insert with agent_id = '__global__' for cross-agent visibility
    let new_id = entity.id.clone();
    conn.execute(
        "INSERT INTO kg_entities (id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            mention_count = mention_count + 1,
            properties = excluded.properties",
        params![
            new_id,
            "__global__",
            entity_type_str,
            entity.name,
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

    async fn create_test_storage() -> GraphStorage {
        let dir = tempdir().unwrap();
        let db_path = dir.keep().join("test.db");
        GraphStorage::new(db_path).unwrap()
    }

    #[tokio::test]
    async fn test_list_entities_with_pagination() {
        let storage = create_test_storage().await;

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
        storage.store_knowledge("agent1", knowledge).await.unwrap();

        // List with limit
        let entities = storage.list_entities("agent1", None, 2, 0).await.unwrap();
        assert_eq!(entities.len(), 2);

        // List with offset
        let entities = storage.list_entities("agent1", None, 2, 2).await.unwrap();
        assert_eq!(entities.len(), 1);

        // List with type filter
        let entities = storage
            .list_entities("agent1", Some("person"), 10, 0)
            .await
            .unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[tokio::test]
    async fn test_list_relationships_with_pagination() {
        let storage = create_test_storage().await;

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
        storage.store_knowledge("agent1", knowledge).await.unwrap();

        // List with limit
        let rels = storage
            .list_relationships("agent1", None, 1, 0)
            .await
            .unwrap();
        assert_eq!(rels.len(), 1);

        // List with type filter
        let rels = storage
            .list_relationships("agent1", Some("uses"), 10, 0)
            .await
            .unwrap();
        assert_eq!(rels.len(), 1);
        assert!(matches!(rels[0].relationship_type, RelationshipType::Uses));
    }

    #[tokio::test]
    async fn test_get_entity_by_name_case_insensitive() {
        let storage = create_test_storage().await;

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
        storage.store_knowledge("agent1", knowledge).await.unwrap();

        // Search with different case
        let result = storage.get_entity_by_name("agent1", "alice").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "Alice");

        let result = storage.get_entity_by_name("agent1", "ALICE").await.unwrap();
        assert!(result.is_some());

        // Non-existent entity
        let result = storage.get_entity_by_name("agent1", "Bob").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_neighbors() {
        let storage = create_test_storage().await;

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
        storage.store_knowledge("agent1", knowledge).await.unwrap();

        // Get Alice's outgoing neighbors (Alice -> Rust)
        let neighbors = storage
            .get_neighbors("agent1", &alice.id, Direction::Outgoing, 10)
            .await
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].entity.name, "Rust");
        assert_eq!(neighbors[0].direction, Direction::Outgoing);

        // Get Rust's incoming neighbors (Alice -> Rust, Bob -> Rust)
        let neighbors = storage
            .get_neighbors("agent1", &rust.id, Direction::Incoming, 10)
            .await
            .unwrap();
        assert_eq!(neighbors.len(), 2);

        // Get both directions
        let neighbors = storage
            .get_neighbors("agent1", &alice.id, Direction::Both, 10)
            .await
            .unwrap();
        assert_eq!(neighbors.len(), 1); // Only outgoing
    }

    #[tokio::test]
    async fn test_count_entities_and_relationships() {
        let storage = create_test_storage().await;

        // Initially empty
        assert_eq!(storage.count_entities("agent1").await.unwrap(), 0);
        assert_eq!(storage.count_relationships("agent1").await.unwrap(), 0);

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
        storage.store_knowledge("agent1", knowledge).await.unwrap();

        // Count after storing
        assert_eq!(storage.count_entities("agent1").await.unwrap(), 2);
        assert_eq!(storage.count_relationships("agent1").await.unwrap(), 1);

        // Different agent also sees __global__ entities
        assert_eq!(storage.count_entities("agent2").await.unwrap(), 2);
    }
}
