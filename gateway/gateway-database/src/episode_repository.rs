//! Repository for session episodes with CRUD and vector search.
//!
//! Phase 1b (v22): constructs on `KnowledgeDatabase` and stores embeddings in
//! the `session_episodes_index` vec0 virtual table through the `VectorIndex` trait.
//! The `embedding` column on `session_episodes` is gone; callers write normalized
//! vectors through `insert`, which delegates to the injected `VectorIndex`.
//! Vectors MUST be L2-normalized by the caller.
//!
//! To read an embedding back, use [`EpisodeRepository::get_episode_embedding`].

use crate::vector_index::VectorIndex;
use crate::KnowledgeDatabase;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    /// Raw f32 embedding. Always `None` when loaded from `session_episodes`
    /// (the column was removed in schema v22). Callers may set this to `Some(v)`
    /// prior to `insert` to have the vector persisted through the `VectorIndex`
    /// — vectors MUST be L2-normalized by the caller.
    ///
    /// To read an embedding back, use [`EpisodeRepository::get_episode_embedding`].
    #[serde(skip)]
    pub embedding: Option<Vec<f32>>,
    pub created_at: String,
}

// ============================================================================
// EPISODE REPOSITORY
// ============================================================================

/// Repository for session episode operations.
pub struct EpisodeRepository {
    db: Arc<KnowledgeDatabase>,
    vec_index: Arc<dyn VectorIndex>,
}

impl EpisodeRepository {
    /// Create a new episode repository.
    ///
    /// `vec_index` must wrap the `session_episodes_index` vec0 table (384-dim).
    pub fn new(db: Arc<KnowledgeDatabase>, vec_index: Arc<dyn VectorIndex>) -> Self {
        Self { db, vec_index }
    }

    // =========================================================================
    // CRUD
    // =========================================================================

    /// Insert a new session episode.
    ///
    /// If `episode.embedding` is `Some(v)`, the vector is written to
    /// `session_episodes_index` via the injected `VectorIndex`. **Callers must
    /// L2-normalize the vector first**.
    pub fn insert(&self, episode: &SessionEpisode) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO session_episodes \
                 (id, session_id, agent_id, ward_id, task_summary, outcome, \
                  strategy_used, key_learnings, token_cost, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                    episode.created_at,
                ],
            )?;
            Ok(())
        })?;

        if let Some(emb) = episode.embedding.as_ref() {
            self.vec_index.upsert(&episode.id, emb)?;
        }

        Ok(())
    }

    /// Get a session episode by session_id.
    pub fn get_by_session_id(&self, session_id: &str) -> Result<Option<SessionEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, ward_id, task_summary, outcome, \
                        strategy_used, key_learnings, token_cost, created_at \
                 FROM session_episodes \
                 WHERE session_id = ?1",
            )?;

            let result = stmt.query_row(params![session_id], row_to_episode);

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
                        "SELECT id, session_id, agent_id, ward_id, task_summary, outcome, \
                                strategy_used, key_learnings, token_cost, created_at \
                         FROM session_episodes \
                         WHERE agent_id = ?1 AND ward_id = ?2 \
                         ORDER BY created_at DESC \
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
                        "SELECT id, session_id, agent_id, ward_id, task_summary, outcome, \
                                strategy_used, key_learnings, token_cost, created_at \
                         FROM session_episodes \
                         WHERE agent_id = ?1 \
                         ORDER BY created_at DESC \
                         LIMIT ?2"
                            .to_string(),
                        vec![Box::new(agent_id.to_string()), Box::new(limit as i64)],
                    )
                };

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), row_to_episode)?;
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
                "SELECT id, session_id, agent_id, ward_id, task_summary, outcome, \
                        strategy_used, key_learnings, token_cost, created_at \
                 FROM session_episodes \
                 WHERE agent_id = ?1 AND outcome = 'success' \
                 ORDER BY created_at DESC \
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![agent_id, limit as i64], row_to_episode)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    // =========================================================================
    // SIMILARITY SEARCH
    // =========================================================================

    /// Search episodes by vector similarity for an agent.
    ///
    /// Performs a nearest-neighbor query through `VectorIndex`, then loads the
    /// matching `session_episodes` rows and filters by agent_id in Rust. The
    /// returned score is cosine similarity (`1 - L2_sq / 2`), valid because
    /// stored and query vectors are required to be L2-normalized.
    pub fn search_by_similarity(
        &self,
        agent_id: &str,
        query_embedding: &[f32],
        threshold: f64,
        limit: usize,
    ) -> Result<Vec<(SessionEpisode, f64)>, String> {
        // Over-fetch so post-filtering by agent still returns `limit` hits.
        let fetch = limit.saturating_mul(4).max(limit);
        let nearest = self.vec_index.query_nearest(query_embedding, fetch)?;
        if nearest.is_empty() {
            return Ok(Vec::new());
        }

        let ids: Vec<String> = nearest.iter().map(|(id, _)| id.clone()).collect();
        let dist_by_id: std::collections::HashMap<String, f32> =
            nearest.iter().map(|(id, d)| (id.clone(), *d)).collect();

        let placeholders = (0..ids.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, session_id, agent_id, ward_id, task_summary, outcome, \
             strategy_used, key_learnings, token_cost, created_at \
             FROM session_episodes WHERE id IN ({placeholders})"
        );

        let episodes: Vec<SessionEpisode> = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let params_iter = rusqlite::params_from_iter(ids.iter());
            let rows = stmt.query_map(params_iter, row_to_episode)?;
            Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        })?;

        let mut scored: Vec<(SessionEpisode, f64)> = episodes
            .into_iter()
            .filter(|ep| ep.agent_id == agent_id)
            .filter_map(|ep| {
                let dist = dist_by_id.get(&ep.id).copied().unwrap_or(f32::MAX);
                // L2 squared on normalized vectors → cosine = 1 - dist/2.
                let score = 1.0 - (dist as f64) / 2.0;
                if score >= threshold {
                    Some((ep, score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
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

    // =========================================================================
    // EMBEDDING ACCESS
    // =========================================================================

    /// Fetch the stored embedding for an episode, if present in `session_episodes_index`.
    /// Returns `None` if the episode has never been indexed.
    ///
    /// `sqlite-vec` stores vectors as `FLOAT[N]` BLOBs (little-endian f32s);
    /// we decode the raw bytes back to `Vec<f32>`.
    pub fn get_episode_embedding(&self, episode_id: &str) -> Result<Option<Vec<f32>>, String> {
        self.db.with_connection(|conn| {
            let r = conn.query_row(
                "SELECT embedding FROM session_episodes_index WHERE episode_id = ?1",
                params![episode_id],
                |row| row.get::<_, Vec<u8>>(0),
            );
            match r {
                Ok(blob) => Ok(Some(blob_to_f32_vec(&blob))),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }
}

// ============================================================================
// HELPERS
// ============================================================================

/// Map a database row to a SessionEpisode struct.
fn row_to_episode(row: &rusqlite::Row) -> Result<SessionEpisode, rusqlite::Error> {
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
        embedding: None,
        created_at: row.get(9)?,
    })
}

/// Convert raw bytes (little-endian) back to f32 vector.
fn blob_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_index::SqliteVecIndex;

    fn setup() -> (tempfile::TempDir, EpisodeRepository) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
        let db = Arc::new(crate::KnowledgeDatabase::new(paths).expect("knowledge db"));
        let vec_index: Arc<dyn VectorIndex> = Arc::new(SqliteVecIndex::new(
            db.clone(),
            "session_episodes_index",
            "episode_id",
            384,
        ));
        let repo = EpisodeRepository::new(db, vec_index);
        (tmp, repo)
    }

    fn normalized(v: Vec<f32>) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < 1e-9 {
            v
        } else {
            v.into_iter().map(|x| x / norm).collect()
        }
    }

    fn make_episode(agent_id: &str, session_id: &str, outcome: &str) -> SessionEpisode {
        SessionEpisode {
            id: format!("ep-{session_id}"),
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
        let (_tmp, repo) = setup();

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
        let (_tmp, repo) = setup();

        let found = repo.get_by_session_id("nonexistent").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_get_by_agent() {
        let (_tmp, repo) = setup();

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
        let (_tmp, repo) = setup();

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
        let (_tmp, repo) = setup();

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
        let (_tmp, repo) = setup();

        assert_eq!(repo.count().unwrap(), 0);

        repo.insert(&make_episode("agent-1", "sess-001", "success"))
            .unwrap();
        repo.insert(&make_episode("agent-1", "sess-002", "failed"))
            .unwrap();

        assert_eq!(repo.count().unwrap(), 2);
    }

    #[test]
    fn test_similarity_search_finds_match() {
        let (_tmp, repo) = setup();

        let emb = normalized(
            (0..384)
                .map(|i| if i == 0 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );
        let mut ep = make_episode("agent-1", "sess-001", "success");
        ep.embedding = Some(emb.clone());
        repo.insert(&ep).unwrap();

        let results = repo.search_by_similarity("agent-1", &emb, 0.5, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.99);
    }

    #[test]
    fn test_similarity_search_threshold_filters() {
        let (_tmp, repo) = setup();

        let emb1 = normalized(
            (0..384)
                .map(|i| if i == 0 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );
        let emb2 = normalized(
            (0..384)
                .map(|i| if i == 1 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );

        let mut ep1 = make_episode("agent-1", "sess-001", "success");
        ep1.embedding = Some(emb1.clone());
        repo.insert(&ep1).unwrap();

        let mut ep2 = make_episode("agent-1", "sess-002", "success");
        ep2.embedding = Some(emb2.clone());
        repo.insert(&ep2).unwrap();

        // High threshold should filter out the orthogonal vector
        let results = repo
            .search_by_similarity("agent-1", &emb1, 0.99, 10)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.session_id, "sess-001");
    }

    #[test]
    fn test_get_episode_embedding_roundtrip() {
        let (_tmp, repo) = setup();

        let emb = normalized(
            (0..384)
                .map(|i| if i == 0 { 1.0_f32 } else { 0.0_f32 })
                .collect(),
        );
        let mut ep = make_episode("agent-1", "sess-001", "success");
        ep.embedding = Some(emb.clone());
        repo.insert(&ep).unwrap();

        let fetched_emb = repo.get_episode_embedding("ep-sess-001").unwrap();
        assert!(fetched_emb.is_some());
        let fetched_emb = fetched_emb.unwrap();
        assert_eq!(fetched_emb.len(), 384);
        // First dimension should be ~1.0 (normalized unit vector)
        assert!((fetched_emb[0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_embedding_returns_none_when_not_indexed() {
        let (_tmp, repo) = setup();

        let ep = make_episode("agent-1", "sess-001", "success");
        repo.insert(&ep).unwrap();

        // No embedding was set, so should return None
        let fetched = repo.get_episode_embedding("ep-sess-001").unwrap();
        assert!(fetched.is_none());
    }
}
