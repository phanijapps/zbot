//! # State Service
//!
//! Business logic for session and execution state management.

use crate::handlers::Message;
use crate::repository::{StateDbProvider, StateRepository};
use crate::types::*;
use rusqlite::params;
use std::sync::Arc;

// ============================================================================
// SERVICE
// ============================================================================

/// Service for session state management.
///
/// Provides session lifecycle management, execution tracking, and stats.
pub struct StateService<D: StateDbProvider> {
    repo: StateRepository<D>,
    db: Arc<D>,
}

impl<D: StateDbProvider> StateService<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self {
            repo: StateRepository::new(db.clone()),
            db,
        }
    }

    // =========================================================================
    // SESSION LIFECYCLE
    // =========================================================================

    /// Create a new session with a root execution.
    pub fn create_session(&self, agent_id: &str) -> Result<(Session, AgentExecution), String> {
        let session = Session::new(agent_id);
        let execution = AgentExecution::new_root(&session.id, agent_id);

        self.repo.create_session(&session)?;
        self.repo.create_execution(&execution)?;

        Ok((session, execution))
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        self.repo.get_session(session_id)
    }

    /// Get session with all executions.
    pub fn get_session_with_executions(&self, session_id: &str) -> Result<Option<SessionWithExecutions>, String> {
        self.repo.get_session_with_executions(session_id)
    }

    /// List sessions with filtering.
    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<Session>, String> {
        self.repo.list_sessions(filter)
    }

    /// List sessions with executions.
    pub fn list_sessions_with_executions(&self, filter: &SessionFilter) -> Result<Vec<SessionWithExecutions>, String> {
        self.repo.list_sessions_with_executions(filter)
    }

    /// Pause a session.
    pub fn pause_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.repo.get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        if session.status != SessionStatus::Running {
            return Err(format!("Cannot pause session in {} state", session.status.as_str()));
        }

        self.repo.update_session_status(session_id, SessionStatus::Paused)
    }

    /// Resume a session.
    pub fn resume_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.repo.get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        if session.status != SessionStatus::Paused {
            return Err(format!("Cannot resume session in {} state", session.status.as_str()));
        }

        self.repo.update_session_status(session_id, SessionStatus::Running)
    }

    /// Cancel a session (by marking root execution cancelled).
    pub fn cancel_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.repo.get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        if session.status.is_terminal() {
            return Err(format!("Cannot cancel session in {} state", session.status.as_str()));
        }

        // Cancel all running executions
        let executions = self.repo.list_executions(&ExecutionFilter {
            session_id: Some(session_id.to_string()),
            ..Default::default()
        })?;

        for exec in executions {
            if !exec.status.is_terminal() {
                self.repo.update_execution_status(&exec.id, ExecutionStatus::Cancelled)?;
            }
        }

        self.repo.update_session_status(session_id, SessionStatus::Crashed)
    }

    /// Complete a session.
    pub fn complete_session(&self, session_id: &str) -> Result<(), String> {
        self.repo.update_session_tokens(session_id)?;
        self.repo.update_session_status(session_id, SessionStatus::Completed)
    }

    /// Mark a session as crashed.
    pub fn crash_session(&self, session_id: &str) -> Result<(), String> {
        self.repo.update_session_tokens(session_id)?;
        self.repo.update_session_status(session_id, SessionStatus::Crashed)
    }

    /// Delete a session.
    pub fn delete_session(&self, session_id: &str) -> Result<bool, String> {
        self.repo.delete_session(session_id)
    }

    /// Delete sessions older than a given timestamp.
    pub fn delete_old_sessions(&self, older_than: &str) -> Result<u64, String> {
        self.repo.delete_old_sessions(older_than)
    }

    // =========================================================================
    // EXECUTION LIFECYCLE
    // =========================================================================

    /// Create an execution directly (for adding to existing session).
    pub fn create_execution(&self, execution: &AgentExecution) -> Result<(), String> {
        self.repo.create_execution(execution)
    }

    /// Create a delegated execution.
    pub fn create_delegated_execution(
        &self,
        session_id: &str,
        agent_id: &str,
        parent_execution_id: &str,
        delegation_type: DelegationType,
        task: &str,
    ) -> Result<AgentExecution, String> {
        let execution = AgentExecution::new_delegated(
            session_id,
            agent_id,
            parent_execution_id,
            delegation_type,
            task,
        );
        self.repo.create_execution(&execution)?;
        Ok(execution)
    }

    /// Get an execution by ID.
    pub fn get_execution(&self, execution_id: &str) -> Result<Option<AgentExecution>, String> {
        self.repo.get_execution(execution_id)
    }

    /// Get root execution for a session.
    pub fn get_root_execution(&self, session_id: &str) -> Result<Option<AgentExecution>, String> {
        self.repo.get_root_execution(session_id)
    }

    /// List executions with filtering.
    pub fn list_executions(&self, filter: &ExecutionFilter) -> Result<Vec<AgentExecution>, String> {
        self.repo.list_executions(filter)
    }

    /// Get child executions.
    pub fn get_child_executions(&self, parent_execution_id: &str) -> Result<Vec<AgentExecution>, String> {
        self.repo.get_child_executions(parent_execution_id)
    }

    /// Start an execution.
    pub fn start_execution(&self, execution_id: &str) -> Result<(), String> {
        self.repo.update_execution_status(execution_id, ExecutionStatus::Running)
    }

    /// Complete an execution.
    pub fn complete_execution(&self, execution_id: &str) -> Result<(), String> {
        self.repo.update_execution_status(execution_id, ExecutionStatus::Completed)
    }

    /// Mark an execution as crashed.
    pub fn crash_execution(&self, execution_id: &str, error: &str) -> Result<(), String> {
        self.repo.set_execution_error(execution_id, error)?;
        self.repo.update_execution_status(execution_id, ExecutionStatus::Crashed)
    }

    /// Update execution tokens.
    pub fn update_execution_tokens(&self, execution_id: &str, tokens_in: u64, tokens_out: u64) -> Result<(), String> {
        self.repo.update_execution_tokens(execution_id, tokens_in, tokens_out)
    }

    /// Save execution checkpoint.
    pub fn save_execution_checkpoint(&self, execution_id: &str, checkpoint: &Checkpoint) -> Result<(), String> {
        self.repo.save_execution_checkpoint(execution_id, checkpoint)
    }

    // =========================================================================
    // CRASH RECOVERY
    // =========================================================================

    /// Mark all running sessions as crashed (on startup).
    pub fn mark_running_as_crashed(&self) -> Result<u64, String> {
        let sessions = self.repo.list_sessions(&SessionFilter {
            status: Some(SessionStatus::Running),
            ..Default::default()
        })?;

        let count = sessions.len() as u64;

        for session in sessions {
            self.repo.update_session_status(&session.id, SessionStatus::Crashed)?;
        }

        Ok(count)
    }

    // =========================================================================
    // STATS
    // =========================================================================

    /// Get dashboard stats (pre-computed).
    pub fn get_dashboard_stats(&self) -> Result<DashboardStats, String> {
        self.repo.get_dashboard_stats()
    }

    // =========================================================================
    // MESSAGES
    // =========================================================================

    /// Get messages for an execution.
    pub fn get_messages(&self, execution_id: &str) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, execution_id, role, content, created_at, tool_calls, tool_results
                 FROM messages WHERE execution_id = ? ORDER BY created_at ASC",
            )?;

            let messages = stmt
                .query_map(params![execution_id], |row| {
                    Ok(Message {
                        id: row.get(0)?,
                        execution_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        created_at: row.get(4)?,
                        tool_calls: row.get::<_, Option<String>>(5)?
                            .and_then(|s| serde_json::from_str(&s).ok()),
                        tool_results: row.get::<_, Option<String>>(6)?
                            .and_then(|s| serde_json::from_str(&s).ok()),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(messages)
        })
    }

    /// Add a message to an execution.
    pub fn add_message(
        &self,
        execution_id: &str,
        role: &str,
        content: &str,
        tool_calls: Option<&serde_json::Value>,
        tool_results: Option<&serde_json::Value>,
    ) -> Result<String, String> {
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        let created_at = chrono::Utc::now().to_rfc3339();

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO messages (id, execution_id, role, content, created_at, tool_calls, tool_results)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    execution_id,
                    role,
                    content,
                    created_at,
                    tool_calls.map(|v| serde_json::to_string(v).ok()).flatten(),
                    tool_results.map(|v| serde_json::to_string(v).ok()).flatten(),
                ],
            )?;
            Ok(id.clone())
        })
    }
}
