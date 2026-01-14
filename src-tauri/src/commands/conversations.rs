// ============================================================================
// CONVERSATIONS COMMANDS
// Chat conversation management
// ============================================================================

use serde_json::Value;

/// Lists all conversations
#[tauri::command]
pub async fn list_conversations() -> Result<Vec<Value>, String> {
    // TODO: Implement storage layer
    Ok(vec![])
}

/// Gets a single conversation by ID
#[tauri::command]
pub async fn get_conversation(id: String) -> Result<Value, String> {
    // TODO: Implement storage layer
    Err(format!("Conversation not found: {}", id))
}

/// Creates a new conversation
#[tauri::command]
pub async fn create_conversation(conversation: Value) -> Result<Value, String> {
    // TODO: Implement storage layer
    Err("Not implemented".to_string())
}

/// Updates an existing conversation
#[tauri::command]
pub async fn update_conversation(id: String, conversation: Value) -> Result<Value, String> {
    // TODO: Implement storage layer
    Err("Not implemented".to_string())
}

/// Deletes a conversation
#[tauri::command]
pub async fn delete_conversation(id: String) -> Result<(), String> {
    // TODO: Implement storage layer
    Err("Not implemented".to_string())
}
