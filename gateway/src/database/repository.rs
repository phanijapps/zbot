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
