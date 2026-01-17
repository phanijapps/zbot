// ============================================================================
// CONVERSATION REPOSITORY
// CRUD operations for conversations
// ============================================================================

use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Conversation data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub metadata: Option<serde_json::Value>,
}

/// Create conversation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversation {
    pub id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    pub title: String,
    pub metadata: Option<serde_json::Value>,
}

/// Update conversation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConversation {
    pub title: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Create a new conversation
pub fn create_conversation(conn: &Connection, req: CreateConversation) -> Result<Conversation> {
    let now = Utc::now().to_rfc3339();
    let metadata_json = req.metadata.as_ref()
        .and_then(|m| serde_json::to_string(m).ok());

    conn.execute(
        "INSERT INTO conversations (id, agent_id, title, created_at, updated_at, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &req.id,
            &req.agent_id,
            &req.title,
            &now,
            &now,
            &metadata_json,
        ],
    )?;

    Ok(Conversation {
        id: req.id,
        agent_id: req.agent_id,
        title: req.title,
        created_at: now.clone(),
        updated_at: now,
        metadata: req.metadata,
    })
}

/// Get a conversation by ID
pub fn get_conversation(conn: &Connection, id: &str) -> Result<Option<Conversation>> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, title, created_at, updated_at, metadata
         FROM conversations WHERE id = ?1"
    )?;

    let result = stmt.query_row(params![id], |row| {
        let metadata_str: Option<String> = row.get(5)?;
        let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Conversation {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            title: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
            metadata,
        })
    });

    match result {
        Ok(conv) => Ok(Some(conv)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// List all conversations for an agent
pub fn list_conversations(conn: &Connection, agent_id: Option<&str>) -> Result<Vec<Conversation>> {
    let query = if agent_id.is_some() {
        "SELECT id, agent_id, title, created_at, updated_at, metadata
         FROM conversations WHERE agent_id = ?1
         ORDER BY updated_at DESC"
    } else {
        "SELECT id, agent_id, title, created_at, updated_at, metadata
         FROM conversations
         ORDER BY updated_at DESC"
    };

    let mut stmt = conn.prepare(query)?;

    let mut conversations = Vec::new();

    let row_mapper = |row: &rusqlite::Row| -> Result<Conversation, rusqlite::Error> {
        let metadata_str: Option<String> = row.get(5)?;
        let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Conversation {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            title: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
            metadata,
        })
    };

    let rows = if let Some(agent) = agent_id {
        stmt.query_map(params![agent], row_mapper)?
    } else {
        stmt.query_map([], row_mapper)?
    };

    for row in rows {
        conversations.push(row?);
    }

    Ok(conversations)
}

/// Update a conversation
pub fn update_conversation(
    conn: &Connection,
    id: &str,
    req: UpdateConversation,
) -> Result<Option<Conversation>> {
    let now = Utc::now().to_rfc3339();

    // Check if conversation exists
    let existing = get_conversation(conn, id)?;

    if existing.is_none() {
        return Ok(None);
    }

    // Update title if provided
    if let Some(ref title) = req.title {
        conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )?;
    }

    // Update metadata if provided
    if let Some(ref metadata) = req.metadata {
        let metadata_json = serde_json::to_string(metadata).unwrap_or_default();
        conn.execute(
            "UPDATE conversations SET metadata = ?1, updated_at = ?2 WHERE id = ?3",
            params![metadata_json, now, id],
        )?;
    }

    get_conversation(conn, id)
}

/// Delete a conversation (cascades to messages)
pub fn delete_conversation(conn: &Connection, id: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "DELETE FROM conversations WHERE id = ?1",
        params![id],
    )?;

    Ok(rows_affected > 0)
}

/// Count conversations for an agent
pub fn count_conversations(conn: &Connection, agent_id: Option<&str>) -> Result<i64> {
    let query = if agent_id.is_some() {
        "SELECT COUNT(*) FROM conversations WHERE agent_id = ?1"
    } else {
        "SELECT COUNT(*) FROM conversations"
    };

    let mut stmt = conn.prepare(query)?;

    if let Some(agent) = agent_id {
        stmt.query_row(params![agent], |row| row.get(0))
    } else {
        stmt.query_row([], |row| row.get(0))
    }
}
