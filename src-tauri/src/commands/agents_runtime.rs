// ============================================================================
// AGENT RUNTIME COMMANDS
// Commands for executing AI agents with streaming support
// ============================================================================

use serde_json::Value;
use crate::domains::conversation_runtime::{get_database, repository};

/// Execute an agent with streaming support
///
/// This command handles:
/// 1. Loading agent configuration and instructions
/// 2. Managing conversation history
/// 3. Streaming events back to the frontend
///
/// The actual LangChain execution happens in the frontend (renderer process)
/// because LangChain runs in Node.js, not Rust.
#[tauri::command]
pub async fn execute_agent_stream(
    conversation_id: String,
    agent_id: String,
    _message: String,
) -> Result<Value, String> {
    // Verify conversation exists
    let db = get_database()?;

    let conversation_exists = db.transaction(|conn| {
        repository::get_conversation(conn, &conversation_id)
    })
    .map_err(|e| format!("Database error: {}", e))?;

    if conversation_exists.is_none() {
        return Err(format!("Conversation not found: {}", conversation_id));
    }

    // Return configuration for frontend to execute agent
    // This includes agent config and conversation history
    let (messages, _agent) = db.transaction(|conn| {
        let msgs = repository::list_messages(conn, &conversation_id, None, None)?;
        let ag = repository::get_conversation(conn, &conversation_id)?;

        // Load agent details
        let agent_details = if let Some(conv) = ag {
            Some(conv)
        } else {
            None
        };

        Ok((msgs, agent_details))
    })
    .map_err(|e| format!("Failed to load data: {}", e))?;

    // Convert messages to frontend format
    let history_json: Vec<Value> = messages.into_iter().map(|msg| {
        serde_json::json!({
            "id": msg.id,
            "conversation_id": msg.conversation_id,
            "role": msg.role.as_str(),
            "content": msg.content,
            "created_at": msg.created_at,
            "tool_calls": msg.tool_calls,
            "tool_results": msg.tool_results,
        })
    }).collect();

    Ok(serde_json::json!({
        "conversation_id": conversation_id,
        "agent_id": agent_id,
        "history": history_json,
        "ready": true
    }))
}

/// Save a streaming event to the conversation
///
/// Called by frontend as events arrive during agent execution
#[tauri::command]
pub async fn save_stream_event(
    _conversation_id: String,
    _event: Value,
) -> Result<(), String> {
    // Events are handled in frontend for real-time display
    // This is just a placeholder for future logging/persistence
    Ok(())
}

/// Get agent execution configuration
///
/// Returns all necessary config for frontend to execute agent
#[tauri::command]
pub async fn get_agent_execution_config(
    agent_id: String,
) -> Result<Value, String> {
    // This would load from agents/{agent_id}/config.yaml
    // For now, return a basic structure
    Ok(serde_json::json!({
        "agent_id": agent_id,
        "config_loaded": false
    }))
}

/// Create a new conversation for an agent
#[tauri::command]
pub async fn create_agent_conversation(
    agent_id: String,
    title: Option<String>,
) -> Result<Value, String> {
    let db = get_database()?;

    let conversation_id = format!("conv_{}_{}", agent_id, chrono::Utc::now().timestamp());
    let agent_id_clone = agent_id.clone();

    let conv = db.transaction(|conn| {
        repository::create_conversation(conn, repository::CreateConversation {
            id: conversation_id.clone(),
            agent_id,
            title: title.unwrap_or_else(|| format!("Chat with {}", agent_id_clone)),
            metadata: None,
        })
    })
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    serde_json::to_value(&conv)
        .map_err(|e| format!("Failed to serialize: {}", e))
}

/// Get or create a conversation for an agent
#[tauri::command]
pub async fn get_or_create_conversation(
    agent_id: String,
    conversation_id: Option<String>,
) -> Result<Value, String> {
    let db = get_database()?;

    if let Some(conv_id) = conversation_id {
        // Try to get existing conversation
        let conv = db.transaction(|conn| {
            repository::get_conversation(conn, &conv_id)
        })
        .map_err(|e| format!("Database error: {}", e))?;

        if let Some(c) = conv {
            return serde_json::to_value(&c)
                .map_err(|e| format!("Failed to serialize: {}", e));
        }
    }

    // Create new conversation
    let agent_id_clone = agent_id.clone();
    let conv = db.transaction(|conn| {
        repository::create_conversation(conn, repository::CreateConversation {
            id: format!("conv_{}_{}", agent_id, chrono::Utc::now().timestamp()),
            agent_id,
            title: format!("Chat with {}", agent_id_clone),
            metadata: None,
        })
    })
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    serde_json::to_value(&conv)
        .map_err(|e| format!("Failed to serialize: {}", e))
}
