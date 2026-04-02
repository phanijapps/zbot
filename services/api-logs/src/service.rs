//! # Log Service
//!
//! Business logic for execution logging.

use crate::repository::{DbProvider, LogsRepository};
use crate::types::*;
use std::sync::Arc;

// ============================================================================
// SERVICE
// ============================================================================

/// Service for execution logging.
///
/// Provides both log emission (called by runner) and query APIs (called by handlers).
pub struct LogService<D: DbProvider> {
    repo: LogsRepository<D>,
}

impl<D: DbProvider> LogService<D> {
    /// Create a new log service.
    pub fn new(db: Arc<D>) -> Self {
        Self {
            repo: LogsRepository::new(db),
        }
    }

    // =========================================================================
    // LOG EMISSION
    // =========================================================================

    /// Log a single entry.
    pub fn log(&self, entry: ExecutionLog) -> Result<(), String> {
        self.repo.insert_log(&entry)
    }

    /// Log multiple entries in a batch.
    pub fn log_batch(&self, entries: Vec<ExecutionLog>) -> Result<(), String> {
        self.repo.insert_batch(&entries)
    }

    /// Log session start.
    pub fn log_session_start(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        parent_session_id: Option<&str>,
    ) -> Result<(), String> {
        let mut log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            LogLevel::Info,
            LogCategory::Session,
            "Session started",
        );

        if let Some(parent) = parent_session_id {
            log = log.with_parent(parent);
        }

        self.repo.insert_log(&log)
    }

    /// Log session end.
    pub fn log_session_end(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        status: SessionStatus,
        result_message: Option<&str>,
    ) -> Result<(), String> {
        let message = match status {
            SessionStatus::Completed => {
                result_message.unwrap_or("Session completed successfully")
            }
            SessionStatus::Error => result_message.unwrap_or("Session ended with error"),
            SessionStatus::Stopped => result_message.unwrap_or("Session stopped by user"),
            SessionStatus::Running => "Session running", // Shouldn't happen
        };

        let level = match status {
            SessionStatus::Error => LogLevel::Error,
            _ => LogLevel::Info,
        };

        let log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            level,
            LogCategory::Session,
            message,
        )
        .with_metadata(serde_json::json!({
            "status": status.as_str()
        }));

        self.repo.insert_log(&log)
    }

    /// Log tool call start.
    pub fn log_tool_call(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        tool_name: &str,
        tool_id: &str,
        args: &serde_json::Value,
    ) -> Result<(), String> {
        let log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            LogLevel::Info,
            LogCategory::ToolCall,
            format!("Calling tool: {}", tool_name),
        )
        .with_metadata(serde_json::json!({
            "tool_name": tool_name,
            "tool_id": tool_id,
            "args": args
        }));

        self.repo.insert_log(&log)
    }

    /// Log tool result.
    pub fn log_tool_result(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        tool_name: &str,
        tool_id: &str,
        result: &str,
        error: Option<&str>,
        duration_ms: i64,
    ) -> Result<(), String> {
        let (level, message) = if error.is_some() {
            (LogLevel::Error, format!("Tool {} failed", tool_name))
        } else {
            (LogLevel::Info, format!("Tool {} completed", tool_name))
        };

        // Truncate result for storage
        let truncated_result = if result.len() > 1000 {
            format!("{}...(truncated)", &result[..result.floor_char_boundary(1000)])
        } else {
            result.to_string()
        };

        let log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            level,
            LogCategory::ToolResult,
            message,
        )
        .with_metadata(serde_json::json!({
            "tool_name": tool_name,
            "tool_id": tool_id,
            "result": truncated_result,
            "error": error
        }))
        .with_duration(duration_ms);

        self.repo.insert_log(&log)
    }

    /// Log delegation start.
    pub fn log_delegation_start(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        child_agent_id: &str,
        child_session_id: &str,
        task: &str,
    ) -> Result<(), String> {
        let log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            LogLevel::Info,
            LogCategory::Delegation,
            format!("Delegating to agent: {}", child_agent_id),
        )
        .with_metadata(serde_json::json!({
            "child_agent_id": child_agent_id,
            "child_session_id": child_session_id,
            "task": task
        }));

        self.repo.insert_log(&log)
    }

    /// Log delegation complete.
    pub fn log_delegation_complete(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        child_agent_id: &str,
        child_session_id: &str,
        success: bool,
        result: Option<&str>,
    ) -> Result<(), String> {
        let (level, message) = if success {
            (
                LogLevel::Info,
                format!("Delegation to {} completed", child_agent_id),
            )
        } else {
            (
                LogLevel::Error,
                format!("Delegation to {} failed", child_agent_id),
            )
        };

        let log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            level,
            LogCategory::Delegation,
            message,
        )
        .with_metadata(serde_json::json!({
            "child_agent_id": child_agent_id,
            "child_session_id": child_session_id,
            "success": success,
            "result": result
        }));

        self.repo.insert_log(&log)
    }

    /// Log an error.
    pub fn log_error(
        &self,
        session_id: &str,
        conversation_id: &str,
        agent_id: &str,
        error_message: &str,
    ) -> Result<(), String> {
        let log = ExecutionLog::new(
            session_id,
            conversation_id,
            agent_id,
            LogLevel::Error,
            LogCategory::Error,
            error_message,
        );

        self.repo.insert_log(&log)
    }

    // =========================================================================
    // QUERY OPERATIONS
    // =========================================================================

    /// List sessions with optional filtering.
    pub fn list_sessions(&self, filter: &LogFilter) -> Result<Vec<LogSession>, String> {
        let mut sessions = self.repo.list_sessions(filter)?;

        // Batch-fetch titles from first user message
        let session_ids: Vec<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
        let titles = self.repo.get_session_titles(&session_ids).unwrap_or_default();

        // Enrich with child session IDs and titles
        for session in &mut sessions {
            if let Some(title) = titles.get(&session.session_id) {
                session.title = Some(title.clone());
            }

            if let Ok(children) = self.repo.get_child_sessions(&session.session_id) {
                session.child_session_ids = children;
            }

            // Compute status based on error count and ended_at
            if session.error_count > 0 {
                session.status = SessionStatus::Error;
            } else if session.ended_at.is_some() {
                session.status = SessionStatus::Completed;
            } else {
                session.status = SessionStatus::Running;
            }

            // Compute duration if both times are available
            if let (Some(started), Some(ended)) = (&Some(&session.started_at), &session.ended_at) {
                if let (Ok(start_time), Ok(end_time)) = (
                    chrono::DateTime::parse_from_rfc3339(started),
                    chrono::DateTime::parse_from_rfc3339(ended),
                ) {
                    session.duration_ms = Some((end_time - start_time).num_milliseconds());
                }
            }
        }

        Ok(sessions)
    }

    /// Get detailed session info with all logs.
    pub fn get_session_detail(&self, session_id: &str) -> Result<Option<SessionDetail>, String> {
        let session = self.repo.get_session(session_id)?;

        match session {
            Some(mut session) => {
                // Get logs
                let logs = self.repo.get_session_logs(session_id)?;

                // Get children
                if let Ok(children) = self.repo.get_child_sessions(session_id) {
                    session.child_session_ids = children;
                }

                // Enrich title from first user message
                if let Ok(titles) =
                    self.repo.get_session_titles(&[session_id.to_string()])
                {
                    if let Some(title) = titles.get(session_id) {
                        session.title = Some(title.clone());
                    }
                }

                // Compute status
                if session.error_count > 0 {
                    session.status = SessionStatus::Error;
                } else if session.ended_at.is_some() {
                    session.status = SessionStatus::Completed;
                } else {
                    session.status = SessionStatus::Running;
                }

                // Compute duration
                if let (Some(started), Some(ended)) =
                    (&Some(&session.started_at), &session.ended_at)
                {
                    if let (Ok(start_time), Ok(end_time)) = (
                        chrono::DateTime::parse_from_rfc3339(started),
                        chrono::DateTime::parse_from_rfc3339(ended),
                    ) {
                        session.duration_ms = Some((end_time - start_time).num_milliseconds());
                    }
                }

                Ok(Some(SessionDetail { session, logs }))
            }
            None => Ok(None),
        }
    }

    /// Check whether a session already has an intent analysis log.
    pub fn has_intent_log(&self, session_id: &str) -> bool {
        self.repo
            .has_category_log(session_id, "intent")
            .unwrap_or(false)
    }

    /// Delete a session and its logs.
    pub fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        self.repo.delete_session(session_id)
    }

    /// Delete old logs.
    pub fn delete_old_logs(&self, older_than: &str) -> Result<u64, String> {
        self.repo.delete_old_logs(older_than)
    }
}
