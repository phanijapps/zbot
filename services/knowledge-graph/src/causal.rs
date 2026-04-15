//! # Causal Edge Store
//!
//! CRUD operations for causal edges in the knowledge graph.
//! Causal edges represent cause-effect relationships between entities.

use crate::error::{GraphError, GraphResult};
use rusqlite::params;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A causal relationship between two knowledge graph entities.
#[derive(Debug, Clone)]
pub struct CausalEdge {
    pub id: String,
    pub agent_id: String,
    pub cause_entity_id: String,
    pub effect_entity_id: String,
    pub relationship: String,
    pub confidence: f64,
    pub session_id: Option<String>,
    pub created_at: String,
}

/// CRUD operations for causal edges in the knowledge graph.
pub struct CausalEdgeStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl CausalEdgeStore {
    /// Create a new causal edge store with a shared database connection.
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    /// Store a causal edge. Skips if duplicate (same primary key).
    pub async fn store_edge(&self, edge: &CausalEdge) -> GraphResult<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR IGNORE INTO kg_causal_edges \
             (id, agent_id, cause_entity_id, effect_entity_id, relationship, confidence, session_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                edge.id,
                edge.agent_id,
                edge.cause_entity_id,
                edge.effect_entity_id,
                edge.relationship,
                edge.confidence,
                edge.session_id,
                edge.created_at,
            ],
        )
        .map_err(GraphError::Database)?;
        Ok(())
    }

    /// Get causal edges where the given entity is the cause.
    pub async fn get_effects(&self, entity_id: &str) -> GraphResult<Vec<CausalEdge>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, cause_entity_id, effect_entity_id, relationship, confidence, session_id, created_at \
                 FROM kg_causal_edges WHERE cause_entity_id = ?1",
            )
            .map_err(GraphError::Database)?;

        let edges = stmt
            .query_map(params![entity_id], |row| {
                Ok(CausalEdge {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    cause_entity_id: row.get(2)?,
                    effect_entity_id: row.get(3)?,
                    relationship: row.get(4)?,
                    confidence: row.get(5)?,
                    session_id: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(GraphError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(GraphError::Database)?;

        Ok(edges)
    }

    /// Get causal edges where the given entity is the effect.
    pub async fn get_causes(&self, entity_id: &str) -> GraphResult<Vec<CausalEdge>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, cause_entity_id, effect_entity_id, relationship, confidence, session_id, created_at \
                 FROM kg_causal_edges WHERE effect_entity_id = ?1",
            )
            .map_err(GraphError::Database)?;

        let edges = stmt
            .query_map(params![entity_id], |row| {
                Ok(CausalEdge {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    cause_entity_id: row.get(2)?,
                    effect_entity_id: row.get(3)?,
                    relationship: row.get(4)?,
                    confidence: row.get(5)?,
                    session_id: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(GraphError::Database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(GraphError::Database)?;

        Ok(edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    async fn setup_test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE kg_entities (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                name TEXT NOT NULL,
                properties TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                mention_count INTEGER DEFAULT 1
            );
            CREATE TABLE kg_causal_edges (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                cause_entity_id TEXT NOT NULL,
                effect_entity_id TEXT NOT NULL,
                relationship TEXT NOT NULL,
                confidence REAL DEFAULT 0.7,
                session_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (cause_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
                FOREIGN KEY (effect_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
            );
            INSERT INTO kg_entities VALUES ('e1', 'root', 'pattern', 'rate_limiting', NULL, '2026-01-01', '2026-01-01', 1);
            INSERT INTO kg_entities VALUES ('e2', 'root', 'outcome', 'api_ban', NULL, '2026-01-01', '2026-01-01', 1);",
        )
        .unwrap();
        Arc::new(Mutex::new(conn))
    }

    fn make_edge(id: &str, session_id: Option<&str>) -> CausalEdge {
        CausalEdge {
            id: id.to_string(),
            agent_id: "root".to_string(),
            cause_entity_id: "e1".to_string(),
            effect_entity_id: "e2".to_string(),
            relationship: "prevents".to_string(),
            confidence: 0.9,
            session_id: session_id.map(String::from),
            created_at: "2026-04-11".to_string(),
        }
    }

    #[tokio::test]
    async fn test_store_and_get_effects() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let edge = make_edge("ce1", Some("sess-1"));
        store.store_edge(&edge).await.unwrap();

        let effects = store.get_effects("e1").await.unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].relationship, "prevents");
        assert_eq!(effects[0].effect_entity_id, "e2");
    }

    #[tokio::test]
    async fn test_get_causes() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let edge = make_edge("ce1", None);
        store.store_edge(&edge).await.unwrap();

        let causes = store.get_causes("e2").await.unwrap();
        assert_eq!(causes.len(), 1);
        assert_eq!(causes[0].cause_entity_id, "e1");
    }

    #[tokio::test]
    async fn test_duplicate_edge_ignored() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let edge = make_edge("ce1", None);
        store.store_edge(&edge).await.unwrap();
        // Duplicate insert should not fail
        store.store_edge(&edge).await.unwrap();

        let effects = store.get_effects("e1").await.unwrap();
        assert_eq!(effects.len(), 1);
    }

    #[tokio::test]
    async fn test_no_edges_returns_empty() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let effects = store.get_effects("nonexistent").await.unwrap();
        assert!(effects.is_empty());
    }
}
