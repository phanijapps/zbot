//! Test utilities for execution-state tests.
//!
//! Provides helper functions for creating mock data and test fixtures.

use crate::types::*;
use tempfile::TempDir;

/// Create a temporary directory for test databases.
///
/// Returns the TempDir (which must be kept alive) and the database path.
pub fn temp_db_path() -> (TempDir, String) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let path = dir.path().join("test.db");
    (dir, path.to_string_lossy().to_string())
}

/// Create a mock session for testing.
///
/// Creates a session in Running state with Web source.
pub fn mock_session(id: &str, agent_id: &str) -> Session {
    Session {
        id: id.to_string(),
        status: SessionStatus::Running,
        source: TriggerSource::Web,
        root_agent_id: agent_id.to_string(),
        title: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        completed_at: None,
        total_tokens_in: 0,
        total_tokens_out: 0,
        metadata: None,
        pending_delegations: 0,
        continuation_needed: false,
    }
}

/// Create a mock session with a specific source.
pub fn mock_session_with_source(id: &str, agent_id: &str, source: TriggerSource) -> Session {
    Session {
        id: id.to_string(),
        status: SessionStatus::Running,
        source,
        root_agent_id: agent_id.to_string(),
        title: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        completed_at: None,
        total_tokens_in: 0,
        total_tokens_out: 0,
        metadata: None,
        pending_delegations: 0,
        continuation_needed: false,
    }
}

/// Create a mock queued session.
pub fn mock_queued_session(id: &str, agent_id: &str, source: TriggerSource) -> Session {
    Session {
        id: id.to_string(),
        status: SessionStatus::Queued,
        source,
        root_agent_id: agent_id.to_string(),
        title: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        started_at: None,
        completed_at: None,
        total_tokens_in: 0,
        total_tokens_out: 0,
        metadata: None,
        pending_delegations: 0,
        continuation_needed: false,
    }
}

/// Create a mock root execution for testing.
pub fn mock_execution(id: &str, session_id: &str, agent_id: &str) -> AgentExecution {
    AgentExecution {
        id: id.to_string(),
        session_id: session_id.to_string(),
        agent_id: agent_id.to_string(),
        parent_execution_id: None,
        delegation_type: DelegationType::Root,
        task: None,
        status: ExecutionStatus::Running,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        completed_at: None,
        tokens_in: 0,
        tokens_out: 0,
        checkpoint: None,
        error: None,
        log_path: None,
    }
}

/// Create a mock subagent execution for testing.
pub fn mock_subagent_execution(
    id: &str,
    session_id: &str,
    agent_id: &str,
    parent_execution_id: &str,
    task: &str,
) -> AgentExecution {
    AgentExecution {
        id: id.to_string(),
        session_id: session_id.to_string(),
        agent_id: agent_id.to_string(),
        parent_execution_id: Some(parent_execution_id.to_string()),
        delegation_type: DelegationType::Sequential,
        task: Some(task.to_string()),
        status: ExecutionStatus::Running,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        completed_at: None,
        tokens_in: 0,
        tokens_out: 0,
        checkpoint: None,
        error: None,
        log_path: None,
    }
}

/// Create a mock completed execution.
pub fn mock_completed_execution(id: &str, session_id: &str, agent_id: &str) -> AgentExecution {
    let now = chrono::Utc::now();
    AgentExecution {
        id: id.to_string(),
        session_id: session_id.to_string(),
        agent_id: agent_id.to_string(),
        parent_execution_id: None,
        delegation_type: DelegationType::Root,
        task: None,
        status: ExecutionStatus::Completed,
        started_at: Some((now - chrono::Duration::seconds(30)).to_rfc3339()),
        completed_at: Some(now.to_rfc3339()),
        tokens_in: 1000,
        tokens_out: 500,
        checkpoint: None,
        error: None,
        log_path: None,
    }
}

/// Create mock dashboard stats for testing.
pub fn mock_dashboard_stats() -> DashboardStats {
    let mut sessions_by_source = std::collections::HashMap::new();
    sessions_by_source.insert("web".to_string(), 5);
    sessions_by_source.insert("cli".to_string(), 2);

    DashboardStats {
        sessions_queued: 1,
        sessions_running: 2,
        sessions_paused: 0,
        sessions_completed: 10,
        sessions_crashed: 1,
        executions_queued: 0,
        executions_running: 3,
        executions_completed: 15,
        executions_crashed: 2,
        executions_cancelled: 0,
        today_sessions: 5,
        today_tokens: 50000,
        sessions_by_source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_session_creates_valid_session() {
        let session = mock_session("sess-test", "root");
        
        assert_eq!(session.id, "sess-test");
        assert_eq!(session.root_agent_id, "root");
        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.source, TriggerSource::Web);
    }

    #[test]
    fn test_mock_execution_creates_valid_execution() {
        let exec = mock_execution("exec-test", "sess-test", "root");
        
        assert_eq!(exec.id, "exec-test");
        assert_eq!(exec.session_id, "sess-test");
        assert_eq!(exec.agent_id, "root");
        assert!(exec.parent_execution_id.is_none());
        assert_eq!(exec.delegation_type, DelegationType::Root);
        assert_eq!(exec.status, ExecutionStatus::Running);
    }

    #[test]
    fn test_mock_subagent_execution_has_parent() {
        let exec = mock_subagent_execution(
            "exec-sub",
            "sess-test",
            "researcher",
            "exec-root",
            "Research AI topics",
        );
        
        assert_eq!(exec.parent_execution_id, Some("exec-root".to_string()));
        assert_eq!(exec.delegation_type, DelegationType::Sequential);
        assert_eq!(exec.task, Some("Research AI topics".to_string()));
    }

    #[test]
    fn test_temp_db_path_creates_valid_path() {
        let (dir, path) = temp_db_path();
        
        assert!(path.contains("test.db"));
        assert!(dir.path().exists());
    }
}
