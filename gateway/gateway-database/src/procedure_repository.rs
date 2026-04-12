//! Repository for learned procedure patterns with CRUD and vector search.

use crate::connection::DatabaseManager;
use rusqlite::params;
use std::sync::Arc;

/// A learned procedure pattern with execution statistics.
#[derive(Debug, Clone)]
pub struct Procedure {
    pub id: String,
    pub agent_id: String,
    pub ward_id: Option<String>,
    pub name: String,
    pub description: String,
    pub trigger_pattern: Option<String>,
    pub steps: String,
    pub parameters: Option<String>,
    pub success_count: i32,
    pub failure_count: i32,
    pub avg_duration_ms: Option<i64>,
    pub avg_token_cost: Option<i64>,
    pub last_used: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub created_at: String,
    pub updated_at: String,
}

/// Repository for procedure CRUD and vector search.
pub struct ProcedureRepository {
    db: Arc<DatabaseManager>,
}

impl ProcedureRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Insert a new procedure.
    pub fn upsert_procedure(&self, proc: &Procedure) -> Result<(), String> {
        self.db.with_connection(|conn| {
            let embedding_bytes = proc
                .embedding
                .as_ref()
                .map(|e| e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>());

            conn.execute(
                "INSERT INTO procedures \
                 (id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                  parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                  last_used, embedding, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    proc.id,
                    proc.agent_id,
                    proc.ward_id,
                    proc.name,
                    proc.description,
                    proc.trigger_pattern,
                    proc.steps,
                    proc.parameters,
                    proc.success_count,
                    proc.failure_count,
                    proc.avg_duration_ms,
                    proc.avg_token_cost,
                    proc.last_used,
                    embedding_bytes,
                    proc.created_at,
                    proc.updated_at,
                ],
            )?;

            Ok(())
        })
    }

    /// Get a procedure by ID.
    pub fn get_procedure(&self, id: &str) -> Result<Option<Procedure>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                 parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                 last_used, embedding, created_at, updated_at \
                 FROM procedures WHERE id = ?1",
            )?;

            let mut rows = stmt.query_map(params![id], |row| Ok(Self::row_to_procedure(row)))?;

            match rows.next() {
                Some(Ok(proc)) => Ok(Some(proc)),
                Some(Err(e)) => Err(e),
                None => Ok(None),
            }
        })
    }

    /// List procedures for an agent, optionally filtered by ward.
    pub fn list_procedures(
        &self,
        agent_id: &str,
        ward_id: Option<&str>,
    ) -> Result<Vec<Procedure>, String> {
        self.db.with_connection(|conn| {
            if let Some(ward) = ward_id {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                     parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                     last_used, embedding, created_at, updated_at \
                     FROM procedures WHERE agent_id = ?1 AND ward_id = ?2 ORDER BY name",
                )?;
                let procs = stmt
                    .query_map(params![agent_id, ward], |row| {
                        Ok(Self::row_to_procedure(row))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(procs)
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                     parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                     last_used, embedding, created_at, updated_at \
                     FROM procedures WHERE agent_id = ?1 ORDER BY name",
                )?;
                let procs = stmt
                    .query_map(params![agent_id], |row| Ok(Self::row_to_procedure(row)))?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(procs)
            }
        })
    }

    /// Search procedures by embedding similarity for an agent/ward.
    pub fn search_by_similarity(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        ward_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Procedure, f64)>, String> {
        self.db.with_connection(|conn| {
            let procedures: Vec<Procedure> = if let Some(ward) = ward_id {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                     parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                     last_used, embedding, created_at, updated_at \
                     FROM procedures \
                     WHERE agent_id = ?1 AND ward_id = ?2 AND embedding IS NOT NULL",
                )?;
                let results = stmt
                    .query_map(params![agent_id, ward], |row| {
                        Ok(Self::row_to_procedure(row))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();
                results
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, agent_id, ward_id, name, description, trigger_pattern, steps, \
                     parameters, success_count, failure_count, avg_duration_ms, avg_token_cost, \
                     last_used, embedding, created_at, updated_at \
                     FROM procedures \
                     WHERE agent_id = ?1 AND embedding IS NOT NULL",
                )?;
                let results = stmt
                    .query_map(params![agent_id], |row| Ok(Self::row_to_procedure(row)))?
                    .filter_map(|r| r.ok())
                    .collect();
                results
            };

            let mut scored: Vec<(Procedure, f64)> = procedures
                .into_iter()
                .filter_map(|proc| {
                    let embedding = proc.embedding.as_ref()?;
                    let sim = cosine_similarity(query_embedding, embedding);
                    Some((proc, sim))
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit);
            Ok(scored)
        })
    }

    /// Increment success count and update running averages.
    pub fn increment_success(
        &self,
        id: &str,
        duration_ms: Option<i64>,
        token_cost: Option<i64>,
    ) -> Result<(), String> {
        self.db.with_connection(|conn| {
            // Update success_count, last_used, and running averages
            conn.execute(
                "UPDATE procedures SET \
                 success_count = success_count + 1, \
                 last_used = datetime('now'), \
                 avg_duration_ms = CASE \
                     WHEN avg_duration_ms IS NULL THEN ?2 \
                     WHEN ?2 IS NULL THEN avg_duration_ms \
                     ELSE (avg_duration_ms * (success_count - 1) + ?2) / success_count \
                 END, \
                 avg_token_cost = CASE \
                     WHEN avg_token_cost IS NULL THEN ?3 \
                     WHEN ?3 IS NULL THEN avg_token_cost \
                     ELSE (avg_token_cost * (success_count - 1) + ?3) / success_count \
                 END, \
                 updated_at = datetime('now') \
                 WHERE id = ?1",
                params![id, duration_ms, token_cost],
            )?;
            Ok(())
        })
    }

    /// Increment failure count.
    pub fn increment_failure(&self, id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE procedures SET \
                 failure_count = failure_count + 1, \
                 updated_at = datetime('now') \
                 WHERE id = ?1",
                params![id],
            )?;
            Ok(())
        })
    }

    fn row_to_procedure(row: &rusqlite::Row) -> Procedure {
        let embedding_blob: Option<Vec<u8>> = row.get(13).ok().flatten();
        let embedding = embedding_blob.map(|blob| {
            blob.chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        });

        Procedure {
            id: row.get(0).unwrap_or_default(),
            agent_id: row.get(1).unwrap_or_default(),
            ward_id: row.get(2).ok().flatten(),
            name: row.get(3).unwrap_or_default(),
            description: row.get(4).unwrap_or_default(),
            trigger_pattern: row.get(5).ok().flatten(),
            steps: row.get(6).unwrap_or_default(),
            parameters: row.get(7).ok().flatten(),
            success_count: row.get(8).unwrap_or(1),
            failure_count: row.get(9).unwrap_or(0),
            avg_duration_ms: row.get(10).ok().flatten(),
            avg_token_cost: row.get(11).ok().flatten(),
            last_used: row.get(12).ok().flatten(),
            embedding,
            created_at: row.get(14).unwrap_or_default(),
            updated_at: row.get(15).unwrap_or_default(),
        }
    }
}

/// Cosine similarity between two f32 vectors.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Arc<DatabaseManager> {
        use gateway_services::VaultPaths;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep();
        let db = DatabaseManager::new(paths).unwrap();
        Arc::new(db)
    }

    fn make_procedure(id: &str, agent_id: &str, ward_id: Option<&str>) -> Procedure {
        Procedure {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            ward_id: ward_id.map(|s| s.to_string()),
            name: format!("proc-{id}"),
            description: format!("Description for {id}"),
            trigger_pattern: None,
            steps: r#"["step1","step2"]"#.to_string(),
            parameters: None,
            success_count: 1,
            failure_count: 0,
            avg_duration_ms: None,
            avg_token_cost: None,
            last_used: None,
            embedding: None,
            created_at: "2026-04-11".to_string(),
            updated_at: "2026-04-11".to_string(),
        }
    }

    #[test]
    fn test_upsert_and_get_round_trip() {
        let db = setup_test_db();
        let repo = ProcedureRepository::new(db);

        let proc = make_procedure("p1", "root", Some("__global__"));
        repo.upsert_procedure(&proc).unwrap();

        let fetched = repo.get_procedure("p1").unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.name, "proc-p1");
        assert_eq!(fetched.agent_id, "root");
        assert_eq!(fetched.steps, r#"["step1","step2"]"#);
    }

    #[test]
    fn test_search_by_similarity_finds_match() {
        let db = setup_test_db();
        let repo = ProcedureRepository::new(db);

        let mut proc = make_procedure("p1", "root", Some("__global__"));
        proc.embedding = Some(vec![1.0, 0.0, 0.0]);
        repo.upsert_procedure(&proc).unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let results = repo
            .search_by_similarity(&query, "root", Some("__global__"), 5)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].1 > 0.99);
    }

    #[test]
    fn test_increment_success_updates_count() {
        let db = setup_test_db();
        let repo = ProcedureRepository::new(db);

        let proc = make_procedure("p1", "root", Some("__global__"));
        repo.upsert_procedure(&proc).unwrap();

        repo.increment_success("p1", Some(100), Some(500)).unwrap();

        let fetched = repo.get_procedure("p1").unwrap().unwrap();
        assert_eq!(fetched.success_count, 2);
        assert!(fetched.last_used.is_some());
    }

    #[test]
    fn test_increment_failure_updates_count() {
        let db = setup_test_db();
        let repo = ProcedureRepository::new(db);

        let proc = make_procedure("p1", "root", Some("__global__"));
        repo.upsert_procedure(&proc).unwrap();

        repo.increment_failure("p1").unwrap();

        let fetched = repo.get_procedure("p1").unwrap().unwrap();
        assert_eq!(fetched.failure_count, 1);
    }

    #[test]
    fn test_list_procedures_filters_by_agent() {
        let db = setup_test_db();
        let repo = ProcedureRepository::new(db);

        repo.upsert_procedure(&make_procedure("p1", "agent-a", Some("__global__")))
            .unwrap();
        repo.upsert_procedure(&make_procedure("p2", "agent-a", Some("__global__")))
            .unwrap();
        repo.upsert_procedure(&make_procedure("p3", "agent-b", Some("__global__")))
            .unwrap();

        let agent_a = repo.list_procedures("agent-a", None).unwrap();
        assert_eq!(agent_a.len(), 2);

        let agent_b = repo.list_procedures("agent-b", None).unwrap();
        assert_eq!(agent_b.len(), 1);

        // Filter by ward
        let filtered = repo.list_procedures("agent-a", Some("__global__")).unwrap();
        assert_eq!(filtered.len(), 2);

        let empty = repo
            .list_procedures("agent-a", Some("nonexistent"))
            .unwrap();
        assert_eq!(empty.len(), 0);
    }
}
