//! # State Repository
//!
//! Database operations for sessions and agent executions.

use crate::types::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;

// ============================================================================
// DATABASE PROVIDER TRAIT
// ============================================================================

/// Trait for database connection access.
pub trait StateDbProvider: Send + Sync {
    fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>;
}

// ============================================================================
// REPOSITORY
// ============================================================================

/// Repository for session and execution operations.
pub struct StateRepository<D: StateDbProvider> {
    db: Arc<D>,
}

impl<D: StateDbProvider> StateRepository<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    // =========================================================================
    // SESSION - CREATE
    // =========================================================================

    /// Create a new session.
    pub fn create_session(&self, session: &Session) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions (
                    id, status, root_agent_id, title,
                    created_at, started_at, completed_at,
                    total_tokens_in, total_tokens_out, metadata
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    session.id,
                    session.status.as_str(),
                    session.root_agent_id,
                    session.title,
                    session.created_at,
                    session.started_at,
                    session.completed_at,
                    session.total_tokens_in as i64,
                    session.total_tokens_out as i64,
                    session.metadata.as_ref().map(|m| serde_json::to_string(m).ok()).flatten(),
                ],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // SESSION - READ
    // =========================================================================

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Result<Option<Session>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, status, root_agent_id, title,
                        created_at, started_at, completed_at,
                        total_tokens_in, total_tokens_out, metadata
                 FROM sessions WHERE id = ?",
            )?;

            let session = stmt
                .query_row(params![id], |row| Self::row_to_session(row))
                .optional()?;

            Ok(session)
        })
    }

    /// List sessions with filtering.
    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<Session>, String> {
        self.db.with_connection(|conn| {
            let mut sql = String::from(
                "SELECT id, status, root_agent_id, title,
                        created_at, started_at, completed_at,
                        total_tokens_in, total_tokens_out, metadata
                 FROM sessions WHERE 1=1",
            );

            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                params_vec.push(Box::new(status.as_str().to_string()));
            }

            if let Some(root_agent_id) = &filter.root_agent_id {
                sql.push_str(" AND root_agent_id = ?");
                params_vec.push(Box::new(root_agent_id.clone()));
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

    /// Get session with all its executions.
    pub fn get_session_with_executions(&self, id: &str) -> Result<Option<SessionWithExecutions>, String> {
        let session = self.get_session(id)?;

        match session {
            Some(session) => {
                let executions = self.list_executions(&ExecutionFilter {
                    session_id: Some(id.to_string()),
                    ..Default::default()
                })?;

                let subagent_count = executions.iter()
                    .filter(|e| e.delegation_type != DelegationType::Root)
                    .count() as u32;

                Ok(Some(SessionWithExecutions {
                    session,
                    executions,
                    subagent_count,
                }))
            }
            None => Ok(None),
        }
    }

    /// List sessions with their executions (for dashboard).
    pub fn list_sessions_with_executions(
        &self,
        filter: &SessionFilter,
    ) -> Result<Vec<SessionWithExecutions>, String> {
        let sessions = self.list_sessions(filter)?;

        let mut result = Vec::with_capacity(sessions.len());
        for session in sessions {
            let executions = self.list_executions(&ExecutionFilter {
                session_id: Some(session.id.clone()),
                ..Default::default()
            })?;

            let subagent_count = executions.iter()
                .filter(|e| e.delegation_type != DelegationType::Root)
                .count() as u32;

            result.push(SessionWithExecutions {
                session,
                executions,
                subagent_count,
            });
        }

        Ok(result)
    }

    // =========================================================================
    // SESSION - UPDATE
    // =========================================================================

    /// Update session status.
    pub fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();

        self.db.with_connection(|conn| {
            if status == SessionStatus::Running {
                conn.execute(
                    "UPDATE sessions
                     SET status = ?1, started_at = COALESCE(started_at, ?2)
                     WHERE id = ?3",
                    params![status.as_str(), now, id],
                )?;
            } else if status.is_terminal() {
                conn.execute(
                    "UPDATE sessions SET status = ?1, completed_at = ?2 WHERE id = ?3",
                    params![status.as_str(), now, id],
                )?;
            } else {
                conn.execute(
                    "UPDATE sessions SET status = ?1 WHERE id = ?2",
                    params![status.as_str(), id],
                )?;
            }
            Ok(())
        })
    }

    /// Update session token totals.
    pub fn update_session_tokens(&self, id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET
                    total_tokens_in = (SELECT COALESCE(SUM(tokens_in), 0) FROM agent_executions WHERE session_id = ?1),
                    total_tokens_out = (SELECT COALESCE(SUM(tokens_out), 0) FROM agent_executions WHERE session_id = ?1)
                 WHERE id = ?1",
                params![id],
            )?;
            Ok(())
        })
    }

    /// Update session title.
    pub fn update_session_title(&self, id: &str, title: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET title = ?1 WHERE id = ?2",
                params![title, id],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // SESSION - DELETE
    // =========================================================================

    /// Delete a session (cascades to executions and messages).
    pub fn delete_session(&self, id: &str) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let count = conn.execute("DELETE FROM sessions WHERE id = ?", params![id])?;
            Ok(count > 0)
        })
    }

    /// Delete sessions older than a given timestamp.
    /// Returns the number of deleted sessions.
    pub fn delete_old_sessions(&self, older_than: &str) -> Result<u64, String> {
        self.db.with_connection(|conn| {
            // Use created_at since started_at can be NULL
            let count = conn.execute(
                "DELETE FROM sessions WHERE created_at < ?",
                params![older_than],
            )?;
            Ok(count as u64)
        })
    }

    // =========================================================================
    // EXECUTION - CREATE
    // =========================================================================

    /// Create a new agent execution.
    pub fn create_execution(&self, execution: &AgentExecution) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO agent_executions (
                    id, session_id, agent_id, parent_execution_id,
                    delegation_type, task, status,
                    started_at, completed_at,
                    tokens_in, tokens_out, checkpoint, error, log_path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    execution.id,
                    execution.session_id,
                    execution.agent_id,
                    execution.parent_execution_id,
                    execution.delegation_type.as_str(),
                    execution.task,
                    execution.status.as_str(),
                    execution.started_at,
                    execution.completed_at,
                    execution.tokens_in as i64,
                    execution.tokens_out as i64,
                    execution.checkpoint.as_ref().map(|c| serde_json::to_string(c).ok()).flatten(),
                    execution.error,
                    execution.log_path,
                ],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // EXECUTION - READ
    // =========================================================================

    /// Get an execution by ID.
    pub fn get_execution(&self, id: &str) -> Result<Option<AgentExecution>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, parent_execution_id,
                        delegation_type, task, status,
                        started_at, completed_at,
                        tokens_in, tokens_out, checkpoint, error, log_path
                 FROM agent_executions WHERE id = ?",
            )?;

            let execution = stmt
                .query_row(params![id], |row| Self::row_to_execution(row))
                .optional()?;

            Ok(execution)
        })
    }

    /// List executions with filtering.
    pub fn list_executions(&self, filter: &ExecutionFilter) -> Result<Vec<AgentExecution>, String> {
        self.db.with_connection(|conn| {
            let mut sql = String::from(
                "SELECT id, session_id, agent_id, parent_execution_id,
                        delegation_type, task, status,
                        started_at, completed_at,
                        tokens_in, tokens_out, checkpoint, error, log_path
                 FROM agent_executions WHERE 1=1",
            );

            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(session_id) = &filter.session_id {
                sql.push_str(" AND session_id = ?");
                params_vec.push(Box::new(session_id.clone()));
            }

            if let Some(agent_id) = &filter.agent_id {
                sql.push_str(" AND agent_id = ?");
                params_vec.push(Box::new(agent_id.clone()));
            }

            if let Some(status) = &filter.status {
                sql.push_str(" AND status = ?");
                params_vec.push(Box::new(status.as_str().to_string()));
            }

            if let Some(parent_id) = &filter.parent_execution_id {
                sql.push_str(" AND parent_execution_id = ?");
                params_vec.push(Box::new(parent_id.clone()));
            }

            sql.push_str(" ORDER BY started_at ASC");

            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }

            if let Some(offset) = filter.offset {
                sql.push_str(&format!(" OFFSET {}", offset));
            }

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let executions = stmt
                .query_map(params_refs.as_slice(), |row| Self::row_to_execution(row))?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(executions)
        })
    }

    /// Get child executions for a parent.
    pub fn get_child_executions(&self, parent_execution_id: &str) -> Result<Vec<AgentExecution>, String> {
        self.list_executions(&ExecutionFilter {
            parent_execution_id: Some(parent_execution_id.to_string()),
            ..Default::default()
        })
    }

    /// Get root execution for a session.
    pub fn get_root_execution(&self, session_id: &str) -> Result<Option<AgentExecution>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, parent_execution_id,
                        delegation_type, task, status,
                        started_at, completed_at,
                        tokens_in, tokens_out, checkpoint, error, log_path
                 FROM agent_executions
                 WHERE session_id = ? AND delegation_type = 'root'",
            )?;

            let execution = stmt
                .query_row(params![session_id], |row| Self::row_to_execution(row))
                .optional()?;

            Ok(execution)
        })
    }

    // =========================================================================
    // EXECUTION - UPDATE
    // =========================================================================

    /// Update execution status.
    pub fn update_execution_status(&self, id: &str, status: ExecutionStatus) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();

        self.db.with_connection(|conn| {
            if status == ExecutionStatus::Running {
                conn.execute(
                    "UPDATE agent_executions
                     SET status = ?1, started_at = COALESCE(started_at, ?2)
                     WHERE id = ?3",
                    params![status.as_str(), now, id],
                )?;
            } else if status.is_terminal() {
                conn.execute(
                    "UPDATE agent_executions SET status = ?1, completed_at = ?2 WHERE id = ?3",
                    params![status.as_str(), now, id],
                )?;
            } else {
                conn.execute(
                    "UPDATE agent_executions SET status = ?1 WHERE id = ?2",
                    params![status.as_str(), id],
                )?;
            }
            Ok(())
        })
    }

    /// Update execution tokens.
    pub fn update_execution_tokens(&self, id: &str, tokens_in: u64, tokens_out: u64) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_executions SET tokens_in = ?1, tokens_out = ?2 WHERE id = ?3",
                params![tokens_in as i64, tokens_out as i64, id],
            )?;
            Ok(())
        })
    }

    /// Save execution checkpoint.
    pub fn save_execution_checkpoint(&self, id: &str, checkpoint: &Checkpoint) -> Result<(), String> {
        let json = serde_json::to_string(checkpoint)
            .map_err(|e| format!("Failed to serialize checkpoint: {}", e))?;

        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_executions SET checkpoint = ?1 WHERE id = ?2",
                params![json, id],
            )?;
            Ok(())
        })
    }

    /// Set execution error.
    pub fn set_execution_error(&self, id: &str, error: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_executions SET error = ?1 WHERE id = ?2",
                params![error, id],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // STATS - DASHBOARD
    // =========================================================================

    /// Get dashboard stats (pre-computed, ready to display).
    pub fn get_dashboard_stats(&self) -> Result<DashboardStats, String> {
        self.db.with_connection(|conn| {
            // Session counts by status
            let mut stmt = conn.prepare(
                "SELECT status, COUNT(*) FROM sessions GROUP BY status",
            )?;

            let mut running = 0u64;
            let mut paused = 0u64;
            let mut completed = 0u64;
            let mut crashed = 0u64;

            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;

            for row in rows {
                let (status, count) = row?;
                match status.as_str() {
                    "running" => running = count,
                    "paused" => paused = count,
                    "completed" => completed = count,
                    "crashed" => crashed = count,
                    _ => {}
                }
            }

            // Today's stats
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let mut stmt = conn.prepare(
                "SELECT COUNT(*), COALESCE(SUM(total_tokens_in + total_tokens_out), 0)
                 FROM sessions WHERE DATE(created_at) = ?",
            )?;

            let (today_count, today_tokens) = stmt.query_row(params![today], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
            })?;

            Ok(DashboardStats {
                running,
                paused,
                completed,
                crashed,
                today_count,
                today_tokens,
            })
        })
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    fn row_to_session(row: &rusqlite::Row) -> Result<Session, rusqlite::Error> {
        let status_str: String = row.get(1)?;
        let metadata_json: Option<String> = row.get(9)?;

        Ok(Session {
            id: row.get(0)?,
            status: status_str.parse().unwrap_or(SessionStatus::Running),
            root_agent_id: row.get(2)?,
            title: row.get(3)?,
            created_at: row.get(4)?,
            started_at: row.get(5)?,
            completed_at: row.get(6)?,
            total_tokens_in: row.get::<_, i64>(7)? as u64,
            total_tokens_out: row.get::<_, i64>(8)? as u64,
            metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
        })
    }

    fn row_to_execution(row: &rusqlite::Row) -> Result<AgentExecution, rusqlite::Error> {
        let delegation_type_str: String = row.get(4)?;
        let status_str: String = row.get(6)?;
        let checkpoint_json: Option<String> = row.get(11)?;

        Ok(AgentExecution {
            id: row.get(0)?,
            session_id: row.get(1)?,
            agent_id: row.get(2)?,
            parent_execution_id: row.get(3)?,
            delegation_type: delegation_type_str.parse().unwrap_or(DelegationType::Root),
            task: row.get(5)?,
            status: status_str.parse().unwrap_or(ExecutionStatus::Queued),
            started_at: row.get(7)?,
            completed_at: row.get(8)?,
            tokens_in: row.get::<_, i64>(9)? as u64,
            tokens_out: row.get::<_, i64>(10)? as u64,
            checkpoint: checkpoint_json.and_then(|s| serde_json::from_str(&s).ok()),
            error: row.get(12)?,
            log_path: row.get(13)?,
        })
    }
}
