// ============================================================================
// EPISODE REPOSITORY
// CRUD and similarity search for the session_episodes table
// ============================================================================

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::DatabaseManager;

// ============================================================================
// TYPES
// ============================================================================

/// A session episode capturing what happened, what worked, and what was learned.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionEpisode {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    /// `'__global__'` or a specific ward name.
    pub ward_id: String,
    pub task_summary: String,
    /// One of: `'success'`, `'partial'`, `'failed'`, `'crashed'`.
    pub outcome: String,
    pub strategy_used: Option<String>,
    pub key_learnings: Option<String>,
    pub token_cost: Option<i64>,
    /// Raw f32 embedding (little-endian). `None` if not yet embedded.
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
    pub created_at: String,
}

// ============================================================================
// EPISODE REPOSITORY
// ============================================================================

/// Repository for session episode operations.
pub struct EpisodeRepository {
    db: Arc<DatabaseManager>,
}

impl EpisodeRepository {
    /// Create a new episode repository.
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    // =========================================================================
    // CRUD
    // =========================================================================

    /// Insert a new session episode.
    pub fn insert(&self, episode: &SessionEpisode) -> Result<(), String> {
        let embedding_blob = episode.embedding.as_ref().map(|v| f32_vec_to_blob(v));

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO session_episodes (id, session_id, agent_id, ward_id, task_summary, outcome, strategy_used, key_learnings, token_cost, embedding, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    episode.id,
                    episode.session_id,
                    episode.agent_id,
                    episode.ward_id,
                    episode.task_summary,
                    episode.outcome,
                    episode.strategy_used,
                    episode.key_learnings,
                    episode.token_cost,
                    embedding_blob,
                    episode.created_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Get a session episode by session_id.
    pub fn get_by_session_id(&self, session_id: &str) -> Result<Option<SessionEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, ward_id, task_summary, outcome,
                        strategy_used, key_learnings, token_cost, embedding, created_at
                 FROM session_episodes
                 WHERE session_id = ?1",
            )?;

            let result = stmt.query_row(params![session_id], |row| row_to_episode(row));

            match result {
                Ok(ep) => Ok(Some(ep)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Get episodes for an agent, optionally filtered by ward.
    pub fn get_by_agent(
        &self,
        agent_id: &str,
        ward_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SessionEpisode>, String> {
        self.db.with_connection(|conn| {
            let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
                if let Some(ward) = ward_id {
                    (
                        "SELECT id, session_id, agent_id, ward_id, task_summary, outcome,
                                strategy_used, key_learnings, token_cost, embedding, created_at
                         FROM session_episodes
                         WHERE agent_id = ?1 AND ward_id = ?2
                         ORDER BY created_at DESC
                         LIMIT ?3"
                            .to_string(),
                        vec![
                            Box::new(agent_id.to_string()),
                            Box::new(ward.to_string()),
                            Box::new(limit as i64),
                        ],
                    )
                } else {
                    (
                        "SELECT id, session_id, agent_id, ward_id, task_summary, outcome,
                                strategy_used, key_learnings, token_cost, embedding, created_at
                         FROM session_episodes
                         WHERE agent_id = ?1
                         ORDER BY created_at DESC
                         LIMIT ?2"
                            .to_string(),
                        vec![Box::new(agent_id.to_string()), Box::new(limit as i64)],
                    )
                };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |row| row_to_episode(row))?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get successful episodes for an agent.
    pub fn get_successful_by_agent(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<SessionEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, ward_id, task_summary, outcome,
                        strategy_used, key_learnings, token_cost, embedding, created_at
                 FROM session_episodes
                 WHERE agent_id = ?1 AND outcome = 'success'
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows =
                stmt.query_map(params![agent_id, limit as i64], |row| row_to_episode(row))?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    // =========================================================================
    // SIMILARITY SEARCH
    // =========================================================================

    /// Search episodes by vector cosine similarity (brute-force).
    ///
    /// Loads all embeddings for the agent, computes cosine similarity in Rust,
    /// and returns the top-K episodes above the given threshold.
    pub fn search_by_similarity(
        &self,
        agent_id: &str,
        query_embedding: &[f32],
        threshold: f64,
        limit: usize,
    ) -> Result<Vec<(SessionEpisode, f64)>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, ward_id, task_summary, outcome,
                        strategy_used, key_learnings, token_cost, embedding, created_at
                 FROM session_episodes
                 WHERE agent_id = ?1 AND embedding IS NOT NULL",
            )?;

            let rows = stmt.query_map(params![agent_id], |row| row_to_episode(row))?;

            let mut scored: Vec<(SessionEpisode, f64)> = rows
                .filter_map(|r| r.ok())
                .filter_map(|ep| {
                    let emb = ep.embedding.as_ref()?;
                    let sim = cosine_similarity(query_embedding, emb);
                    if sim >= threshold {
                        Some((ep, sim))
                    } else {
                        None
                    }
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            Ok(scored)
        })
    }

    // =========================================================================
    // COUNTS
    // =========================================================================

    /// Count all session episodes.
    pub fn count(&self) -> Result<i64, String> {
        self.db.with_connection(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM session_episodes", [], |row| {
                    row.get(0)
                })?;
            Ok(count)
        })
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Map a database row to a SessionEpisode struct.
fn row_to_episode(row: &rusqlite::Row) -> Result<SessionEpisode, rusqlite::Error> {
    let embedding_blob: Option<Vec<u8>> = row.get(9)?;
    let embedding = embedding_blob.map(|b| blob_to_f32_vec(&b));

    Ok(SessionEpisode {
        id: row.get(0)?,
        session_id: row.get(1)?,
        agent_id: row.get(2)?,
        ward_id: row.get(3)?,
        task_summary: row.get(4)?,
        outcome: row.get(5)?,
        strategy_used: row.get(6)?,
        key_learnings: row.get(7)?,
        token_cost: row.get(8)?,
        embedding,
        created_at: row.get(10)?,
    })
}

/// Convert f32 vector to raw bytes (little-endian) for SQLite BLOB storage.
fn f32_vec_to_blob(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert raw bytes (little-endian) back to f32 vector.
fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;

    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Arc<DatabaseManager> {
        use gateway_services::VaultPaths;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep();
        let db = DatabaseManager::new(paths).unwrap();
        Arc::new(db)
    }

    fn make_episode(agent_id: &str, session_id: &str, outcome: &str) -> SessionEpisode {
        SessionEpisode {
            id: format!("ep-{}", uuid::Uuid::new_v4()),
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            ward_id: "__global__".to_string(),
            task_summary: format!("Test task for session {session_id}"),
            outcome: outcome.to_string(),
            strategy_used: Some("direct".to_string()),
            key_learnings: Some("learned something".to_string()),
            token_cost: Some(500),
            embedding: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn test_insert_and_get_by_session() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        let ep = make_episode("agent-1", "sess-001", "success");
        repo.insert(&ep).unwrap();

        let found = repo.get_by_session_id("sess-001").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.session_id, "sess-001");
        assert_eq!(found.agent_id, "agent-1");
        assert_eq!(found.outcome, "success");
    }

    #[test]
    fn test_get_by_session_not_found() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        let found = repo.get_by_session_id("nonexistent").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_get_by_agent() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        repo.insert(&make_episode("agent-1", "sess-001", "success"))
            .unwrap();
        repo.insert(&make_episode("agent-1", "sess-002", "failed"))
            .unwrap();
        repo.insert(&make_episode("agent-2", "sess-003", "success"))
            .unwrap();

        let results = repo.get_by_agent("agent-1", None, 10).unwrap();
        assert_eq!(results.len(), 2);

        let results = repo.get_by_agent("agent-2", None, 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_get_by_agent_with_ward_filter() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        let mut ep1 = make_episode("agent-1", "sess-001", "success");
        ep1.ward_id = "finance".to_string();
        repo.insert(&ep1).unwrap();

        let mut ep2 = make_episode("agent-1", "sess-002", "success");
        ep2.ward_id = "hr".to_string();
        repo.insert(&ep2).unwrap();

        repo.insert(&make_episode("agent-1", "sess-003", "success"))
            .unwrap();

        let finance = repo.get_by_agent("agent-1", Some("finance"), 10).unwrap();
        assert_eq!(finance.len(), 1);
        assert_eq!(finance[0].ward_id, "finance");

        let global = repo
            .get_by_agent("agent-1", Some("__global__"), 10)
            .unwrap();
        assert_eq!(global.len(), 1);
    }

    #[test]
    fn test_get_successful_by_agent() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        repo.insert(&make_episode("agent-1", "sess-001", "success"))
            .unwrap();
        repo.insert(&make_episode("agent-1", "sess-002", "failed"))
            .unwrap();
        repo.insert(&make_episode("agent-1", "sess-003", "success"))
            .unwrap();
        repo.insert(&make_episode("agent-1", "sess-004", "partial"))
            .unwrap();

        let successes = repo.get_successful_by_agent("agent-1", 10).unwrap();
        assert_eq!(successes.len(), 2);
        for ep in &successes {
            assert_eq!(ep.outcome, "success");
        }
    }

    #[test]
    fn test_count() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        assert_eq!(repo.count().unwrap(), 0);

        repo.insert(&make_episode("agent-1", "sess-001", "success"))
            .unwrap();
        repo.insert(&make_episode("agent-1", "sess-002", "failed"))
            .unwrap();

        assert_eq!(repo.count().unwrap(), 2);
    }

    #[test]
    fn test_similarity_search() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        let mut ep1 = make_episode("agent-1", "sess-001", "success");
        ep1.embedding = Some(vec![1.0, 0.0, 0.0]);
        repo.insert(&ep1).unwrap();

        let mut ep2 = make_episode("agent-1", "sess-002", "success");
        ep2.embedding = Some(vec![0.0, 1.0, 0.0]);
        repo.insert(&ep2).unwrap();

        let mut ep3 = make_episode("agent-1", "sess-003", "failed");
        ep3.embedding = Some(vec![0.9, 0.1, 0.0]);
        repo.insert(&ep3).unwrap();

        // Query close to ep1 and ep3
        let query = vec![0.95, 0.05, 0.0];
        let results = repo
            .search_by_similarity("agent-1", &query, 0.5, 10)
            .unwrap();

        assert!(results.len() >= 2, "Should find at least ep1 and ep3");
        // First result should be the most similar
        assert!(results[0].1 > results[1].1);
    }

    #[test]
    fn test_similarity_search_threshold() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        let mut ep1 = make_episode("agent-1", "sess-001", "success");
        ep1.embedding = Some(vec![1.0, 0.0, 0.0]);
        repo.insert(&ep1).unwrap();

        let mut ep2 = make_episode("agent-1", "sess-002", "success");
        ep2.embedding = Some(vec![0.0, 1.0, 0.0]);
        repo.insert(&ep2).unwrap();

        // High threshold should filter out the orthogonal vector
        let query = vec![1.0, 0.0, 0.0];
        let results = repo
            .search_by_similarity("agent-1", &query, 0.99, 10)
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.session_id, "sess-001");
    }

    #[test]
    fn test_embedding_roundtrip() {
        let db = create_test_db();
        let repo = EpisodeRepository::new(db);

        let mut ep = make_episode("agent-1", "sess-001", "success");
        ep.embedding = Some(vec![1.5, -2.5, 0.0, 3.14159]);
        repo.insert(&ep).unwrap();

        let found = repo.get_by_session_id("sess-001").unwrap().unwrap();
        let emb = found.embedding.unwrap();
        assert_eq!(emb.len(), 4);
        assert!((emb[0] - 1.5).abs() < 0.0001);
        assert!((emb[1] - (-2.5)).abs() < 0.0001);
        assert!((emb[3] - 3.14159).abs() < 0.001);
    }
}
