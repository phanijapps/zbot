// ============================================================================
// GLOBAL EVENT EMITTER
// Provides event emission capability from anywhere in the app
// ============================================================================

use once_cell::sync::OnceCell;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;

/// Global app handle for event emission
static APP_HANDLE: OnceCell<Arc<RwLock<Option<AppHandle>>>> = OnceCell::new();

/// Initialize the global event emitter with the app handle
/// Call this in the Tauri setup function
pub fn init(app: &AppHandle) {
    let handle = APP_HANDLE.get_or_init(|| Arc::new(RwLock::new(None)));
    if let Ok(mut guard) = handle.try_write() {
        *guard = Some(app.clone());
        tracing::info!("Global event emitter initialized");
    }
}

/// Emit an event to the frontend
/// Returns Ok(()) if emitted successfully, Err if app handle not available
pub async fn emit<S: Serialize + Clone>(event: &str, payload: S) -> Result<(), String> {
    let handle = APP_HANDLE.get()
        .ok_or_else(|| "Event emitter not initialized".to_string())?;

    let guard = handle.read().await;
    let app = guard.as_ref()
        .ok_or_else(|| "App handle not available".to_string())?;

    app.emit(event, payload)
        .map_err(|e| format!("Failed to emit event: {}", e))
}

/// Emit an event synchronously (blocking)
/// Use sparingly - prefer async emit when possible
pub fn emit_sync<S: Serialize + Clone>(event: &str, payload: S) -> Result<(), String> {
    let handle = APP_HANDLE.get()
        .ok_or_else(|| "Event emitter not initialized".to_string())?;

    // Try to get read lock without blocking for too long
    let guard = handle.try_read()
        .map_err(|_| "Could not acquire read lock".to_string())?;

    let app = guard.as_ref()
        .ok_or_else(|| "App handle not available".to_string())?;

    app.emit(event, payload)
        .map_err(|e| format!("Failed to emit event: {}", e))
}

/// Check if the event emitter is initialized
pub fn is_initialized() -> bool {
    APP_HANDLE.get()
        .and_then(|h| h.try_read().ok())
        .map(|g| g.is_some())
        .unwrap_or(false)
}

// ============================================================================
// ACTIVITY EVENT TYPES
// ============================================================================

/// Activity item types
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Todo,
    ToolCall,
    SubagentStart,
    SubagentEnd,
}

/// Tool call status
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Success,
    Error,
}

/// A tool call record for activity tracking
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallActivity {
    pub id: String,
    pub name: String,
    pub status: ToolStatus,
    pub duration_ms: Option<u64>,
    pub arguments_preview: Option<String>,
    pub result_preview: Option<String>,
    pub error: Option<String>,
}

/// An activity item (tool call or todo)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub is_orchestrator: bool,
    pub item_type: ActivityType,
    pub timestamp: String,
    pub tool_call: Option<ToolCallActivity>,
}

/// Emit a subagent event to the parent's channel
pub async fn emit_subagent_event(
    parent_session_id: &str,
    subagent_id: &str,
    subagent_name: &str,
    event_type: &str,
    payload: Value,
) -> Result<(), String> {
    let event_name = format!("agent-stream://{}", parent_session_id);

    let mut enriched_payload = payload;
    if let Some(obj) = enriched_payload.as_object_mut() {
        obj.insert("subagentId".to_string(), Value::String(subagent_id.to_string()));
        obj.insert("subagentName".to_string(), Value::String(subagent_name.to_string()));
        obj.insert("isSubagent".to_string(), Value::Bool(true));
    }

    emit(&event_name, enriched_payload).await
}
