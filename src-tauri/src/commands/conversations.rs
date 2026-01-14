// ============================================================================
// CONVERSATIONS COMMANDS
// Chat conversation management
// ============================================================================

use serde_json::Value;
use crate::domains::conversation_runtime::{get_database, repository};

/// Lists all conversations, optionally filtered by agent_id
#[tauri::command]
pub async fn list_conversations(agent_id: Option<String>) -> Result<Vec<Value>, String> {
    let db = get_database()?;
    let agent_id_ref = agent_id.as_deref();

    let conversations = db.transaction(|conn| {
        repository::list_conversations(conn, agent_id_ref)
    })
    .map_err(|e| format!("Failed to list conversations: {}", e))?;

    serde_json::to_value(&conversations)
        .map_err(|e| format!("Failed to serialize conversations: {}", e))
        .map(|v| v.as_array().cloned().unwrap_or_default())
}

/// Gets a single conversation by ID with its messages
#[tauri::command]
pub async fn get_conversation(id: String) -> Result<Value, String> {
    let db = get_database()?;

    let (conv, messages, token_count) = db.transaction(|conn| {
        let conversation = repository::get_conversation(conn, &id)?;

        match conversation {
            Some(conv) => {
                let messages = repository::list_messages(conn, &id, None, None)?;
                let token_count = repository::get_token_count(conn, &id)?;
                Ok((Some(conv), messages, token_count))
            }
            None => Ok((None, vec![], 0))
        }
    })
    .map_err(|e| format!("Database error: {}", e))?;

    match conv {
        Some(c) => {
            let mut result = serde_json::to_value(&c)
                .map_err(|e| format!("Failed to serialize conversation: {}", e))?;

            if let Some(obj) = result.as_object_mut() {
                obj.insert("messages".to_string(), serde_json::to_value(&messages).unwrap());
                obj.insert("token_count".to_string(), serde_json::json!(token_count));
            }

            Ok(result)
        }
        None => Err(format!("Conversation not found: {}", id))
    }
}

/// Creates a new conversation
#[tauri::command]
pub async fn create_conversation(data: Value) -> Result<Value, String> {
    let db = get_database()?;

    let req: repository::CreateConversation = serde_json::from_value(data)
        .map_err(|e| format!("Invalid conversation data: {}", e))?;

    let conv = db.transaction(|conn| {
        repository::create_conversation(conn, req)
    })
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    serde_json::to_value(&conv)
        .map_err(|e| format!("Failed to serialize conversation: {}", e))
}

/// Updates an existing conversation
#[tauri::command]
pub async fn update_conversation(id: String, data: Value) -> Result<Value, String> {
    let db = get_database()?;

    let req: repository::UpdateConversation = serde_json::from_value(data)
        .map_err(|e| format!("Invalid update data: {}", e))?;

    let conv = db.transaction(|conn| {
        repository::update_conversation(conn, &id, req)
    })
    .map_err(|e| format!("Failed to update conversation: {}", e))?;

    match conv {
        Some(updated) => {
            serde_json::to_value(&updated)
                .map_err(|e| format!("Failed to serialize conversation: {}", e))
        }
        None => Err(format!("Conversation not found: {}", id))
    }
}

/// Deletes a conversation and all its messages
#[tauri::command]
pub async fn delete_conversation(id: String) -> Result<(), String> {
    let db = get_database()?;

    db.transaction(|conn| {
        repository::delete_conversation(conn, &id)
    })
    .map_err(|e| format!("Failed to delete conversation: {}", e))?;

    Ok(())
}

/// Lists messages for a conversation
#[tauri::command]
pub async fn list_messages(conversation_id: String, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<Value>, String> {
    let db = get_database()?;

    let messages = db.transaction(|conn| {
        repository::list_messages(conn, &conversation_id, limit, offset)
    })
    .map_err(|e| format!("Failed to list messages: {}", e))?;

    serde_json::to_value(&messages)
        .map_err(|e| format!("Failed to serialize messages: {}", e))
        .map(|v| v.as_array().cloned().unwrap_or_default())
}

/// Creates a new message in a conversation
#[tauri::command]
pub async fn create_message(data: Value) -> Result<Value, String> {
    let db = get_database()?;

    let req: repository::CreateMessage = serde_json::from_value(data.clone())
        .map_err(|e| format!("Invalid message data: {}", e))?;

    // Save conversation_id before moving req
    let conversation_id = req.conversation_id.clone();

    let msg = db.transaction(|conn| {
        // Verify conversation exists
        let conv = repository::get_conversation(conn, &conversation_id)?;
        if conv.is_none() {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let msg = repository::create_message(conn, req)?;

        // Update conversation's updated_at timestamp
        conn.execute(
            "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![&conversation_id]
        )?;

        Ok(msg)
    })
    .map_err(|e| format!("Failed to create message: {}", e))?;

    serde_json::to_value(&msg)
        .map_err(|e| format!("Failed to serialize message: {}", e))
}

/// Gets a single message by ID
#[tauri::command]
pub async fn get_message(id: String) -> Result<Value, String> {
    let db = get_database()?;

    let msg = db.transaction(|conn| {
        repository::get_message(conn, &id)
    })
    .map_err(|e| format!("Failed to get message: {}", e))?;

    match msg {
        Some(m) => {
            serde_json::to_value(&m)
                .map_err(|e| format!("Failed to serialize message: {}", e))
        }
        None => Err(format!("Message not found: {}", id))
    }
}

/// Deletes a message
#[tauri::command]
pub async fn delete_message(id: String) -> Result<bool, String> {
    let db = get_database()?;

    db.transaction(|conn| {
        repository::delete_message(conn, &id)
    })
    .map_err(|e| format!("Failed to delete message: {}", e))
}

/// Gets conversation statistics
#[tauri::command]
pub async fn get_conversation_stats(conversation_id: String) -> Result<Value, String> {
    let db = get_database()?;

    let (message_count, token_count) = db.transaction(|conn| {
        let mc = repository::count_messages(conn, &conversation_id)?;
        let tc = repository::get_token_count(conn, &conversation_id)?;
        Ok((mc, tc))
    })
    .map_err(|e| format!("Failed to get stats: {}", e))?;

    Ok(serde_json::json!({
        "message_count": message_count,
        "token_count": token_count,
    }))
}
