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
                    id, status, source, root_agent_id, title,
                    created_at, started_at, completed_at,
                    total_tokens_in, total_tokens_out, metadata,
                    pending_delegations, continuation_needed, ward_id,
                    parent_session_id, thread_id, connector_id, respond_to
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    session.id,
                    session.status.as_str(),
                    session.source.as_str(),
                    session.root_agent_id,
                    session.title,
                    session.created_at,
                    session.started_at,
                    session.completed_at,
                    session.total_tokens_in as i64,
                    session.total_tokens_out as i64,
                    session.metadata.as_ref().map(|m| serde_json::to_string(m).ok()).flatten(),
                    session.pending_delegations as i64,
                    session.continuation_needed as i64,
                    session.ward_id,
                    session.parent_session_id,
                    session.thread_id,
                    session.connector_id,
                    session.respond_to.as_ref().map(|r| serde_json::to_string(r).ok()).flatten(),
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
                "SELECT id, status, source, root_agent_id, title,
                        created_at, started_at, completed_at,
                        total_tokens_in, total_tokens_out, metadata,
                        pending_delegations, continuation_needed, ward_id,
                        parent_session_id, thread_id, connector_id, respond_to
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
                "SELECT id, status, source, root_agent_id, title,
                        created_at, started_at, completed_at,
                        total_tokens_in, total_tokens_out, metadata,
                        pending_delegations, continuation_needed, ward_id,
                        parent_session_id, thread_id, connector_id, respond_to
                 FROM sessions WHERE parent_session_id IS NULL",
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

            if let Some(thread_id) = &filter.thread_id {
                sql.push_str(" AND thread_id = ?");
                params_vec.push(Box::new(thread_id.clone()));
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
    pub fn get_session_with_executions(
        &self,
        id: &str,
    ) -> Result<Option<SessionWithExecutions>, String> {
        let session = self.get_session(id)?;

        match session {
            Some(session) => {
                let executions = self.list_executions(&ExecutionFilter {
                    session_id: Some(id.to_string()),
                    ..Default::default()
                })?;

                let subagent_count = executions
                    .iter()
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

            let subagent_count = executions
                .iter()
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
    /// When session becomes terminal (crashed/completed/cancelled), also updates
    /// all running/queued executions to match.
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

                // Cascade status to running/queued executions
                // - If session crashed, mark running executions as crashed
                // - If session completed, mark running executions as completed
                let exec_status = match status {
                    SessionStatus::Crashed => "crashed",
                    SessionStatus::Completed => "completed",
                    _ => "completed", // fallback for any other terminal state
                };
                conn.execute(
                    "UPDATE agent_executions
                     SET status = ?1, completed_at = ?2
                     WHERE session_id = ?3 AND status IN ('running', 'queued')",
                    params![exec_status, now, id],
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

    /// Update session ward (active project directory).
    pub fn update_session_ward(&self, id: &str, ward_id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET ward_id = ?1 WHERE id = ?2",
                params![ward_id, id],
            )?;
            Ok(())
        })
    }

    /// Update session routing fields (thread_id, connector_id, respond_to).
    pub fn update_session_routing(
        &self,
        id: &str,
        thread_id: Option<&str>,
        connector_id: Option<&str>,
        respond_to: Option<&Vec<String>>,
    ) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET thread_id = ?1, connector_id = ?2, respond_to = ?3 WHERE id = ?4",
                params![
                    thread_id,
                    connector_id,
                    respond_to.map(|r| serde_json::to_string(r).ok()).flatten(),
                    id,
                ],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // SESSION - DELEGATION TRACKING
    // =========================================================================

    /// Increment pending delegations count.
    pub fn increment_pending_delegations(&self, session_id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET pending_delegations = pending_delegations + 1 WHERE id = ?1",
                params![session_id],
            )?;
            Ok(())
        })
    }

    /// Decrement pending delegations count, returns new count.
    pub fn decrement_pending_delegations(&self, session_id: &str) -> Result<u32, String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET pending_delegations = MAX(0, pending_delegations - 1) WHERE id = ?1",
                params![session_id],
            )?;
            let count: i64 = conn.query_row(
                "SELECT pending_delegations FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }

    /// Set continuation_needed flag.
    pub fn set_continuation_needed(&self, session_id: &str, needed: bool) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE sessions SET continuation_needed = ?1 WHERE id = ?2",
                params![needed as i32, session_id],
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
                    tokens_in, tokens_out, checkpoint, error, log_path,
                    child_session_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
                    execution
                        .checkpoint
                        .as_ref()
                        .map(|c| serde_json::to_string(c).ok())
                        .flatten(),
                    execution.error,
                    execution.log_path,
                    execution.child_session_id,
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
                        tokens_in, tokens_out, checkpoint, error, log_path,
                        child_session_id
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
                        tokens_in, tokens_out, checkpoint, error, log_path,
                        child_session_id
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
    pub fn get_child_executions(
        &self,
        parent_execution_id: &str,
    ) -> Result<Vec<AgentExecution>, String> {
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
                        tokens_in, tokens_out, checkpoint, error, log_path,
                        child_session_id
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

    /// Cancel a single execution by marking it as cancelled with a completed_at timestamp.
    pub fn cancel_execution(&self, execution_id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_executions SET status = 'cancelled', completed_at = ?1 WHERE id = ?2",
                params![chrono::Utc::now().to_rfc3339(), execution_id],
            )?;
            Ok(())
        })
    }

    /// Update execution tokens.
    pub fn update_execution_tokens(
        &self,
        id: &str,
        tokens_in: u64,
        tokens_out: u64,
    ) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_executions SET tokens_in = ?1, tokens_out = ?2 WHERE id = ?3",
                params![tokens_in as i64, tokens_out as i64, id],
            )?;
            Ok(())
        })
    }

    /// Save execution checkpoint.
    pub fn save_execution_checkpoint(
        &self,
        id: &str,
        checkpoint: &Checkpoint,
    ) -> Result<(), String> {
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

    /// Set child_session_id on an execution (for smart resume).
    pub fn set_child_session_id(
        &self,
        execution_id: &str,
        child_session_id: &str,
    ) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE agent_executions SET child_session_id = ?1 WHERE id = ?2",
                params![child_session_id, execution_id],
            )?;
            Ok(())
        })
    }

    /// Find the most recently crashed subagent execution for a session.
    /// Returns None if only the root execution crashed or no crashes exist.
    pub fn get_last_crashed_subagent(
        &self,
        session_id: &str,
    ) -> Result<Option<AgentExecution>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, agent_id, parent_execution_id,
                        delegation_type, task, status,
                        started_at, completed_at,
                        tokens_in, tokens_out, checkpoint, error, log_path, child_session_id
                 FROM agent_executions
                 WHERE session_id = ? AND status = 'crashed' AND parent_execution_id IS NOT NULL
                 ORDER BY started_at DESC
                 LIMIT 1",
            )?;

            let execution = stmt
                .query_row(params![session_id], |row| Self::row_to_execution(row))
                .optional()?;

            Ok(execution)
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
            // =====================================================================
            // SESSION COUNTS BY STATUS
            // =====================================================================
            let mut stmt = conn.prepare(
                "SELECT status, COUNT(*) FROM sessions WHERE parent_session_id IS NULL GROUP BY status",
            )?;

            let mut sessions_queued = 0u64;
            let mut sessions_running = 0u64;
            let mut sessions_paused = 0u64;
            let mut sessions_completed = 0u64;
            let mut sessions_crashed = 0u64;

            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;

            for row in rows {
                let (status, count) = row?;
                match status.as_str() {
                    "queued" => sessions_queued = count,
                    "running" => sessions_running = count,
                    "paused" => sessions_paused = count,
                    "completed" => sessions_completed = count,
                    "crashed" => sessions_crashed = count,
                    _ => {}
                }
            }

            // =====================================================================
            // EXECUTION COUNTS BY STATUS
            // Only count executions from sessions that are still running/queued
            // (executions from crashed/completed sessions are not truly "running")
            // =====================================================================
            let mut stmt = conn.prepare(
                "SELECT e.status, COUNT(*)
                 FROM agent_executions e
                 JOIN sessions s ON e.session_id = s.id
                 WHERE s.status IN ('running', 'queued')
                 GROUP BY e.status",
            )?;

            let mut executions_queued = 0u64;
            let mut executions_running = 0u64;
            let mut executions_completed = 0u64;
            let mut executions_crashed = 0u64;
            let mut executions_cancelled = 0u64;

            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;

            for row in rows {
                let (status, count) = row?;
                match status.as_str() {
                    "queued" => executions_queued = count,
                    "running" => executions_running = count,
                    "completed" => executions_completed = count,
                    "crashed" => executions_crashed = count,
                    "cancelled" => executions_cancelled = count,
                    _ => {}
                }
            }

            // =====================================================================
            // TODAY'S STATS
            // =====================================================================
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let mut stmt = conn.prepare(
                "SELECT COUNT(*), COALESCE(SUM(total_tokens_in + total_tokens_out), 0)
                 FROM sessions WHERE parent_session_id IS NULL AND DATE(created_at) = ?",
            )?;

            let (today_sessions, today_tokens) = stmt.query_row(params![today], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64))
            })?;

            // =====================================================================
            // SESSIONS BY SOURCE
            // =====================================================================
            let mut stmt = conn.prepare(
                "SELECT source, COUNT(*) FROM sessions WHERE parent_session_id IS NULL GROUP BY source",
            )?;

            let mut sessions_by_source = std::collections::HashMap::new();
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;

            for row in rows {
                let (source, count) = row?;
                sessions_by_source.insert(source, count);
            }

            Ok(DashboardStats {
                sessions_queued,
                sessions_running,
                sessions_paused,
                sessions_completed,
                sessions_crashed,
                executions_queued,
                executions_running,
                executions_completed,
                executions_crashed,
                executions_cancelled,
                today_sessions,
                today_tokens,
                sessions_by_source,
            })
        })
    }

    // =========================================================================
    // SESSION MESSAGES
    // =========================================================================

    /// Get messages for a session with scope filtering.
    ///
    /// Joins messages with executions to include agent_id and delegation_type.
    ///
    /// Scopes:
    /// - `all`: All messages from all executions
    /// - `root`: Only messages from root executions
    /// - `execution`: Messages from a specific execution
    /// - `delegates`: Only messages from delegated executions
    pub fn get_session_messages(
        &self,
        session_id: &str,
        scope: &str,
        execution_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> Result<Vec<crate::handlers::SessionMessage>, String> {
        self.db.with_connection(|conn| {
            // Base query joining messages with executions
            let mut sql = String::from(
                "SELECT m.id, m.execution_id, e.agent_id, e.delegation_type,
                        m.role, m.content, m.created_at, m.tool_calls, m.tool_results
                 FROM messages m
                 JOIN agent_executions e ON m.execution_id = e.id
                 WHERE e.session_id = ?1",
            );

            // Apply scope filter
            match scope {
                "root" => sql.push_str(" AND e.delegation_type = 'root'"),
                "execution" => sql.push_str(" AND e.id = ?2"),
                "delegates" => sql.push_str(" AND e.delegation_type != 'root'"),
                // "all" - no additional filter
                _ => {}
            }

            // Apply agent_id filter if provided
            if agent_id.is_some() {
                if scope == "execution" {
                    sql.push_str(" AND e.agent_id = ?3");
                } else {
                    sql.push_str(" AND e.agent_id = ?2");
                }
            }

            // Order by execution start time, then message created_at
            sql.push_str(" ORDER BY e.started_at ASC, m.created_at ASC");

            let mut stmt = conn.prepare(&sql)?;

            // Build params based on scope and filters
            let messages: Vec<crate::handlers::SessionMessage> =
                match (scope, execution_id, agent_id) {
                    ("execution", Some(exec_id), Some(agent)) => stmt
                        .query_map(
                            params![session_id, exec_id, agent],
                            Self::row_to_session_message,
                        )?
                        .filter_map(|r| r.ok())
                        .collect(),
                    ("execution", Some(exec_id), None) => stmt
                        .query_map(params![session_id, exec_id], Self::row_to_session_message)?
                        .filter_map(|r| r.ok())
                        .collect(),
                    (_, _, Some(agent)) => stmt
                        .query_map(params![session_id, agent], Self::row_to_session_message)?
                        .filter_map(|r| r.ok())
                        .collect(),
                    _ => stmt
                        .query_map(params![session_id], Self::row_to_session_message)?
                        .filter_map(|r| r.ok())
                        .collect(),
                };

            Ok(messages)
        })
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    fn row_to_session(row: &rusqlite::Row) -> Result<Session, rusqlite::Error> {
        let status_str: String = row.get(1)?;
        let source_str: String = row.get(2)?;
        let metadata_json: Option<String> = row.get(10)?;
        let respond_to_json: Option<String> = row.get(17).ok().flatten();

        Ok(Session {
            id: row.get(0)?,
            status: status_str.parse().unwrap_or(SessionStatus::Running),
            source: source_str.parse().unwrap_or(TriggerSource::Web),
            root_agent_id: row.get(3)?,
            title: row.get(4)?,
            created_at: row.get(5)?,
            started_at: row.get(6)?,
            completed_at: row.get(7)?,
            total_tokens_in: row.get::<_, i64>(8)? as u64,
            total_tokens_out: row.get::<_, i64>(9)? as u64,
            metadata: metadata_json.and_then(|s| serde_json::from_str(&s).ok()),
            pending_delegations: row.get::<_, i64>(11).unwrap_or(0) as u32,
            continuation_needed: row.get::<_, i64>(12).unwrap_or(0) != 0,
            ward_id: row.get(13).ok().flatten(),
            parent_session_id: row.get(14).ok().flatten(),
            thread_id: row.get(15).ok().flatten(),
            connector_id: row.get(16).ok().flatten(),
            respond_to: respond_to_json.and_then(|s| serde_json::from_str(&s).ok()),
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
            child_session_id: row.get(14)?,
        })
    }

    // =========================================================================
    // ARTIFACTS
    // =========================================================================

    /// Insert a new artifact record.
    pub fn create_artifact(&self, artifact: &Artifact) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO artifacts (
                    id, session_id, ward_id, execution_id, agent_id,
                    file_path, file_name, file_type, file_size, label, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    artifact.id,
                    artifact.session_id,
                    artifact.ward_id,
                    artifact.execution_id,
                    artifact.agent_id,
                    artifact.file_path,
                    artifact.file_name,
                    artifact.file_type,
                    artifact.file_size,
                    artifact.label,
                    artifact.created_at,
                ],
            )?;
            Ok(())
        })
    }

    /// List all artifacts for a session, ordered by creation time.
    pub fn list_artifacts_by_session(&self, session_id: &str) -> Result<Vec<Artifact>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, ward_id, execution_id, agent_id,
                        file_path, file_name, file_type, file_size, label, created_at
                 FROM artifacts
                 WHERE session_id = ?1
                 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(params![session_id], |row| {
                    Ok(Artifact {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        ward_id: row.get(2)?,
                        execution_id: row.get(3)?,
                        agent_id: row.get(4)?,
                        file_path: row.get(5)?,
                        file_name: row.get(6)?,
                        file_type: row.get(7)?,
                        file_size: row.get(8)?,
                        label: row.get(9)?,
                        created_at: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Get a single artifact by ID.
    pub fn get_artifact(&self, artifact_id: &str) -> Result<Option<Artifact>, String> {
        self.db.with_connection(|conn| {
            let result = conn
                .query_row(
                    "SELECT id, session_id, ward_id, execution_id, agent_id,
                            file_path, file_name, file_type, file_size, label, created_at
                     FROM artifacts
                     WHERE id = ?1",
                    params![artifact_id],
                    |row| {
                        Ok(Artifact {
                            id: row.get(0)?,
                            session_id: row.get(1)?,
                            ward_id: row.get(2)?,
                            execution_id: row.get(3)?,
                            agent_id: row.get(4)?,
                            file_path: row.get(5)?,
                            file_name: row.get(6)?,
                            file_type: row.get(7)?,
                            file_size: row.get(8)?,
                            label: row.get(9)?,
                            created_at: row.get(10)?,
                        })
                    },
                )
                .optional()?;
            Ok(result)
        })
    }

    fn row_to_session_message(
        row: &rusqlite::Row,
    ) -> Result<crate::handlers::SessionMessage, rusqlite::Error> {
        let tool_calls_json: Option<String> = row.get(7)?;
        let tool_results_json: Option<String> = row.get(8)?;

        Ok(crate::handlers::SessionMessage {
            id: row.get(0)?,
            execution_id: row.get(1)?,
            agent_id: row.get(2)?,
            delegation_type: row.get(3)?,
            role: row.get(4)?,
            content: row.get(5)?,
            created_at: row.get(6)?,
            tool_calls: tool_calls_json.and_then(|s| serde_json::from_str(&s).ok()),
            tool_results: tool_results_json.and_then(|s| serde_json::from_str(&s).ok()),
        })
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::Mutex;

    /// Test database provider using in-memory SQLite.
    struct TestDbProvider {
        conn: Mutex<Connection>,
    }

    impl TestDbProvider {
        fn new() -> Self {
            let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

            // Create tables matching the actual schema
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    status TEXT NOT NULL DEFAULT 'queued',
                    source TEXT NOT NULL DEFAULT 'web',
                    root_agent_id TEXT NOT NULL,
                    title TEXT,
                    created_at TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT,
                    total_tokens_in INTEGER NOT NULL DEFAULT 0,
                    total_tokens_out INTEGER NOT NULL DEFAULT 0,
                    metadata TEXT,
                    pending_delegations INTEGER DEFAULT 0,
                    continuation_needed INTEGER DEFAULT 0,
                    ward_id TEXT,
                    parent_session_id TEXT,
                    thread_id TEXT,
                    connector_id TEXT,
                    respond_to TEXT
                );

                CREATE TABLE IF NOT EXISTS agent_executions (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    agent_id TEXT NOT NULL,
                    parent_execution_id TEXT,
                    delegation_type TEXT NOT NULL DEFAULT 'root',
                    task TEXT,
                    status TEXT NOT NULL DEFAULT 'queued',
                    started_at TEXT,
                    completed_at TEXT,
                    tokens_in INTEGER NOT NULL DEFAULT 0,
                    tokens_out INTEGER NOT NULL DEFAULT 0,
                    checkpoint TEXT,
                    error TEXT,
                    log_path TEXT,
                    child_session_id TEXT,
                    FOREIGN KEY (session_id) REFERENCES sessions(id)
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    execution_id TEXT,
                    session_id TEXT,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    token_count INTEGER DEFAULT 0,
                    tool_calls TEXT,
                    tool_results TEXT,
                    tool_call_id TEXT,
                    FOREIGN KEY (execution_id) REFERENCES agent_executions(id),
                    FOREIGN KEY (session_id) REFERENCES sessions(id)
                );

                CREATE INDEX IF NOT EXISTS idx_agent_executions_session ON agent_executions(session_id);
                CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
                CREATE INDEX IF NOT EXISTS idx_messages_execution ON messages(execution_id);
                CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
                "#,
            )
            .expect("Failed to create tables");

            Self {
                conn: Mutex::new(conn),
            }
        }
    }

    impl StateDbProvider for TestDbProvider {
        fn with_connection<F, R>(&self, f: F) -> Result<R, String>
        where
            F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
        {
            let conn = self.conn.lock().map_err(|e| e.to_string())?;
            f(&conn).map_err(|e| e.to_string())
        }
    }

    fn setup_repo() -> StateRepository<TestDbProvider> {
        let db = Arc::new(TestDbProvider::new());
        StateRepository::new(db)
    }

    // ========================================================================
    // Session Tests
    // ========================================================================

    #[test]
    fn create_session_success() {
        let repo = setup_repo();
        let session = Session::new("root-agent");

        let result = repo.create_session(&session);
        assert!(
            result.is_ok(),
            "Failed to create session: {:?}",
            result.err()
        );
    }

    #[test]
    fn get_session_success() {
        let repo = setup_repo();
        let session = Session::new_with_source("test-agent", TriggerSource::Cli);

        repo.create_session(&session).unwrap();

        let retrieved = repo.get_session(&session.id).unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, session.id);
        assert_eq!(retrieved.status, SessionStatus::Running);
        assert_eq!(retrieved.source, TriggerSource::Cli);
        assert_eq!(retrieved.root_agent_id, "test-agent");
    }

    #[test]
    fn get_session_not_found() {
        let repo = setup_repo();

        let result = repo.get_session("nonexistent-session");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn list_sessions_empty() {
        let repo = setup_repo();
        let filter = SessionFilter::default();

        let sessions = repo.list_sessions(&filter).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn list_sessions_with_data() {
        let repo = setup_repo();

        let s1 = Session::new("agent1");
        let s2 = Session::new("agent2");
        let s3 = Session::new_with_source("agent3", TriggerSource::Cron);

        repo.create_session(&s1).unwrap();
        repo.create_session(&s2).unwrap();
        repo.create_session(&s3).unwrap();

        let filter = SessionFilter::default();
        let sessions = repo.list_sessions(&filter).unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn list_sessions_with_limit() {
        let repo = setup_repo();

        for i in 0..5 {
            let session = Session::new(format!("agent{}", i));
            repo.create_session(&session).unwrap();
        }

        let filter = SessionFilter {
            limit: Some(2),
            ..Default::default()
        };
        let sessions = repo.list_sessions(&filter).unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn update_session_status() {
        let repo = setup_repo();
        let session = Session::new("agent");

        repo.create_session(&session).unwrap();
        repo.update_session_status(&session.id, SessionStatus::Completed)
            .unwrap();

        let updated = repo.get_session(&session.id).unwrap().unwrap();
        assert_eq!(updated.status, SessionStatus::Completed);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn update_session_tokens() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        // Create an execution with tokens
        let mut exec = AgentExecution::new_root(&session.id, "agent");
        exec.tokens_in = 100;
        exec.tokens_out = 50;
        repo.create_execution(&exec).unwrap();

        // Update session tokens from execution totals
        repo.update_session_tokens(&session.id).unwrap();

        let updated = repo.get_session(&session.id).unwrap().unwrap();
        assert_eq!(updated.total_tokens_in, 100);
        assert_eq!(updated.total_tokens_out, 50);
    }

    #[test]
    fn delete_session() {
        let repo = setup_repo();
        let session = Session::new("agent");

        repo.create_session(&session).unwrap();
        assert!(repo.get_session(&session.id).unwrap().is_some());

        repo.delete_session(&session.id).unwrap();
        assert!(repo.get_session(&session.id).unwrap().is_none());
    }

    // ========================================================================
    // Execution Tests
    // ========================================================================

    #[test]
    fn create_execution_success() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec = AgentExecution::new_root(&session.id, "root-agent");
        let result = repo.create_execution(&exec);

        assert!(
            result.is_ok(),
            "Failed to create execution: {:?}",
            result.err()
        );
    }

    #[test]
    fn get_execution_success() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec = AgentExecution::new_root(&session.id, "root-agent");
        repo.create_execution(&exec).unwrap();

        let retrieved = repo.get_execution(&exec.id).unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, exec.id);
        assert_eq!(retrieved.session_id, session.id);
        assert_eq!(retrieved.agent_id, "root-agent");
        assert_eq!(retrieved.delegation_type, DelegationType::Root);
    }

    #[test]
    fn get_execution_not_found() {
        let repo = setup_repo();

        let result = repo.get_execution("nonexistent");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn create_delegated_execution() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let root_exec = AgentExecution::new_root(&session.id, "root");
        repo.create_execution(&root_exec).unwrap();

        let sub_exec = AgentExecution::new_delegated(
            &session.id,
            "researcher",
            &root_exec.id,
            DelegationType::Sequential,
            "Research AI topics",
        );
        repo.create_execution(&sub_exec).unwrap();

        let retrieved = repo.get_execution(&sub_exec.id).unwrap().unwrap();
        assert_eq!(retrieved.parent_execution_id, Some(root_exec.id.clone()));
        assert_eq!(retrieved.delegation_type, DelegationType::Sequential);
        assert_eq!(retrieved.task, Some("Research AI topics".to_string()));
    }

    #[test]
    fn list_executions_by_session() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec1 = AgentExecution::new_root(&session.id, "agent1");
        let exec2 = AgentExecution::new_delegated(
            &session.id,
            "agent2",
            &exec1.id,
            DelegationType::Sequential,
            "task",
        );

        repo.create_execution(&exec1).unwrap();
        repo.create_execution(&exec2).unwrap();

        let filter = ExecutionFilter {
            session_id: Some(session.id.clone()),
            ..Default::default()
        };

        let executions = repo.list_executions(&filter).unwrap();
        assert_eq!(executions.len(), 2);
    }

    #[test]
    fn get_child_executions() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let root = AgentExecution::new_root(&session.id, "root");
        repo.create_execution(&root).unwrap();

        let child1 = AgentExecution::new_delegated(
            &session.id,
            "child1",
            &root.id,
            DelegationType::Sequential,
            "task1",
        );
        let child2 = AgentExecution::new_delegated(
            &session.id,
            "child2",
            &root.id,
            DelegationType::Parallel,
            "task2",
        );

        repo.create_execution(&child1).unwrap();
        repo.create_execution(&child2).unwrap();

        let children = repo.get_child_executions(&root.id).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn update_execution_status() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec = AgentExecution::new_root(&session.id, "agent");
        repo.create_execution(&exec).unwrap();

        repo.update_execution_status(&exec.id, ExecutionStatus::Running)
            .unwrap();
        let updated = repo.get_execution(&exec.id).unwrap().unwrap();
        assert_eq!(updated.status, ExecutionStatus::Running);
        assert!(updated.started_at.is_some());

        repo.update_execution_status(&exec.id, ExecutionStatus::Completed)
            .unwrap();
        let updated = repo.get_execution(&exec.id).unwrap().unwrap();
        assert_eq!(updated.status, ExecutionStatus::Completed);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn update_execution_tokens() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec = AgentExecution::new_root(&session.id, "agent");
        repo.create_execution(&exec).unwrap();

        repo.update_execution_tokens(&exec.id, 200, 100).unwrap();

        let updated = repo.get_execution(&exec.id).unwrap().unwrap();
        assert_eq!(updated.tokens_in, 200);
        assert_eq!(updated.tokens_out, 100);
    }

    #[test]
    fn set_execution_error() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec = AgentExecution::new_root(&session.id, "agent");
        repo.create_execution(&exec).unwrap();

        repo.set_execution_error(&exec.id, "Something went wrong")
            .unwrap();

        let updated = repo.get_execution(&exec.id).unwrap().unwrap();
        assert_eq!(updated.error, Some("Something went wrong".to_string()));
        // Note: set_execution_error only sets the error message, not the status
        // Status should be updated separately via update_execution_status
    }

    // ========================================================================
    // Session with Executions Tests
    // ========================================================================

    #[test]
    fn get_session_with_executions() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let root = AgentExecution::new_root(&session.id, "root");
        let sub = AgentExecution::new_delegated(
            &session.id,
            "sub",
            &root.id,
            DelegationType::Sequential,
            "task",
        );

        repo.create_execution(&root).unwrap();
        repo.create_execution(&sub).unwrap();

        let result = repo.get_session_with_executions(&session.id).unwrap();
        assert!(result.is_some());

        let swe = result.unwrap();
        assert_eq!(swe.session.id, session.id);
        assert_eq!(swe.executions.len(), 2);
        assert_eq!(swe.subagent_count, 1); // One subagent
    }

    #[test]
    fn list_sessions_with_executions() {
        let repo = setup_repo();

        let s1 = Session::new("agent1");
        let s2 = Session::new("agent2");

        repo.create_session(&s1).unwrap();
        repo.create_session(&s2).unwrap();

        let e1 = AgentExecution::new_root(&s1.id, "agent1");
        let e2 = AgentExecution::new_root(&s2.id, "agent2");

        repo.create_execution(&e1).unwrap();
        repo.create_execution(&e2).unwrap();

        let filter = SessionFilter::default();
        let sessions = repo.list_sessions_with_executions(&filter).unwrap();
        assert_eq!(sessions.len(), 2);

        for swe in sessions {
            assert!(!swe.executions.is_empty());
        }
    }

    // ========================================================================
    // Dashboard Stats Tests
    // ========================================================================

    #[test]
    fn get_dashboard_stats_empty() {
        let repo = setup_repo();

        let stats = repo.get_dashboard_stats().unwrap();

        assert_eq!(stats.sessions_running, 0);
        assert_eq!(stats.sessions_queued, 0);
        assert_eq!(stats.sessions_completed, 0);
        assert_eq!(stats.executions_running, 0);
    }

    #[test]
    fn get_dashboard_stats_with_data() {
        let repo = setup_repo();

        // Create sessions with different statuses and sources
        let s1 = Session::new_with_source("agent1", TriggerSource::Web);
        let s2 = Session::new_with_source("agent2", TriggerSource::Web);
        let s3 = Session::new_with_source("agent3", TriggerSource::Cli);
        let s4 = Session::new_queued("agent4", TriggerSource::Cron);

        repo.create_session(&s1).unwrap();
        repo.create_session(&s2).unwrap();
        repo.create_session(&s3).unwrap();
        repo.create_session(&s4).unwrap();

        // Complete one session
        repo.update_session_status(&s2.id, SessionStatus::Completed)
            .unwrap();

        // Create executions
        let e1 = AgentExecution::new_root(&s1.id, "agent1");
        let e3 = AgentExecution::new_root(&s3.id, "agent3");

        repo.create_execution(&e1).unwrap();
        repo.create_execution(&e3).unwrap();

        // Start one execution
        repo.update_execution_status(&e1.id, ExecutionStatus::Running)
            .unwrap();

        let stats = repo.get_dashboard_stats().unwrap();

        assert_eq!(stats.sessions_running, 2); // s1, s3 (s2 completed, s4 queued)
        assert_eq!(stats.sessions_queued, 1); // s4
        assert_eq!(stats.sessions_completed, 1); // s2
        assert_eq!(stats.executions_running, 1); // e1
        assert_eq!(stats.executions_queued, 1); // e3 (not started yet)

        // Check by source
        assert_eq!(*stats.sessions_by_source.get("web").unwrap_or(&0), 2);
        assert_eq!(*stats.sessions_by_source.get("cli").unwrap_or(&0), 1);
        assert_eq!(*stats.sessions_by_source.get("cron").unwrap_or(&0), 1);
    }

    // ========================================================================
    // Checkpoint Tests
    // ========================================================================

    #[test]
    fn save_execution_checkpoint() {
        let repo = setup_repo();
        let session = Session::new("agent");
        repo.create_session(&session).unwrap();

        let exec = AgentExecution::new_root(&session.id, "agent");
        repo.create_execution(&exec).unwrap();

        let checkpoint = Checkpoint::new(5, "msg-last");
        repo.save_execution_checkpoint(&exec.id, &checkpoint)
            .unwrap();

        let updated = repo.get_execution(&exec.id).unwrap().unwrap();
        assert!(updated.checkpoint.is_some());

        let saved_checkpoint = updated.checkpoint.unwrap();
        assert_eq!(saved_checkpoint.llm_turn, 5);
        assert_eq!(saved_checkpoint.last_message_id, "msg-last");
    }

    // ========================================================================
    // Session Messages Tests
    // ========================================================================

    /// Helper to add a message to an execution
    fn add_message(
        repo: &StateRepository<TestDbProvider>,
        execution_id: &str,
        role: &str,
        content: &str,
    ) {
        repo.db.with_connection(|conn| {
            let id = format!("msg-{}", uuid::Uuid::new_v4());
            let created_at = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO messages (id, execution_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, execution_id, role, content, created_at],
            )?;
            Ok(())
        }).unwrap();
    }

    #[test]
    fn get_session_messages_all_scope() {
        let repo = setup_repo();
        let session = Session::new("root-agent");
        repo.create_session(&session).unwrap();

        // Create root execution
        let mut root_exec = AgentExecution::new_root(&session.id, "claude");
        root_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&root_exec).unwrap();

        // Create subagent execution
        let mut sub_exec = AgentExecution::new_delegated(
            &session.id,
            "research-agent",
            &root_exec.id,
            DelegationType::Sequential,
            "Research AI",
        );
        sub_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub_exec).unwrap();

        // Add messages to both
        add_message(&repo, &root_exec.id, "user", "Hello root");
        add_message(&repo, &root_exec.id, "assistant", "Root response");
        add_message(&repo, &sub_exec.id, "user", "Research task");
        add_message(&repo, &sub_exec.id, "assistant", "Research result");

        // Get all messages
        let messages = repo
            .get_session_messages(&session.id, "all", None, None)
            .unwrap();
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn get_session_messages_root_scope() {
        let repo = setup_repo();
        let session = Session::new("root-agent");
        repo.create_session(&session).unwrap();

        // Create root execution
        let mut root_exec = AgentExecution::new_root(&session.id, "claude");
        root_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&root_exec).unwrap();

        // Create subagent execution
        let mut sub_exec = AgentExecution::new_delegated(
            &session.id,
            "research-agent",
            &root_exec.id,
            DelegationType::Sequential,
            "Research AI",
        );
        sub_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub_exec).unwrap();

        // Add messages to both
        add_message(&repo, &root_exec.id, "user", "Hello root");
        add_message(&repo, &root_exec.id, "assistant", "Root response");
        add_message(&repo, &sub_exec.id, "user", "Research task");
        add_message(&repo, &sub_exec.id, "assistant", "Research result");

        // Get only root messages
        let messages = repo
            .get_session_messages(&session.id, "root", None, None)
            .unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.delegation_type == "root"));
    }

    #[test]
    fn get_session_messages_execution_scope() {
        let repo = setup_repo();
        let session = Session::new("root-agent");
        repo.create_session(&session).unwrap();

        // Create root execution
        let mut root_exec = AgentExecution::new_root(&session.id, "claude");
        root_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&root_exec).unwrap();

        // Create subagent execution
        let mut sub_exec = AgentExecution::new_delegated(
            &session.id,
            "research-agent",
            &root_exec.id,
            DelegationType::Sequential,
            "Research AI",
        );
        sub_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub_exec).unwrap();

        // Add messages
        add_message(&repo, &root_exec.id, "user", "Hello root");
        add_message(&repo, &sub_exec.id, "user", "Research task");
        add_message(&repo, &sub_exec.id, "assistant", "Research result");

        // Get only specific execution messages
        let messages = repo
            .get_session_messages(&session.id, "execution", Some(&sub_exec.id), None)
            .unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.execution_id == sub_exec.id));
    }

    #[test]
    fn get_session_messages_delegates_scope() {
        let repo = setup_repo();
        let session = Session::new("root-agent");
        repo.create_session(&session).unwrap();

        // Create root execution
        let mut root_exec = AgentExecution::new_root(&session.id, "claude");
        root_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&root_exec).unwrap();

        // Create two subagent executions
        let mut sub1 = AgentExecution::new_delegated(
            &session.id,
            "research-agent",
            &root_exec.id,
            DelegationType::Sequential,
            "Research",
        );
        sub1.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub1).unwrap();

        let mut sub2 = AgentExecution::new_delegated(
            &session.id,
            "writer-agent",
            &root_exec.id,
            DelegationType::Parallel,
            "Write",
        );
        sub2.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub2).unwrap();

        // Add messages
        add_message(&repo, &root_exec.id, "user", "Root message");
        add_message(&repo, &sub1.id, "assistant", "Research result");
        add_message(&repo, &sub2.id, "assistant", "Writing result");

        // Get only delegate messages
        let messages = repo
            .get_session_messages(&session.id, "delegates", None, None)
            .unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.delegation_type != "root"));
    }

    #[test]
    fn get_session_messages_agent_filter() {
        let repo = setup_repo();
        let session = Session::new("root-agent");
        repo.create_session(&session).unwrap();

        // Create root execution
        let mut root_exec = AgentExecution::new_root(&session.id, "claude");
        root_exec.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&root_exec).unwrap();

        // Create two different agent executions
        let mut sub1 = AgentExecution::new_delegated(
            &session.id,
            "research-agent",
            &root_exec.id,
            DelegationType::Sequential,
            "Research 1",
        );
        sub1.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub1).unwrap();

        let mut sub2 = AgentExecution::new_delegated(
            &session.id,
            "writer-agent",
            &root_exec.id,
            DelegationType::Sequential,
            "Write",
        );
        sub2.started_at = Some(chrono::Utc::now().to_rfc3339());
        repo.create_execution(&sub2).unwrap();

        // Add messages
        add_message(&repo, &root_exec.id, "assistant", "Claude response");
        add_message(&repo, &sub1.id, "assistant", "Research result");
        add_message(&repo, &sub2.id, "assistant", "Writing result");

        // Filter by agent_id
        let messages = repo
            .get_session_messages(&session.id, "all", None, Some("research-agent"))
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].agent_id, "research-agent");
    }
}
