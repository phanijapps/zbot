//! # Graph Storage
//!
//! SQLite storage for knowledge graph entities and relationships.

use crate::error::{GraphError, GraphResult};
use crate::types::{Entity, EntityType, Relationship, RelationshipType, ExtractedKnowledge};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// SQLite storage for knowledge graph
pub struct GraphStorage {
    conn: Arc<Mutex<Connection>>,
}

impl GraphStorage {
    /// Create a new graph storage
    pub fn new(db_path: PathBuf) -> GraphResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| GraphError::Config(format!("Failed to create directory: {}", e)))?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| GraphError::Database(e))?;

        // Initialize schema
        initialize_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Store extracted knowledge (entities and relationships)
    pub async fn store_knowledge(&self, agent_id: &str, knowledge: ExtractedKnowledge) -> GraphResult<()> {
        let conn = self.conn.lock().await;

        // Store entities
        for entity in knowledge.entities {
            store_entity(&conn, agent_id, entity)?;
        }

        // Store relationships
        for relationship in knowledge.relationships {
            store_relationship(&conn, agent_id, relationship)?;
        }

        Ok(())
    }

    /// Get all entities for an agent
    pub async fn get_entities(&self, agent_id: &str) -> GraphResult<Vec<Entity>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities WHERE agent_id = ?1"
        ).map_err(|e| GraphError::Database(e))?;

        let rows = stmt.query_map(params![agent_id], |row| {
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
        }).map_err(|e| GraphError::Database(e))?;

        let mut entities = Vec::new();
        for row in rows {
            let (id, agent_id, entity_type_str, name, properties_json, first_seen_at, last_seen_at, mention_count) = row?;

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

    /// Get all relationships for an agent
    pub async fn get_relationships(&self, agent_id: &str) -> GraphResult<Vec<Relationship>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_relationships WHERE agent_id = ?1"
        ).map_err(|e| GraphError::Database(e))?;

        let rows = stmt.query_map(params![agent_id], |row| {
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
        }).map_err(|e| GraphError::Database(e))?;

        let mut relationships = Vec::new();
        for row in rows {
            let (id, agent_id, source_entity_id, target_entity_id, rel_type_str, properties_json, first_seen_at, last_seen_at, mention_count) = row?;

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

    /// Search entities by name
    pub async fn search_entities(&self, agent_id: &str, query: &str) -> GraphResult<Vec<Entity>> {
        let conn = self.conn.lock().await;

        let mut stmt = conn.prepare(
            "SELECT id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count
             FROM kg_entities
             WHERE agent_id = ?1 AND name LIKE ?2
             ORDER BY mention_count DESC"
        ).map_err(|e| GraphError::Database(e))?;

        let pattern = format!("%{}%", query);
        let rows = stmt.query_map(params![agent_id, pattern], |row| {
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
        }).map_err(|e| GraphError::Database(e))?;

        let mut entities = Vec::new();
        for row in rows {
            let (id, agent_id, entity_type_str, name, properties_json, first_seen_at, last_seen_at, mention_count) = row?;

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

    /// Delete all data for an agent
    pub async fn delete_agent_data(&self, agent_id: &str) -> GraphResult<usize> {
        let conn = self.conn.lock().await;

        // Delete relationships
        let rel_count = conn.execute(
            "DELETE FROM kg_relationships WHERE agent_id = ?1",
            params![agent_id],
        ).map_err(|e| GraphError::Database(e))?;

        // Delete entities
        let ent_count = conn.execute(
            "DELETE FROM kg_entities WHERE agent_id = ?1",
            params![agent_id],
        ).map_err(|e| GraphError::Database(e))?;

        Ok((rel_count + ent_count) as usize)
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
    ).map_err(|e| GraphError::Database(e))?;

    // Create indexes for entities
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entities_agent ON kg_entities(agent_id)",
        [],
    ).map_err(|e| GraphError::Database(e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entities_name ON kg_entities(name)",
        [],
    ).map_err(|e| GraphError::Database(e))?;

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
    ).map_err(|e| GraphError::Database(e))?;

    // Create indexes for relationships
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relationships_agent ON kg_relationships(agent_id)",
        [],
    ).map_err(|e| GraphError::Database(e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relationships_source ON kg_relationships(source_entity_id)",
        [],
    ).map_err(|e| GraphError::Database(e))?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relationships_target ON kg_relationships(target_entity_id)",
        [],
    ).map_err(|e| GraphError::Database(e))?;

    // Migration: Add mention_count column if it doesn't exist (for databases created before this feature)
    // Check if mention_count column exists in kg_entities
    let has_entities_mention_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('kg_entities') WHERE name='mention_count'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if has_entities_mention_count == 0 {
        tracing::info!("Migrating kg_entities: adding mention_count column");
        conn.execute(
            "ALTER TABLE kg_entities ADD COLUMN mention_count INTEGER DEFAULT 1",
            [],
        ).map_err(|e| GraphError::Database(e))?;
    }

    // Check if mention_count column exists in kg_relationships
    let has_relationships_mention_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('kg_relationships') WHERE name='mention_count'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if has_relationships_mention_count == 0 {
        tracing::info!("Migrating kg_relationships: adding mention_count column");
        conn.execute(
            "ALTER TABLE kg_relationships ADD COLUMN mention_count INTEGER DEFAULT 1",
            [],
        ).map_err(|e| GraphError::Database(e))?;
    }

    Ok(())
}

/// Store an entity (upsert based on agent_id + entity_type + name)
fn store_entity(conn: &Connection, agent_id: &str, entity: Entity) -> GraphResult<()> {
    let entity_type_str = entity.entity_type.as_str();
    let properties_json = serde_json::to_string(&entity.properties)
        .unwrap_or_else(|_| "".to_string());

    conn.execute(
        "INSERT INTO kg_entities (id, agent_id, entity_type, name, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            mention_count = mention_count + 1,
            properties = excluded.properties",
        params![
            entity.id,
            agent_id,
            entity_type_str,
            entity.name,
            properties_json,
            entity.first_seen_at.to_rfc3339(),
            entity.last_seen_at.to_rfc3339(),
            entity.mention_count,
        ],
    ).map_err(|e| GraphError::Database(e))?;

    Ok(())
}

/// Store a relationship (upsert based on source + target + type)
fn store_relationship(conn: &Connection, agent_id: &str, relationship: Relationship) -> GraphResult<()> {
    let rel_type_str = relationship.relationship_type.as_str();
    let properties_json = serde_json::to_string(&relationship.properties)
        .unwrap_or_else(|_| "".to_string());

    conn.execute(
        "INSERT INTO kg_relationships (id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
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
    ).map_err(|e| GraphError::Database(e))?;

    Ok(())
}
