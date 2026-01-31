//! # State Repository
//!
//! Database operations for execution sessions.

use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;

// ============================================================================
// DATABASE PROVIDER TRAIT
// ============================================================================

/// Trait for database connection access.
///
/// Gateway implements this with its DatabaseManager, keeping execution-state
/// decoupled from gateway internals.
pub trait StateDbProvider: Send + Sync {
    /// Execute a function with a database connection.
    fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>;
}

// ============================================================================
// REPOSITORY
// ============================================================================

/// Repository for execution session operations.
pub struct StateRepository<D: StateDbProvider> {
    db: Arc<D>,
}

impl<D: StateDbProvider> StateRepository<D> {
    /// Create a new repository.
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    // =========================================================================
    // CREATE
    // =========================================================================

    /// Insert a new session.
    pub fn create_session(&self, session: &ExecutionSession) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO execution_sessions (
                    id, conversation_id, agent_id, parent_session_id,
                    status, created_at, started_at, completed_at,
                    tokens_in, tokens_out, checkpoint, error
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    session.id,
                    session.conversation_id,
                    session.agent_id,
                    session.parent_session_id,
                    session.status.as_str(),
                    session.created_at,
                    session.started_at,
                    session.completed_at,
                    session.tokens_in as i64,
                    session.tokens_out as i64,
                    session.checkpoint.as_ref().map(|c| serde_json::to_string(c).ok()).flatten(),
                    session.error,
                ],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // READ
    // =========================================================================

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Result<Option<ExecutionSession>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, conversation_id, agent_id, parent_session_id,
                    status, created_at, started_at, completed_at,
                    tokens_in, tokens_out, checkpoint, error
                FROM execution_sessions
                WHERE id = ?",
            )?;

            let session = stmt
                .query_row(params![id], |row| Self::row_to_session(row))
                .optional()?;

            Ok(session)
        })
    }

    /// List sessions with optional filtering.
    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<ExecutionSession>, String> {
        self.db.with_connection(|conn| {
            let mut sql = String::from(
                "SELECT
                    id, conversation_id, agent_id, parent_session_id,
                    status, created_at, started_at, completed_at,
                    tokens_in, tokens_out, checkpoint, error
                FROM execution_sessions
                WHERE 1=1",
            );

            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(agent_id) = &filter.agent_id {
                sql.push_str(" AND agent_id = ?");
                params_vec.push(Box::new(agent_id.clone()));
            }

            if let Some(conversation_id) = &filter.conversation_id {
                sql.push_str(" AND conversation_id = ?");
                params_vec.push(Box::new(conversation_id.clone()));
            }

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                params_vec.push(Box::new(status.as_str().to_string()));
            }

            if let Some(parent_id) = &filter.parent_session_id {
                sql.push_str(" AND parent_session_id = ?");
                params_vec.push(Box::new(parent_id.clone()));
            }

            if let Some(from_time) = &filter.from_time {
                sql.push_str(" AND created_at >= ?");
                params_vec.push(Box::new(from_time.clone()));
            }

            if let Some(to_time) = &filter.to_time {
                sql.push_str(" AND created_at <= ?");
                params_vec.push(Box::new(to_time.clone()));
            }

            sql.push_str(" ORDER BY created_at DESC");

            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            } else {
                sql.push_str(" LIMIT 100");
            }

            if let Some(offset) = filter.offset {
                sql.push_str(&format!(" OFFSET {}", offset));
            }

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let sessions = stmt
                .query_map(params_refs.as_slice(), |row| Self::row_to_session(row))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(sessions)
        })
    }

    /// Get sessions by status.
    pub fn get_by_status(&self, status: ExecutionStatus) -> Result<Vec<ExecutionSession>, String> {
        self.list_sessions(&SessionFilter {
            status: Some(status),
            ..Default::default()
        })
    }

    /// Get child sessions for a parent.
    pub fn get_children(&self, parent_session_id: &str) -> Result<Vec<ExecutionSession>, String> {
        self.list_sessions(&SessionFilter {
            parent_session_id: Some(parent_session_id.to_string()),
            ..Default::default()
        })
    }

    /// Get all resumable sessions (paused or crashed).
    pub fn get_resumable(&self) -> Result<Vec<ExecutionSession>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, conversation_id, agent_id, parent_session_id,
                    status, created_at, started_at, completed_at,
                    tokens_in, tokens_out, checkpoint, error
                FROM execution_sessions
                WHERE status IN ('paused', 'crashed')
                ORDER BY created_at DESC",
            )?;

            let sessions = stmt
                .query_map([], |row| Self::row_to_session(row))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(sessions)
        })
    }

    /// Get currently running sessions.
    pub fn get_running(&self) -> Result<Vec<ExecutionSession>, String> {
        self.get_by_status(ExecutionStatus::Running)
    }

    // =========================================================================
    // UPDATE
    // =========================================================================

    /// Update session status.
    pub fn update_status(&self, id: &str, status: ExecutionStatus) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();

        self.db.with_connection(|conn| {
            // Set started_at when transitioning to Running
            if status == ExecutionStatus::Running {
                conn.execute(
                    "UPDATE execution_sessions
                     SET status = ?1, started_at = COALESCE(started_at, ?2)
                     WHERE id = ?3",
                    params![status.as_str(), now, id],
                )?;
            }
            // Set completed_at when transitioning to terminal state
            else if status.is_terminal() {
                conn.execute(
                    "UPDATE execution_sessions
                     SET status = ?1, completed_at = ?2
                     WHERE id = ?3",
                    params![status.as_str(), now, id],
                )?;
            }
            // Otherwise just update status
            else {
                conn.execute(
                    "UPDATE execution_sessions SET status = ?1 WHERE id = ?2",
                    params![status.as_str(), id],
                )?;
            }
            Ok(())
        })
    }

    /// Update token counts.
    pub fn update_tokens(&self, id: &str, tokens_in: u64, tokens_out: u64) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE execution_sessions
                 SET tokens_in = ?1, tokens_out = ?2
                 WHERE id = ?3",
                params![tokens_in as i64, tokens_out as i64, id],
            )?;
            Ok(())
        })
    }

    /// Save a checkpoint.
    pub fn save_checkpoint(&self, id: &str, checkpoint: &Checkpoint) -> Result<(), String> {
        let json = serde_json::to_string(checkpoint)
            .map_err(|e| format!("Failed to serialize checkpoint: {}", e))?;

        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE execution_sessions SET checkpoint = ?1 WHERE id = ?2",
                params![json, id],
            )?;
            Ok(())
        })
    }

    /// Set error message.
    pub fn set_error(&self, id: &str, error: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE execution_sessions SET error = ?1 WHERE id = ?2",
                params![error, id],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // DELETE
    // =========================================================================

    /// Delete a session.
    pub fn delete_session(&self, id: &str) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute(
                "DELETE FROM execution_sessions WHERE id = ?",
                params![id],
            )?;
            Ok(count > 0)
        })
    }

    /// Delete old completed sessions.
    pub fn delete_old_sessions(&self, older_than: &str) -> Result<u64, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute(
                "DELETE FROM execution_sessions
                 WHERE status IN ('completed', 'cancelled')
                 AND completed_at < ?",
                params![older_than],
            )?;
            Ok(count as u64)
        })
    }

    // =========================================================================
    // AGGREGATES
    // =========================================================================

    /// Get daily summary for a date (YYYY-MM-DD).
    pub fn get_daily_summary(&self, date: &str) -> Result<DailySummary, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    COALESCE(SUM(tokens_in), 0) as total_in,
                    COALESCE(SUM(tokens_out), 0) as total_out,
                    COUNT(*) as session_count,
                    SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) as completed,
                    SUM(CASE WHEN status IN ('crashed', 'cancelled') AND error IS NOT NULL THEN 1 ELSE 0 END) as failed
                FROM execution_sessions
                WHERE DATE(created_at) = ?",
            )?;

            let summary = stmt.query_row(params![date], |row| {
                Ok(DailySummary {
                    date: date.to_string(),
                    total_tokens_in: row.get::<_, i64>(0)? as u64,
                    total_tokens_out: row.get::<_, i64>(1)? as u64,
                    session_count: row.get::<_, i64>(2)? as u64,
                    completed_count: row.get::<_, i64>(3)? as u64,
                    failed_count: row.get::<_, i64>(4)? as u64,
                })
            })?;

            Ok(summary)
        })
    }

    /// Get count of sessions by status.
    pub fn get_status_counts(&self) -> Result<std::collections::HashMap<String, u64>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT status, COUNT(*) FROM execution_sessions GROUP BY status",
            )?;

            let mut counts = std::collections::HashMap::new();
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;

            for row in rows {
                let (status, count) = row?;
                counts.insert(status, count);
            }

            Ok(counts)
        })
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    /// Convert a database row to ExecutionSession.
    fn row_to_session(row: &rusqlite::Row) -> Result<ExecutionSession, rusqlite::Error> {
        let status_str: String = row.get(4)?;
        let checkpoint_json: Option<String> = row.get(10)?;

        Ok(ExecutionSession {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            agent_id: row.get(2)?,
            parent_session_id: row.get(3)?,
            status: status_str.parse().unwrap_or(ExecutionStatus::Queued),
            created_at: row.get(5)?,
            started_at: row.get(6)?,
            completed_at: row.get(7)?,
            tokens_in: row.get::<_, i64>(8)? as u64,
            tokens_out: row.get::<_, i64>(9)? as u64,
            checkpoint: checkpoint_json.and_then(|s| serde_json::from_str(&s).ok()),
            error: row.get(11)?,
        })
    }
}
