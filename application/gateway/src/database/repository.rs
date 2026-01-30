// ============================================================================
// CONVERSATION REPOSITORY
// CRUD operations for conversations and messages
// ============================================================================

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::DatabaseManager;

// ============================================================================
// TYPES
// ============================================================================

/// A conversation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub agent_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<String>,
}

/// A message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
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

/// Repository for conversation and message operations
pub struct ConversationRepository {
    db: Arc<DatabaseManager>,
}

impl ConversationRepository {
    /// Create a new repository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    // =========================================================================
    // CONVERSATION OPERATIONS
    // =========================================================================

    /// Get or create a conversation
    pub fn get_or_create_conversation(
        &self,
        conversation_id: &str,
        agent_id: &str,
    ) -> Result<Conversation, String> {
        // First try to get existing
        if let Ok(conv) = self.get_conversation(conversation_id) {
            return Ok(conv);
        }

        // Create new conversation
        let now = Utc::now().to_rfc3339();
        let conversation = Conversation {
            id: conversation_id.to_string(),
            agent_id: agent_id.to_string(),
            title: None,
            created_at: now.clone(),
            updated_at: now,
            metadata: None,
        };

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO conversations (id, agent_id, title, created_at, updated_at, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    conversation.id,
                    conversation.agent_id,
                    conversation.title,
                    conversation.created_at,
                    conversation.updated_at,
                    conversation.metadata,
                ],
            )
        })?;

        Ok(conversation)
    }

    /// Get a conversation by ID
    pub fn get_conversation(&self, conversation_id: &str) -> Result<Conversation, String> {
        self.db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, agent_id, title, created_at, updated_at, metadata
                 FROM conversations WHERE id = ?1",
                [conversation_id],
                |row| {
                    Ok(Conversation {
                        id: row.get(0)?,
                        agent_id: row.get(1)?,
                        title: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        metadata: row.get(5)?,
                    })
                },
            )
        })
    }

    /// List conversations for an agent
    pub fn list_conversations(&self, agent_id: &str, limit: usize) -> Result<Vec<Conversation>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, agent_id, title, created_at, updated_at, metadata
                 FROM conversations
                 WHERE agent_id = ?1
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![agent_id, limit as i64], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    metadata: row.get(5)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    /// Update conversation title
    pub fn update_conversation_title(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, conversation_id],
            )?;
            Ok(())
        })
    }

    /// Delete a conversation and its messages
    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), String> {
        self.db.with_connection(|conn| {
            conn.execute(
                "DELETE FROM conversations WHERE id = ?1",
                [conversation_id],
            )?;
            Ok(())
        })
    }

    // =========================================================================
    // MESSAGE OPERATIONS
    // =========================================================================

    /// Add a message to a conversation
    pub fn add_message(
        &self,
        conversation_id: &str,
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
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now.clone(),
            token_count,
            tool_calls: tool_calls.map(String::from),
            tool_results: tool_results.map(String::from),
        };

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count, tool_calls, tool_results)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    message.id,
                    message.conversation_id,
                    message.role,
                    message.content,
                    message.created_at,
                    message.token_count,
                    message.tool_calls,
                    message.tool_results,
                ],
            )?;

            // Update conversation updated_at
            conn.execute(
                "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                params![now, conversation_id],
            )?;

            Ok(())
        })?;

        Ok(message)
    }

    /// Get messages for a conversation
    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, role, content, created_at, token_count, tool_calls, tool_results
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at ASC",
            )?;

            let rows = stmt.query_map([conversation_id], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
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
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, role, content, created_at, token_count, tool_calls, tool_results
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
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
