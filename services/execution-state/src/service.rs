//! # State Service
//!
//! Business logic for session and execution state management.

use crate::handlers::{Message, SessionMessage, SessionMessagesQuery};
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

    /// Create a new session with source and a root execution.
    pub fn create_session_with_source(&self, agent_id: &str, source: TriggerSource) -> Result<(Session, AgentExecution), String> {
        let session = Session::new_with_source(agent_id, source);
        let execution = AgentExecution::new_root(&session.id, agent_id);

        self.repo.create_session(&session)?;
        self.repo.create_execution(&execution)?;

        Ok((session, execution))
    }

    /// Create a new session in QUEUED state (does not create root execution yet).
    pub fn create_session_queued(&self, agent_id: &str, source: TriggerSource) -> Result<Session, String> {
        let session = Session::new_queued(agent_id, source);
        self.repo.create_session(&session)?;
        Ok(session)
    }

    /// Start a queued session (transition Queued → Running and create root execution).
    pub fn start_session(&self, session_id: &str) -> Result<(Session, AgentExecution), String> {
        let session = self.repo.get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        if session.status != SessionStatus::Queued {
            return Err(format!("Cannot start session in {} state (must be queued)", session.status.as_str()));
        }

        // Transition to Running
        self.repo.update_session_status(session_id, SessionStatus::Running)?;

        // Create root execution
        let execution = AgentExecution::new_root(session_id, &session.root_agent_id);
        self.repo.create_execution(&execution)?;

        // Fetch updated session
        let updated_session = self.repo.get_session(session_id)?
            .ok_or_else(|| "Session disappeared after update".to_string())?;

        Ok((updated_session, execution))
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
    ///
    /// Also marks all running/queued executions as crashed to maintain data consistency.
    pub fn crash_session(&self, session_id: &str) -> Result<(), String> {
        // First, mark all running/queued executions as crashed
        let executions = self.repo.list_executions(&ExecutionFilter {
            session_id: Some(session_id.to_string()),
            ..Default::default()
        })?;

        for exec in executions {
            if matches!(exec.status, ExecutionStatus::Running | ExecutionStatus::Queued) {
                if let Err(e) = self.repo.update_execution_status(&exec.id, ExecutionStatus::Crashed) {
                    tracing::warn!("Failed to crash execution {}: {}", exec.id, e);
                }
            }
        }

        self.repo.update_session_tokens(session_id)?;
        self.repo.update_session_status(session_id, SessionStatus::Crashed)
    }

    /// Reactivate a terminal session (completed/crashed) back to running.
    ///
    /// Called when a new execution is added to an existing session that was
    /// previously completed or crashed.
    pub fn reactivate_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.repo.get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        if session.status.is_terminal() {
            tracing::info!(
                session_id = %session_id,
                old_status = %session.status.as_str(),
                "Reactivating terminal session for new execution"
            );
            self.repo.update_session_status(session_id, SessionStatus::Running)?;
        }

        Ok(())
    }

    /// Update the active ward for a session.
    pub fn update_session_ward(&self, session_id: &str, ward_id: &str) -> Result<(), String> {
        self.repo.update_session_ward(session_id, ward_id)
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

    /// Check if a session has any pending executions (running or queued).
    ///
    /// This checks for both RUNNING and QUEUED executions. QUEUED executions
    /// are important to include because delegated subagent executions are
    /// created in QUEUED status synchronously when delegation is requested,
    /// before the actual spawn happens asynchronously.
    pub fn has_running_executions(&self, session_id: &str) -> Result<bool, String> {
        // Check for RUNNING executions
        let running = self.repo.list_executions(&ExecutionFilter {
            session_id: Some(session_id.to_string()),
            status: Some(ExecutionStatus::Running),
            ..Default::default()
        })?;

        if !running.is_empty() {
            return Ok(true);
        }

        // Check for QUEUED executions (pending subagents)
        let queued = self.repo.list_executions(&ExecutionFilter {
            session_id: Some(session_id.to_string()),
            status: Some(ExecutionStatus::Queued),
            ..Default::default()
        })?;

        Ok(!queued.is_empty())
    }

    /// Try to complete a session, but only if all executions are done.
    ///
    /// Returns true if session was completed, false if:
    /// - There are still running executions
    /// - The session source doesn't auto-complete (e.g., Web sessions stay open)
    ///
    /// Web sessions stay open for interactive use until user explicitly ends them.
    /// CLI, Cron, API sessions auto-complete when all executions finish.
    pub fn try_complete_session(&self, session_id: &str) -> Result<bool, String> {
        // Check if there are running executions
        if self.has_running_executions(session_id)? {
            return Ok(false);
        }

        // Get session to check if it should auto-complete
        let session = self.repo.get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        // Web sessions stay open for interactive use
        if !session.source.should_auto_complete_session() {
            tracing::debug!(
                session_id = %session_id,
                source = %session.source,
                "Session staying open (source doesn't auto-complete)"
            );
            return Ok(false);
        }

        self.complete_session(session_id)?;
        Ok(true)
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

    /// Eagerly aggregate session token totals from all executions.
    ///
    /// This runs a SUM query over all executions in the session and updates
    /// the session's total_tokens_in/out. Unlike `complete_session()`, this
    /// can be called at any time without changing session status — useful for
    /// web sessions that never auto-complete but still need token visibility.
    pub fn aggregate_session_tokens(&self, session_id: &str) -> Result<(), String> {
        self.repo.update_session_tokens(session_id)
    }

    /// Save execution checkpoint.
    pub fn save_execution_checkpoint(&self, execution_id: &str, checkpoint: &Checkpoint) -> Result<(), String> {
        self.repo.save_execution_checkpoint(execution_id, checkpoint)
    }

    // =========================================================================
    // DELEGATION TRACKING
    // =========================================================================

    /// Called when spawning a delegated execution.
    ///
    /// Increments the pending delegation count for the session.
    pub fn register_delegation(&self, session_id: &str) -> Result<(), String> {
        self.repo.increment_pending_delegations(session_id)
    }

    /// Called when a delegated execution completes.
    ///
    /// Returns true if this was the last pending delegation AND continuation is needed.
    pub fn complete_delegation(&self, session_id: &str) -> Result<bool, String> {
        let remaining = self.repo.decrement_pending_delegations(session_id)?;
        if remaining == 0 {
            let session = self.repo.get_session(session_id)?.ok_or("Session not found")?;
            Ok(session.continuation_needed)
        } else {
            Ok(false)
        }
    }

    /// Mark that session needs continuation after delegations complete.
    pub fn request_continuation(&self, session_id: &str) -> Result<(), String> {
        self.repo.set_continuation_needed(session_id, true)
    }

    /// Clear continuation flag (after spawning continuation turn).
    pub fn clear_continuation(&self, session_id: &str) -> Result<(), String> {
        self.repo.set_continuation_needed(session_id, false)
    }

    // =========================================================================
    // SHUTDOWN & CRASH RECOVERY
    // =========================================================================

    /// Mark all running sessions and their executions as paused (graceful shutdown).
    ///
    /// This should be called during graceful server shutdown to pause active sessions
    /// so they can be resumed when the server restarts.
    pub fn mark_running_as_paused(&self) -> Result<u64, String> {
        let sessions = self.repo.list_sessions(&SessionFilter {
            status: Some(SessionStatus::Running),
            ..Default::default()
        })?;

        let count = sessions.len() as u64;

        for session in &sessions {
            // First, mark all running/queued executions in this session as paused
            let executions = self.repo.list_executions(&ExecutionFilter {
                session_id: Some(session.id.clone()),
                ..Default::default()
            })?;

            for exec in executions {
                if matches!(exec.status, ExecutionStatus::Running | ExecutionStatus::Queued) {
                    if let Err(e) = self.repo.update_execution_status(&exec.id, ExecutionStatus::Paused) {
                        tracing::warn!("Failed to pause execution {} during shutdown: {}", exec.id, e);
                    }
                }
            }

            // Then mark the session as paused
            if let Err(e) = self.repo.update_session_status(&session.id, SessionStatus::Paused) {
                tracing::warn!("Failed to pause session {} during shutdown: {}", session.id, e);
            }
        }

        Ok(count)
    }

    /// Mark all running sessions and their executions as crashed (on startup).
    ///
    /// This is called during server startup to clean up any sessions/executions
    /// that were still in RUNNING state - these must have been interrupted by
    /// an unexpected crash (graceful shutdown would have paused them).
    pub fn mark_running_as_crashed(&self) -> Result<u64, String> {
        let sessions = self.repo.list_sessions(&SessionFilter {
            status: Some(SessionStatus::Running),
            ..Default::default()
        })?;

        let count = sessions.len() as u64;

        for session in &sessions {
            // First, mark all running/queued executions in this session as crashed
            let executions = self.repo.list_executions(&ExecutionFilter {
                session_id: Some(session.id.clone()),
                ..Default::default()
            })?;

            for exec in executions {
                if matches!(exec.status, ExecutionStatus::Running | ExecutionStatus::Queued) {
                    if let Err(e) = self.repo.update_execution_status(&exec.id, ExecutionStatus::Crashed) {
                        tracing::warn!("Failed to crash execution {} during recovery: {}", exec.id, e);
                    }
                }
            }

            // Then mark the session as crashed
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

    /// Get messages for a session with scope filtering.
    ///
    /// Scopes:
    /// - `all`: All messages from all executions
    /// - `root`: Only messages from root executions (main chat view)
    /// - `execution`: Messages from a specific execution (requires query.execution_id)
    /// - `delegates`: Only messages from delegated executions
    pub fn get_session_messages(
        &self,
        session_id: &str,
        query: &SessionMessagesQuery,
    ) -> Result<Vec<SessionMessage>, String> {
        self.repo.get_session_messages(
            session_id,
            query.scope.as_str(),
            query.execution_id.as_deref(),
            query.agent_id.as_deref(),
        )
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
            let conn =
                Connection::open_in_memory().expect("Failed to create in-memory database");

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
                    ward_id TEXT
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
                    FOREIGN KEY (session_id) REFERENCES sessions(id)
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    execution_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    tool_calls TEXT,
                    tool_results TEXT,
                    FOREIGN KEY (execution_id) REFERENCES agent_executions(id)
                );

                CREATE INDEX IF NOT EXISTS idx_agent_executions_session ON agent_executions(session_id);
                CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
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

    fn setup_service() -> StateService<TestDbProvider> {
        let db = Arc::new(TestDbProvider::new());
        StateService::new(db)
    }

    // ========================================================================
    // Session Lifecycle Tests
    // ========================================================================

    #[test]
    fn test_create_session() {
        let service = setup_service();
        let (session, execution) = service.create_session("test-agent").unwrap();

        assert_eq!(session.root_agent_id, "test-agent");
        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(execution.agent_id, "test-agent");
        assert_eq!(execution.session_id, session.id);
    }

    #[test]
    fn test_get_session() {
        let service = setup_service();
        let (session, _) = service.create_session("test-agent").unwrap();

        let retrieved = service.get_session(&session.id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, session.id);
    }

    // ========================================================================
    // Delegation Tracking Tests
    // ========================================================================

    #[test]
    fn test_delegation_tracking() {
        let service = setup_service();
        let (session, _) = service.create_session("test-agent").unwrap();

        // Register 2 delegations
        service.register_delegation(&session.id).unwrap();
        service.register_delegation(&session.id).unwrap();

        let session = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session.pending_delegations, 2);

        // Complete first - should return false (more pending, no continuation requested)
        let trigger = service.complete_delegation(&session.id).unwrap();
        assert!(!trigger);

        // Check pending count
        let session = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session.pending_delegations, 1);

        // Complete second - should return false (no continuation requested)
        let trigger = service.complete_delegation(&session.id).unwrap();
        assert!(!trigger);

        // Verify all delegations completed
        let session = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session.pending_delegations, 0);
    }

    #[test]
    fn test_continuation_trigger() {
        let service = setup_service();
        let (session, _) = service.create_session("test-agent").unwrap();

        // Register delegation and request continuation
        service.register_delegation(&session.id).unwrap();
        service.request_continuation(&session.id).unwrap();

        // Complete delegation - should trigger continuation
        let trigger = service.complete_delegation(&session.id).unwrap();
        assert!(trigger);
    }

    #[test]
    fn test_needs_continuation() {
        let service = setup_service();
        let (session, _) = service.create_session("test-agent").unwrap();

        // Initially doesn't need continuation
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert!(!session_state.needs_continuation());

        // With pending delegations, still doesn't need continuation
        service.register_delegation(&session.id).unwrap();
        service.request_continuation(&session.id).unwrap();
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert!(!session_state.needs_continuation()); // has pending

        // After delegations complete, needs continuation
        service.complete_delegation(&session.id).unwrap();
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert!(session_state.needs_continuation());
    }

    #[test]
    fn test_clear_continuation() {
        let service = setup_service();
        let (session, _) = service.create_session("test-agent").unwrap();

        // Request and then clear continuation
        service.request_continuation(&session.id).unwrap();

        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert!(session_state.continuation_needed);

        service.clear_continuation(&session.id).unwrap();

        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert!(!session_state.continuation_needed);
    }

    #[test]
    fn test_multiple_delegations_with_continuation() {
        let service = setup_service();
        let (session, _) = service.create_session("root-agent").unwrap();

        // Spawn 3 delegations
        service.register_delegation(&session.id).unwrap();
        service.register_delegation(&session.id).unwrap();
        service.register_delegation(&session.id).unwrap();

        // Request continuation
        service.request_continuation(&session.id).unwrap();

        // Complete first 2 - should NOT trigger continuation yet
        assert!(!service.complete_delegation(&session.id).unwrap());
        assert!(!service.complete_delegation(&session.id).unwrap());

        // Session should still have 1 pending
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.pending_delegations, 1);
        assert!(!session_state.needs_continuation()); // still has pending

        // Complete last one - should trigger continuation
        assert!(service.complete_delegation(&session.id).unwrap());

        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.pending_delegations, 0);
        assert!(session_state.needs_continuation());
    }

    #[test]
    fn test_decrement_does_not_go_negative() {
        let service = setup_service();
        let (session, _) = service.create_session("test-agent").unwrap();

        // Complete without registering any delegations
        let trigger = service.complete_delegation(&session.id).unwrap();
        assert!(!trigger);

        // Should remain at 0, not go negative
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.pending_delegations, 0);
    }

    // ========================================================================
    // Session Auto-Complete Tests
    // ========================================================================

    #[test]
    fn test_web_session_does_not_auto_complete() {
        let service = setup_service();
        // Default source is Web
        let (session, execution) = service.create_session("test-agent").unwrap();
        assert_eq!(session.source, TriggerSource::Web);

        // Complete the execution
        service.complete_execution(&execution.id).unwrap();

        // Try to auto-complete - should return false for Web sessions
        let completed = service.try_complete_session(&session.id).unwrap();
        assert!(!completed, "Web sessions should NOT auto-complete");

        // Session should still be Running
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.status, SessionStatus::Running);
    }

    #[test]
    fn test_cli_session_auto_completes() {
        let service = setup_service();
        let (session, execution) = service.create_session_with_source("test-agent", TriggerSource::Cli).unwrap();
        assert_eq!(session.source, TriggerSource::Cli);

        // Complete the execution
        service.complete_execution(&execution.id).unwrap();

        // Try to auto-complete - should return true for CLI sessions
        let completed = service.try_complete_session(&session.id).unwrap();
        assert!(completed, "CLI sessions should auto-complete");

        // Session should be Completed
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.status, SessionStatus::Completed);
    }

    #[test]
    fn test_cron_session_auto_completes() {
        let service = setup_service();
        let (session, execution) = service.create_session_with_source("test-agent", TriggerSource::Cron).unwrap();
        assert_eq!(session.source, TriggerSource::Cron);

        // Complete the execution
        service.complete_execution(&execution.id).unwrap();

        // Try to auto-complete - should return true for Cron sessions
        let completed = service.try_complete_session(&session.id).unwrap();
        assert!(completed, "Cron sessions should auto-complete");

        // Session should be Completed
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.status, SessionStatus::Completed);
    }

    #[test]
    fn test_session_with_running_execution_does_not_complete() {
        let service = setup_service();
        let (session, execution) = service.create_session_with_source("test-agent", TriggerSource::Cli).unwrap();

        // Start but don't complete the execution
        service.start_execution(&execution.id).unwrap();

        // Try to auto-complete - should return false (execution still running)
        let completed = service.try_complete_session(&session.id).unwrap();
        assert!(!completed, "Sessions with running executions should not complete");

        // Session should still be Running
        let session_state = service.get_session(&session.id).unwrap().unwrap();
        assert_eq!(session_state.status, SessionStatus::Running);
    }

    // ========================================================================
    // Session Messages Tests
    // ========================================================================

    fn setup_session_with_messages(service: &StateService<TestDbProvider>) -> (Session, AgentExecution, AgentExecution) {
        let (session, root_exec) = service.create_session("root-agent").unwrap();

        // Create delegated execution
        let delegate_exec = service.create_delegated_execution(
            &session.id,
            "researcher",
            &root_exec.id,
            DelegationType::Sequential,
            "Research task",
        ).unwrap();

        // Add messages to root execution
        service.add_message(&root_exec.id, "user", "Hello root", None, None).unwrap();
        service.add_message(&root_exec.id, "assistant", "Root response", None, None).unwrap();

        // Add messages to delegated execution
        service.add_message(&delegate_exec.id, "user", "Research this", None, None).unwrap();
        service.add_message(&delegate_exec.id, "assistant", "Research results", None, None).unwrap();

        (session, root_exec, delegate_exec)
    }

    #[test]
    fn test_session_messages_all_scope() {
        let service = setup_service();
        let (session, _, _) = setup_session_with_messages(&service);

        let query = SessionMessagesQuery {
            scope: crate::handlers::MessageScope::All,
            execution_id: None,
            agent_id: None,
        };

        let messages = service.get_session_messages(&session.id, &query).unwrap();

        // Should return all 4 messages (2 from root, 2 from delegate)
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn test_session_messages_root_scope() {
        let service = setup_service();
        let (session, _, _) = setup_session_with_messages(&service);

        let query = SessionMessagesQuery {
            scope: crate::handlers::MessageScope::Root,
            execution_id: None,
            agent_id: None,
        };

        let messages = service.get_session_messages(&session.id, &query).unwrap();

        // Should return only root execution messages (2)
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.agent_id == "root-agent"));
        assert!(messages.iter().all(|m| m.delegation_type == "root"));
    }

    #[test]
    fn test_session_messages_delegates_scope() {
        let service = setup_service();
        let (session, _, _) = setup_session_with_messages(&service);

        let query = SessionMessagesQuery {
            scope: crate::handlers::MessageScope::Delegates,
            execution_id: None,
            agent_id: None,
        };

        let messages = service.get_session_messages(&session.id, &query).unwrap();

        // Should return only delegated execution messages (2)
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.agent_id == "researcher"));
        assert!(messages.iter().all(|m| m.delegation_type == "sequential"));
    }

    #[test]
    fn test_session_messages_execution_scope() {
        let service = setup_service();
        let (session, _, delegate_exec) = setup_session_with_messages(&service);

        let query = SessionMessagesQuery {
            scope: crate::handlers::MessageScope::Execution,
            execution_id: Some(delegate_exec.id.clone()),
            agent_id: None,
        };

        let messages = service.get_session_messages(&session.id, &query).unwrap();

        // Should return only the delegated execution's messages (2)
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.execution_id == delegate_exec.id));
    }

    #[test]
    fn test_session_messages_agent_filter() {
        let service = setup_service();
        let (session, _, _) = setup_session_with_messages(&service);

        let query = SessionMessagesQuery {
            scope: crate::handlers::MessageScope::All,
            execution_id: None,
            agent_id: Some("researcher".to_string()),
        };

        let messages = service.get_session_messages(&session.id, &query).unwrap();

        // Should return only researcher agent's messages (2)
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().all(|m| m.agent_id == "researcher"));
    }
}
