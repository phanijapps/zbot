// ============================================================================
// MESSAGE REPOSITORY
// CRUD operations for messages
// ============================================================================

use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Message role (user/assistant/system/tool)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(MessageRole::User),
            "assistant" => Some(MessageRole::Assistant),
            "system" => Some(MessageRole::System),
            "tool" => Some(MessageRole::Tool),
            _ => None,
        }
    }
}

/// Tool call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Tool result information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub output: String,
    pub error: Option<String>,
}

/// Message data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
    pub token_count: i64,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,
}

/// Create message request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub token_count: Option<i64>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,
}

/// Create a new message
pub fn create_message(conn: &Connection, req: CreateMessage) -> Result<Message> {
    let now = Utc::now().to_rfc3339();
    let token_count = req.token_count.unwrap_or(0);
    let tool_calls_json = req.tool_calls.as_ref()
        .and_then(|t| serde_json::to_string(t).ok());
    let tool_results_json = req.tool_results.as_ref()
        .and_then(|t| serde_json::to_string(t).ok());

    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count, tool_calls, tool_results)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &req.id,
            &req.conversation_id,
            req.role.as_str(),
            &req.content,
            &now,
            &token_count,
            &tool_calls_json,
            &tool_results_json,
        ],
    )?;

    Ok(Message {
        id: req.id,
        conversation_id: req.conversation_id,
        role: req.role,
        content: req.content,
        created_at: now,
        token_count,
        tool_calls: req.tool_calls,
        tool_results: req.tool_results,
    })
}

/// Get a message by ID
pub fn get_message(conn: &Connection, id: &str) -> Result<Option<Message>> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, created_at, token_count, tool_calls, tool_results
         FROM messages WHERE id = ?1"
    )?;

    let result = stmt.query_row(params![id], |row| {
        let role_str: String = row.get(2)?;
        let role = MessageRole::from_str(&role_str).unwrap_or(MessageRole::User);

        let tool_calls_str: Option<String> = row.get(6)?;
        let tool_calls = tool_calls_str.and_then(|s| serde_json::from_str(&s).ok());

        let tool_results_str: Option<String> = row.get(7)?;
        let tool_results = tool_results_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role,
            content: row.get(3)?,
            created_at: row.get(4)?,
            token_count: row.get(5)?,
            tool_calls,
            tool_results,
        })
    });

    match result {
        Ok(msg) => Ok(Some(msg)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// List messages for a conversation
pub fn list_messages(
    conn: &Connection,
    conversation_id: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<Message>> {
    let query = "SELECT id, conversation_id, role, content, created_at, token_count, tool_calls, tool_results
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at ASC
                 LIMIT ?2 OFFSET ?3";

    let mut stmt = conn.prepare(query)?;

    let limit = limit.unwrap_or(100);
    let offset = offset.unwrap_or(0);

    let rows = stmt.query_map(params![conversation_id, limit, offset], |row| {
        let role_str: String = row.get(2)?;
        let role = MessageRole::from_str(&role_str).unwrap_or(MessageRole::User);

        let tool_calls_str: Option<String> = row.get(6)?;
        let tool_calls = tool_calls_str.and_then(|s| serde_json::from_str(&s).ok());

        let tool_results_str: Option<String> = row.get(7)?;
        let tool_results = tool_results_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role,
            content: row.get(3)?,
            created_at: row.get(4)?,
            token_count: row.get(5)?,
            tool_calls,
            tool_results,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row?);
    }

    Ok(messages)
}

/// Get recent messages (for context window)
pub fn get_recent_messages(
    conn: &Connection,
    conversation_id: &str,
    max_tokens: Option<i64>,
) -> Result<Vec<Message>> {
    let max_tokens = max_tokens.unwrap_or(4000);

    // Get all messages for the conversation
    let all_messages = list_messages(conn, conversation_id, None, None)?;

    // Filter messages starting from the most recent and adding until we hit max_tokens
    let mut result = Vec::new();
    let mut current_tokens = 0i64;

    for msg in all_messages.into_iter().rev() {
        if current_tokens + msg.token_count > max_tokens && !result.is_empty() {
            break;
        }
        current_tokens += msg.token_count;
        result.push(msg);
    }

    result.reverse();
    Ok(result)
}

/// Count messages in a conversation
pub fn count_messages(conn: &Connection, conversation_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
        params![conversation_id],
        |row| row.get(0),
    )
}

/// Get total token count for a conversation
pub fn get_token_count(conn: &Connection, conversation_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(SUM(token_count), 0) FROM messages WHERE conversation_id = ?1",
        params![conversation_id],
        |row| row.get(0),
    )
}

/// Delete a message
pub fn delete_message(conn: &Connection, id: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "DELETE FROM messages WHERE id = ?1",
        params![id],
    )?;

    Ok(rows_affected > 0)
}

/// Delete all messages in a conversation
pub fn delete_conversation_messages(conn: &Connection, conversation_id: &str) -> Result<usize> {
    conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1",
        params![conversation_id],
    )
}
