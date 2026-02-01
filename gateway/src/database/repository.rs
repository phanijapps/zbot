// ============================================================================
// MESSAGE REPOSITORY
// CRUD operations for messages linked to agent executions
// ============================================================================

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::DatabaseManager;

// ============================================================================
// TYPES
// ============================================================================

/// A message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub execution_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: i32,
    pub tool_calls: Option<String>,
    pub tool_results: Option<String>,
}

// ============================================================================
// REPOSITORY
// ============================================================================

/// Repository for message operations (now linked to executions)
pub struct ConversationRepository {
    db: Arc<DatabaseManager>,
}

impl ConversationRepository {
    /// Create a new repository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    // =========================================================================
    // LEGACY COMPATIBILITY
    // These methods exist for backward compatibility during migration
    // =========================================================================

    /// Get or create a conversation - now a no-op since sessions/executions are created elsewhere
    pub fn get_or_create_conversation(
        &self,
        _conversation_id: &str,
        _agent_id: &str,
    ) -> Result<(), String> {
        // Sessions and executions are created by StateService
        // This is now a no-op for compatibility
        Ok(())
    }

    // =========================================================================
    // MESSAGE OPERATIONS
    // =========================================================================

    /// Add a message to an execution
    pub fn add_message(
        &self,
        execution_id: &str,
        role: &str,
        content: &str,
        tool_calls: Option<&str>,
        tool_results: Option<&str>,
    ) -> Result<Message, String> {
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let token_count = content.len() as i32 / 4; // Rough estimate

        let message = Message {
            id: id.clone(),
            execution_id: execution_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now.clone(),
            token_count,
            tool_calls: tool_calls.map(String::from),
            tool_results: tool_results.map(String::from),
        };

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO messages (id, execution_id, role, content, created_at, token_count, tool_calls, tool_results)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    message.id,
                    message.execution_id,
                    message.role,
                    message.content,
                    message.created_at,
                    message.token_count,
                    message.tool_calls,
                    message.tool_results,
                ],
            )?;
            Ok(())
        })?;

        Ok(message)
    }

    /// Get messages for an execution
    pub fn get_messages(&self, execution_id: &str) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, execution_id, role, content, created_at, token_count, tool_calls, tool_results
                 FROM messages
                 WHERE execution_id = ?1
                 ORDER BY created_at ASC",
            )?;

            let rows = stmt.query_map([execution_id], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    execution_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    token_count: row.get(5)?,
                    tool_calls: row.get(6)?,
                    tool_results: row.get(7)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Get recent messages for context (with limit)
    pub fn get_recent_messages(
        &self,
        execution_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, execution_id, role, content, created_at, token_count, tool_calls, tool_results
                 FROM messages
                 WHERE execution_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![execution_id, limit as i64], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    execution_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    token_count: row.get(5)?,
                    tool_calls: row.get(6)?,
                    tool_results: row.get(7)?,
                })
            })?;

            // Collect and reverse to get chronological order
            let mut messages: Vec<Message> = rows.collect::<Result<Vec<_>, _>>()?;
            messages.reverse();
            Ok(messages)
        })
    }

    /// Get messages from all root executions in a session.
    ///
    /// This loads the full conversation history across all root agent turns,
    /// including callback messages from completed subagents.
    pub fn get_session_root_messages(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            // Join messages with agent_executions to filter by session and root only
            // Root executions have delegation_type = 'root'
            let mut stmt = conn.prepare(
                "SELECT m.id, m.execution_id, m.role, m.content, m.created_at,
                        m.token_count, m.tool_calls, m.tool_results
                 FROM messages m
                 INNER JOIN agent_executions e ON m.execution_id = e.id
                 WHERE e.session_id = ?1 AND e.delegation_type = 'root'
                 ORDER BY m.created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![session_id, limit as i64], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    execution_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    token_count: row.get(5)?,
                    tool_calls: row.get(6)?,
                    tool_results: row.get(7)?,
                })
            })?;

            // Collect and reverse to get chronological order
            let mut messages: Vec<Message> = rows.collect::<Result<Vec<_>, _>>()?;
            messages.reverse();
            Ok(messages)
        })
    }

    /// Convert messages to ChatMessage format for LLM
    pub fn messages_to_chat_format(&self, messages: &[Message]) -> Vec<agent_runtime::ChatMessage> {
        messages
            .iter()
            .map(|m| agent_runtime::ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
                tool_calls: None,
                tool_call_id: None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test database using tempfile
    fn create_test_db() -> Arc<DatabaseManager> {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let _ = temp_dir.keep(); // Prevent cleanup during test
        let db = DatabaseManager::new(path).unwrap();
        Arc::new(db)
    }

    fn create_test_session(db: &DatabaseManager, session_id: &str, root_agent_id: &str) {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions (id, status, source, root_agent_id, created_at)
                 VALUES (?1, 'running', 'web', ?2, datetime('now'))",
                params![session_id, root_agent_id],
            )?;
            Ok(())
        })
        .unwrap();
    }

    fn create_test_execution(
        db: &DatabaseManager,
        exec_id: &str,
        session_id: &str,
        agent_id: &str,
        delegation_type: &str,
    ) {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO agent_executions (id, session_id, agent_id, delegation_type, status)
                 VALUES (?1, ?2, ?3, ?4, 'running')",
                params![exec_id, session_id, agent_id, delegation_type],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_get_session_root_messages_filters_by_session() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        // Create two sessions
        create_test_session(&db, "session-1", "root-agent");
        create_test_session(&db, "session-2", "root-agent");

        // Create root executions in each session
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");
        create_test_execution(&db, "exec-2", "session-2", "root-agent", "root");

        // Add messages to both
        repo.add_message("exec-1", "user", "Message in session 1", None, None)
            .unwrap();
        repo.add_message("exec-2", "user", "Message in session 2", None, None)
            .unwrap();

        // Query session 1 - should only get session 1 messages
        let messages = repo.get_session_root_messages("session-1", 50).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Message in session 1");
    }

    #[test]
    fn test_get_session_root_messages_excludes_delegate_executions() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");

        // Create root and delegate executions
        create_test_execution(&db, "root-exec", "session-1", "root-agent", "root");
        create_test_execution(&db, "delegate-exec", "session-1", "sub-agent", "sequential");

        // Add messages to both
        repo.add_message("root-exec", "user", "Root message", None, None)
            .unwrap();
        repo.add_message("delegate-exec", "user", "Delegate message", None, None)
            .unwrap();

        // Query should only get root messages
        let messages = repo.get_session_root_messages("session-1", 50).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Root message");
    }

    #[test]
    fn test_get_session_root_messages_includes_all_root_executions() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");

        // Create multiple root executions (simulating multiple user messages)
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");
        create_test_execution(&db, "exec-2", "session-1", "root-agent", "root");

        // Add messages to both root executions
        repo.add_message("exec-1", "user", "First user message", None, None)
            .unwrap();
        repo.add_message("exec-1", "assistant", "First response", None, None)
            .unwrap();
        repo.add_message("exec-1", "system", "Callback from subagent", None, None)
            .unwrap();
        repo.add_message("exec-2", "user", "Second user message", None, None)
            .unwrap();

        // Query should get all root messages in chronological order
        let messages = repo.get_session_root_messages("session-1", 50).unwrap();
        assert_eq!(messages.len(), 4);

        // Verify order (chronological)
        assert_eq!(messages[0].content, "First user message");
        assert_eq!(messages[1].content, "First response");
        assert_eq!(messages[2].content, "Callback from subagent");
        assert_eq!(messages[3].content, "Second user message");
    }

    #[test]
    fn test_get_session_root_messages_respects_limit() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");

        // Add 5 messages
        for i in 1..=5 {
            repo.add_message("exec-1", "user", &format!("Message {}", i), None, None)
                .unwrap();
        }

        // Query with limit of 3 - should get the MOST RECENT 3
        let messages = repo.get_session_root_messages("session-1", 3).unwrap();
        assert_eq!(messages.len(), 3);

        // Should be messages 3, 4, 5 in chronological order
        assert_eq!(messages[0].content, "Message 3");
        assert_eq!(messages[1].content, "Message 4");
        assert_eq!(messages[2].content, "Message 5");
    }
}
