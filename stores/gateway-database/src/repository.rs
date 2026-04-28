// ============================================================================
// MESSAGE REPOSITORY
// CRUD operations for messages linked to agent executions
// ============================================================================

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::DatabaseManager;

// ============================================================================
// TYPES
// ============================================================================

/// A message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub execution_id: Option<String>,
    pub session_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub token_count: i32,
    pub tool_calls: Option<String>,
    pub tool_results: Option<String>,
    pub tool_call_id: Option<String>,
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
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        let token_count = content.len() as i32 / 4; // Rough estimate

        let message = Message {
            id: id.clone(),
            execution_id: Some(execution_id.to_string()),
            session_id: None,
            role: role.to_string(),
            content: content.to_string(),
            created_at: now.clone(),
            token_count,
            tool_calls: tool_calls.map(String::from),
            tool_results: tool_results.map(String::from),
            tool_call_id: None,
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
                "SELECT id, execution_id, role, content, created_at, token_count, tool_calls, tool_results, session_id, tool_call_id
                 FROM messages
                 WHERE execution_id = ?1
                 ORDER BY created_at ASC",
            )?;

            let rows = stmt.query_map([execution_id], Self::row_to_message)?;
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
                "SELECT id, execution_id, role, content, created_at, token_count, tool_calls, tool_results, session_id, tool_call_id
                 FROM messages
                 WHERE execution_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![execution_id, limit as i64], Self::row_to_message)?;

            // Collect and reverse to get chronological order
            let mut messages: Vec<Message> = rows.collect::<Result<Vec<_>, _>>()?;
            messages.reverse();
            Ok(messages)
        })
    }

    // =========================================================================

    /// Append a single message to a session's conversation stream.
    ///
    /// This is the primary write path for the session tree architecture.
    /// Messages are written directly to the session (not via execution JOIN).
    pub fn append_session_message(
        &self,
        session_id: &str,
        execution_id: &str,
        role: &str,
        content: &str,
        tool_calls: Option<&str>,
        tool_call_id: Option<&str>,
    ) -> Result<Message, String> {
        let now = Utc::now().to_rfc3339();
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        let token_count = content.len() as i32 / 4;

        let message = Message {
            id: id.clone(),
            execution_id: Some(execution_id.to_string()),
            session_id: Some(session_id.to_string()),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now,
            token_count,
            tool_calls: tool_calls.map(String::from),
            tool_results: None,
            tool_call_id: tool_call_id.map(String::from),
        };

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO messages (id, execution_id, session_id, role, content, created_at, token_count, tool_calls, tool_call_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    message.id,
                    message.execution_id,
                    message.session_id,
                    message.role,
                    message.content,
                    message.created_at,
                    message.token_count,
                    message.tool_calls,
                    message.tool_call_id,
                ],
            )?;
            Ok(())
        })?;

        Ok(message)
    }

    /// Get full conversation for a session (no JOIN needed).
    ///
    /// Returns messages in chronological order, newest-limited then reversed.
    pub fn get_session_conversation(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, execution_id, role, content, created_at, token_count, tool_calls, tool_results, session_id, tool_call_id
                 FROM messages
                 WHERE session_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![session_id, limit as i64], Self::row_to_message)?;

            let mut messages: Vec<Message> = rows.collect::<Result<Vec<_>, _>>()?;
            messages.reverse();
            Ok(messages)
        })
    }

    /// Get the ward_id for a session (from the sessions table).
    ///
    /// Returns `None` if the session has no ward or the session doesn't exist.
    pub fn get_session_ward_id(&self, session_id: &str) -> Result<Option<String>, String> {
        self.db.with_connection(|conn| {
            let result = conn.query_row(
                "SELECT ward_id FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get::<_, Option<String>>(0),
            );
            match result {
                Ok(ward_id) => Ok(ward_id),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Get the root_agent_id for a session.
    ///
    /// Returns `None` if the session doesn't exist.
    pub fn get_session_agent_id(&self, session_id: &str) -> Result<Option<String>, String> {
        self.db.with_connection(|conn| {
            let result = conn.query_row(
                "SELECT root_agent_id FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get::<_, String>(0),
            );
            match result {
                Ok(agent_id) => Ok(Some(agent_id)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Convert session messages to ChatMessage format for LLM.
    ///
    /// Handles role='tool' messages with tool_call_id, and assistant messages
    /// with tool_calls arrays. This produces the exact format the LLM expects.
    pub fn session_messages_to_chat_format(
        &self,
        messages: &[Message],
    ) -> Vec<agent_runtime::ChatMessage> {
        messages
            .iter()
            .map(|m| {
                // Parse tool_calls if present on assistant messages
                let tool_calls = if m.role == "assistant" {
                    m.tool_calls
                        .as_ref()
                        .and_then(|tc_json| self.parse_tool_calls_json(tc_json))
                } else {
                    None
                };

                agent_runtime::ChatMessage {
                    role: m.role.clone(),
                    content: vec![zero_core::types::Part::Text {
                        text: m.content.clone(),
                    }],
                    tool_calls,
                    tool_call_id: m.tool_call_id.clone(),
                    is_summary: false,
                }
            })
            .collect()
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    /// Map a database row to a Message struct.
    ///
    /// Expected column order: id, execution_id, role, content, created_at,
    /// token_count, tool_calls, tool_results, session_id, tool_call_id
    fn row_to_message(row: &rusqlite::Row) -> Result<Message, rusqlite::Error> {
        Ok(Message {
            id: row.get(0)?,
            execution_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            token_count: row.get(5)?,
            tool_calls: row.get(6)?,
            tool_results: row.get(7)?,
            session_id: row.get(8)?,
            tool_call_id: row.get(9)?,
        })
    }

    // =========================================================================
    // `ConversationStore` trait impl helpers
    // =========================================================================
    // Trait impl itself is below the `impl ConversationRepository` block;
    // see TD-021 in `memory-bank/tech-debt.md`. Forwarding only — the
    // existing inherent methods stay the canonical surface.

    /// Parse stored tool calls JSON into ToolCall format.
    ///
    /// Our stored format: [{"tool_id": "...", "tool_name": "...", "args": {...}, "result": "...", "error": null}]
    /// LLM format: [{"id": "...", "name": "...", "arguments": {...}}]
    fn parse_tool_calls_json(&self, json_str: &str) -> Option<Vec<agent_runtime::types::ToolCall>> {
        // Parse our stored format
        let stored: Vec<serde_json::Value> = serde_json::from_str(json_str).ok()?;

        let tool_calls: Vec<agent_runtime::types::ToolCall> = stored
            .into_iter()
            .filter_map(|v| {
                let tool_id = v.get("tool_id")?.as_str()?.to_string();
                let tool_name = v.get("tool_name")?.as_str()?.to_string();
                let args = v.get("args")?.clone();

                Some(agent_runtime::types::ToolCall::new(
                    tool_id, tool_name, args,
                ))
            })
            .collect();

        if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        }
    }
}

// =====================================================================
// `ConversationStore` trait impl (TD-021 hygiene)
// =====================================================================
//
// Forwards to the existing inherent methods. Surface intentionally
// narrow — see `stores/zero-stores-traits/src/conversation.rs` for
// rationale. No consumer is expected to migrate to
// `Arc<dyn ConversationStore>` as part of this scaffold; see
// `memory-bank/tech-debt.md` TD-021.

impl zero_stores_traits::ConversationStore for ConversationRepository {
    fn get_session_ward_id(&self, session_id: &str) -> Result<Option<String>, String> {
        ConversationRepository::get_session_ward_id(self, session_id)
    }

    fn get_session_agent_id(&self, session_id: &str) -> Result<Option<String>, String> {
        ConversationRepository::get_session_agent_id(self, session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test database using tempfile
    fn create_test_db() -> Arc<DatabaseManager> {
        use gateway_services::VaultPaths;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let paths = Arc::new(VaultPaths::new(temp_dir.path().to_path_buf()));
        let _ = temp_dir.keep(); // Prevent cleanup during test
        let db = DatabaseManager::new(paths).unwrap();
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

    // ========================================================================
    // Session conversation tests
    // ========================================================================

    #[test]
    fn test_append_session_message_user() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");

        let msg = repo
            .append_session_message("session-1", "exec-1", "user", "Hello world", None, None)
            .unwrap();

        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello world");
        assert_eq!(msg.session_id, Some("session-1".to_string()));
        assert_eq!(msg.execution_id, Some("exec-1".to_string()));
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn test_append_session_message_tool_result() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");

        let msg = repo
            .append_session_message(
                "session-1",
                "exec-1",
                "tool",
                "file created at /tmp/output.txt",
                None,
                Some("call_abc123"),
            )
            .unwrap();

        assert_eq!(msg.role, "tool");
        assert_eq!(msg.tool_call_id, Some("call_abc123".to_string()));
    }

    #[test]
    fn test_get_session_conversation_returns_all_types() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");

        // Simulate a full conversation flow
        repo.append_session_message("session-1", "exec-1", "user", "build a docx", None, None)
            .unwrap();

        let tc_json =
            r#"[{"tool_id":"call_1","tool_name":"shell","args":{"cmd":"pip install docx"}}]"#;
        repo.append_session_message(
            "session-1",
            "exec-1",
            "assistant",
            "[tool calls]",
            Some(tc_json),
            None,
        )
        .unwrap();

        repo.append_session_message(
            "session-1",
            "exec-1",
            "tool",
            "Successfully installed python-docx",
            None,
            Some("call_1"),
        )
        .unwrap();

        repo.append_session_message(
            "session-1",
            "exec-1",
            "assistant",
            "Done! Created the docx file.",
            None,
            None,
        )
        .unwrap();

        let messages = repo.get_session_conversation("session-1", 100).unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1].tool_calls.is_some());
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[2].tool_call_id, Some("call_1".to_string()));
        assert_eq!(messages[3].role, "assistant");
    }

    #[test]
    fn test_session_messages_to_chat_format_with_tool_call_id() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");

        repo.append_session_message("session-1", "exec-1", "user", "Hello", None, None)
            .unwrap();
        repo.append_session_message(
            "session-1",
            "exec-1",
            "tool",
            "Result data",
            None,
            Some("call_xyz"),
        )
        .unwrap();

        let messages = repo.get_session_conversation("session-1", 100).unwrap();
        let chat_messages = repo.session_messages_to_chat_format(&messages);

        assert_eq!(chat_messages.len(), 2);
        assert_eq!(chat_messages[0].role, "user");
        assert!(chat_messages[0].tool_call_id.is_none());
        assert_eq!(chat_messages[1].role, "tool");
        assert_eq!(chat_messages[1].tool_call_id, Some("call_xyz".to_string()));
    }

    #[test]
    fn test_get_session_conversation_respects_limit() {
        let db = create_test_db();
        let repo = ConversationRepository::new(db.clone());

        create_test_session(&db, "session-1", "root-agent");
        create_test_execution(&db, "exec-1", "session-1", "root-agent", "root");

        for i in 1..=10 {
            repo.append_session_message(
                "session-1",
                "exec-1",
                "user",
                &format!("Message {}", i),
                None,
                None,
            )
            .unwrap();
        }

        let messages = repo.get_session_conversation("session-1", 3).unwrap();
        assert_eq!(messages.len(), 3);
        // Should be the most recent 3, in chronological order
        assert_eq!(messages[0].content, "Message 8");
        assert_eq!(messages[1].content, "Message 9");
        assert_eq!(messages[2].content, "Message 10");
    }
}
