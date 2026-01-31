//! # State Service
//!
//! Business logic for execution state management.

use crate::repository::{StateDbProvider, StateRepository};
use crate::types::*;
use std::sync::Arc;

// ============================================================================
// SERVICE
// ============================================================================

/// Service for execution state management.
///
/// Provides session lifecycle management, token tracking, and checkpointing.
/// Used by gateway's execution runner for state updates and by handlers for queries.
pub struct StateService<D: StateDbProvider> {
    repo: StateRepository<D>,
}

impl<D: StateDbProvider> StateService<D> {
    /// Create a new state service.
    pub fn new(db: Arc<D>) -> Self {
        Self {
            repo: StateRepository::new(db),
        }
    }

    // =========================================================================
    // SESSION LIFECYCLE
    // =========================================================================

    /// Create a new execution session in QUEUED state.
    pub fn create_session(
        &self,
        conversation_id: &str,
        agent_id: &str,
        parent_session_id: Option<String>,
    ) -> Result<ExecutionSession, String> {
        let session = ExecutionSession::new(conversation_id, agent_id, parent_session_id);
        self.repo.create_session(&session)?;
        Ok(session)
    }

    /// Start a session (QUEUED → RUNNING).
    pub fn start_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;

        if session.status != ExecutionStatus::Queued {
            return Err(format!(
                "Cannot start session in {} state",
                session.status.as_str()
            ));
        }

        self.repo.update_status(session_id, ExecutionStatus::Running)
    }

    /// Pause a running session (RUNNING → PAUSED).
    pub fn pause_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;

        if session.status != ExecutionStatus::Running {
            return Err(format!(
                "Cannot pause session in {} state",
                session.status.as_str()
            ));
        }

        self.repo.update_status(session_id, ExecutionStatus::Paused)
    }

    /// Resume a paused or crashed session (PAUSED/CRASHED → RUNNING).
    pub fn resume_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;

        if !session.status.is_resumable() {
            return Err(format!(
                "Cannot resume session in {} state",
                session.status.as_str()
            ));
        }

        self.repo.update_status(session_id, ExecutionStatus::Running)
    }

    /// Cancel a session (any non-terminal → CANCELLED).
    pub fn cancel_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;

        if session.status.is_terminal() {
            return Err(format!(
                "Cannot cancel session in {} state",
                session.status.as_str()
            ));
        }

        self.repo.update_status(session_id, ExecutionStatus::Cancelled)
    }

    /// Complete a session successfully (RUNNING → COMPLETED).
    pub fn complete_session(&self, session_id: &str) -> Result<(), String> {
        let session = self.get_session(session_id)?;

        if session.status != ExecutionStatus::Running {
            return Err(format!(
                "Cannot complete session in {} state",
                session.status.as_str()
            ));
        }

        self.repo.update_status(session_id, ExecutionStatus::Completed)
    }

    /// Mark a session as crashed (RUNNING → CRASHED).
    pub fn crash_session(&self, session_id: &str, error: &str) -> Result<(), String> {
        self.repo.set_error(session_id, error)?;
        self.repo.update_status(session_id, ExecutionStatus::Crashed)
    }

    // =========================================================================
    // TOKEN TRACKING
    // =========================================================================

    /// Update token counts for a session.
    pub fn update_tokens(&self, session_id: &str, tokens_in: u64, tokens_out: u64) -> Result<(), String> {
        self.repo.update_tokens(session_id, tokens_in, tokens_out)
    }

    // =========================================================================
    // CHECKPOINTING
    // =========================================================================

    /// Save a checkpoint for crash recovery.
    pub fn save_checkpoint(&self, session_id: &str, checkpoint: &Checkpoint) -> Result<(), String> {
        self.repo.save_checkpoint(session_id, checkpoint)
    }

    // =========================================================================
    // CRASH RECOVERY
    // =========================================================================

    /// Mark all RUNNING sessions as CRASHED.
    ///
    /// Call this on daemon startup to handle sessions that were interrupted
    /// by a daemon crash.
    pub fn mark_running_as_crashed(&self) -> Result<u64, String> {
        let running = self.repo.get_running()?;
        let count = running.len() as u64;

        for session in running {
            self.repo.set_error(&session.id, "Daemon crashed during execution")?;
            self.repo.update_status(&session.id, ExecutionStatus::Crashed)?;
        }

        Ok(count)
    }

    /// Get all resumable sessions (PAUSED or CRASHED with checkpoint).
    pub fn get_resumable_sessions(&self) -> Result<Vec<ExecutionSession>, String> {
        self.repo.get_resumable()
    }

    // =========================================================================
    // QUERY OPERATIONS
    // =========================================================================

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Result<ExecutionSession, String> {
        self.repo
            .get_session(session_id)?
            .ok_or_else(|| format!("Session not found: {}", session_id))
    }

    /// Get a session by ID (returns Option).
    pub fn find_session(&self, session_id: &str) -> Result<Option<ExecutionSession>, String> {
        self.repo.get_session(session_id)
    }

    /// List sessions with filtering.
    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<Vec<ExecutionSession>, String> {
        self.repo.list_sessions(filter)
    }

    /// Get child sessions for a parent.
    pub fn get_child_sessions(&self, parent_session_id: &str) -> Result<Vec<ExecutionSession>, String> {
        self.repo.get_children(parent_session_id)
    }

    /// Get all currently running sessions.
    pub fn get_running_sessions(&self) -> Result<Vec<ExecutionSession>, String> {
        self.repo.get_running()
    }

    /// Get sessions by status.
    pub fn get_sessions_by_status(&self, status: ExecutionStatus) -> Result<Vec<ExecutionSession>, String> {
        self.repo.get_by_status(status)
    }

    // =========================================================================
    // AGGREGATES
    // =========================================================================

    /// Get daily summary for a date (YYYY-MM-DD).
    pub fn get_daily_summary(&self, date: &str) -> Result<DailySummary, String> {
        self.repo.get_daily_summary(date)
    }

    /// Get status counts for all sessions.
    pub fn get_status_counts(&self) -> Result<std::collections::HashMap<String, u64>, String> {
        self.repo.get_status_counts()
    }

    // =========================================================================
    // CLEANUP
    // =========================================================================

    /// Delete a session.
    pub fn delete_session(&self, session_id: &str) -> Result<bool, String> {
        self.repo.delete_session(session_id)
    }

    /// Delete old completed sessions.
    pub fn cleanup_old_sessions(&self, older_than: &str) -> Result<u64, String> {
        self.repo.delete_old_sessions(older_than)
    }
}
