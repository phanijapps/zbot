// ============================================================================
// AGENT RUNTIME COMMANDS
// Commands for executing AI agents with streaming support (Agent Channel model)
// ============================================================================

use std::sync::Arc;
use std::collections::HashMap;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::domains::conversation_runtime::repository::{MessageRole, ToolCall, ToolResult};
use crate::domains::agent_runtime::executor_v2::{create_zero_executor, ZeroAppStreamEvent};
use daily_sessions::{DailySessionManager, DailySessionRepository};
use crate::commands::agent_channels::SqliteSessionRepository;
use crate::settings::AppDirs;

// ============================================================================
// EXECUTOR CACHE
// ============================================================================

/// Cache key for executors (agent_id, session_id)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    agent_id: String,
    session_id: String,
}

/// Global executor cache
lazy_static::lazy_static! {
    static ref EXECUTOR_CACHE: Arc<RwLock<HashMap<CacheKey, Arc<crate::domains::agent_runtime::executor_v2::ZeroAppExecutor>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

/// Remove a specific agent from the executor cache
pub async fn invalidate_executor_cache(agent_id: &str) {
    let mut cache = EXECUTOR_CACHE.write().await;
    // Remove all cached executors for this agent
    cache.retain(|key, _| key.agent_id != agent_id);
}

/// Clear the entire executor cache
pub async fn clear_executor_cache_internal() {
    let mut cache = EXECUTOR_CACHE.write().await;
    cache.clear();
}

/// Execute an agent with streaming support
///
/// This command:
/// 1. Gets or creates today's session for the agent
/// 2. Loads agent configuration
/// 3. Creates/reuses an executor
/// 4. Executes the agent with tool calling
/// 5. Emits events to frontend in real-time
/// 6. Saves messages to the daily session database
#[tauri::command]
pub async fn execute_agent_stream(
    app: AppHandle,
    agent_id: String,
    message: String,
) -> Result<Value, String> {
    // Get or create today's session for this agent
    let db_path = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();

    let repo = Arc::new(SqliteSessionRepository::new(db_path)?);
    let manager = DailySessionManager::new(repo.clone());

    let session = manager.get_or_create_today(&agent_id).await
        .map_err(|e| format!("Failed to get session: {}", e))?;

    let session_id = session.id.clone();

    // Record user message
    let user_message = daily_sessions::SessionMessage {
        id: format!("msg_{}", chrono::Utc::now().timestamp_millis()),
        session_id: session_id.clone(),
        role: "user".to_string(),
        content: message.clone(),
        created_at: chrono::Utc::now(),
        token_count: 0,
        tool_calls: None,
        tool_results: None,
    };
    manager.record_message(&session_id, user_message.clone()).await
        .map_err(|e| format!("Failed to record user message: {}", e))?;

    // Index user message for search (fire and forget)
    let db_path = AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();
    let session_id_clone = session_id.clone();
    let agent_id_clone = agent_id.clone();
    tokio::spawn(async move {
        let _ = index_message_from_db(&db_path, &session_id_clone, &agent_id_clone).await;
    });

    // Get or create zero-app executor from cache
    let cache_key = CacheKey {
        agent_id: agent_id.clone(),
        session_id: session_id.clone(),
    };

    let executor = {
        let cache_read = EXECUTOR_CACHE.read().await;
        cache_read.get(&cache_key).cloned()
    };

    let executor = if let Some(exec) = executor {
        exec
    } else {
        // Create new executor (cache miss)
        tracing::info!("Creating new executor for agent: {} (session: {})", agent_id, session_id);
        let exec = Arc::new(create_zero_executor(&agent_id, Some(session_id.clone()), None, None).await?);

        // Add to cache
        let mut cache_write = EXECUTOR_CACHE.write().await;
        cache_write.insert(cache_key, exec.clone());

        exec
    };

    // Clear any previous stop flag before starting new execution
    executor.clear_stop()?;

    // Execute with streaming
    let mut final_response = String::new();
    let mut current_tool_calls: Vec<ToolCall> = Vec::new();
    let mut current_tool_results: Vec<ToolResult> = Vec::new();

    executor.run_stream(message, |stream_event| {
        let event_name = format!("agent-stream://{}", session_id);

        match stream_event {
            ZeroAppStreamEvent::Content { delta } => {
                final_response.push_str(&delta);

                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "token",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "content": delta
                })) {
                    eprintln!("Failed to emit event to frontend: {}", e);
                }
            }
            ZeroAppStreamEvent::ToolCall { id, name, arguments } => {
                current_tool_calls.push(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: serde_json::from_str(&arguments).unwrap_or_default(),
                });

                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "tool_call_start",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "toolId": id,
                    "toolName": name,
                    "args": arguments
                })) {
                    eprintln!("Failed to emit event to frontend: {}", e);
                }
            }
            ZeroAppStreamEvent::ToolResponse { id, response } => {
                tracing::debug!("ToolResponse: toolId={}, response.len={}", id, response.len());

                // Add to tool results for database
                current_tool_results.push(ToolResult {
                    tool_call_id: id.clone(),
                    output: response.clone(),
                    error: None,
                });

                // Check for special UI markers in the response
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                    tracing::debug!("ToolResponse parsed: toolId={}, has___show_content={}, has___request_input={}",
                        id,
                        parsed.get("__show_content").is_some(),
                        parsed.get("__request_input").is_some());

                    // Check for show_content marker
                    if parsed.get("__show_content").and_then(|v| v.as_bool()).unwrap_or(false) {
                        tracing::debug!("Detected show_content marker, emitting show_content event");
                        if let Err(e) = app.emit(&event_name, serde_json::json!({
                            "type": "show_content",
                            "timestamp": chrono::Utc::now().timestamp_millis(),
                            "contentType": parsed.get("content_type").and_then(|v| v.as_str()),
                            "title": parsed.get("title").and_then(|v| v.as_str()),
                            "content": parsed.get("content").and_then(|v| v.as_str()),
                            "filePath": parsed.get("file_path").and_then(|v| v.as_str()),
                            "isAttachment": parsed.get("is_attachment").and_then(|v| v.as_bool()),
                            "base64": parsed.get("base64").and_then(|v| v.as_bool()),
                        })) {
                            eprintln!("Failed to emit show_content event to frontend: {}", e);
                        }
                        // Don't return - continue to emit tool_result so the UI knows the tool completed
                    }

                    // Check for request_input marker
                    if parsed.get("__request_input").and_then(|v| v.as_bool()).unwrap_or(false) {
                        tracing::info!("Detected request_input marker, emitting request_input event");
                        if let Err(e) = app.emit(&event_name, serde_json::json!({
                            "type": "request_input",
                            "timestamp": chrono::Utc::now().timestamp_millis(),
                            "toolId": id,
                            "formId": parsed.get("form_id").and_then(|v| v.as_str()),
                            "title": parsed.get("title").and_then(|v| v.as_str()),
                            "description": parsed.get("description").and_then(|v| v.as_str()),
                            "schema": parsed.get("schema"),
                            "submitButton": parsed.get("submit_button").and_then(|v| v.as_str()),
                        })) {
                            eprintln!("Failed to emit request_input event to frontend: {}", e);
                        }
                        // Don't return - continue to emit tool_result so the UI knows the tool completed
                    }

                    // Check for todo_update marker
                    if parsed.get("__todo_update").and_then(|v| v.as_bool()).unwrap_or(false) {
                        tracing::debug!("Detected todo_update marker, emitting todo_update event");
                        if let Err(e) = app.emit(&event_name, serde_json::json!({
                            "type": "todo_update",
                            "timestamp": chrono::Utc::now().timestamp_millis(),
                            "todos": parsed.get("todos"),
                        })) {
                            eprintln!("Failed to emit todo_update event to frontend: {}", e);
                        }
                        // Don't return - continue to emit tool_result so the UI knows the tool completed
                    }
                } else {
                    tracing::debug!("Failed to parse tool response as JSON: {}", response.chars().take(200).collect::<String>());
                }

                // Regular tool result - emit tool_result event
                tracing::debug!("Emitting regular tool_result: toolId={}, result.preview={}",
                    id, response.chars().take(200).collect::<String>());
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "tool_result",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "toolId": id,
                    "result": response
                })) {
                    eprintln!("Failed to emit event to frontend: {}", e);
                } else {
                    tracing::debug!("Successfully emitted tool_result event");
                }
            }
            ZeroAppStreamEvent::Complete { turn_complete } => {
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "done",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "finalMessage": final_response,
                    "tokenCount": final_response.len(),
                    "turnComplete": turn_complete
                })) {
                    eprintln!("Failed to emit event to frontend: {}", e);
                }
            }
            ZeroAppStreamEvent::Error { message } => {
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "error",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "error": message,
                    "recoverable": false
                })) {
                    eprintln!("Failed to emit event to frontend: {}", e);
                }
            }
            ZeroAppStreamEvent::IterationUpdate { current, max } => {
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "iteration_update",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "current": current,
                    "max": max
                })) {
                    eprintln!("Failed to emit iteration update event: {}", e);
                }
            }
            ZeroAppStreamEvent::ContinuationPrompt { iteration, message } => {
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "continuation_prompt",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "iteration": iteration,
                    "message": message
                })) {
                    eprintln!("Failed to emit continuation prompt event: {}", e);
                }
            }
            ZeroAppStreamEvent::Stopped { iteration, reason } => {
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "stopped",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "iteration": iteration,
                    "reason": reason
                })) {
                    eprintln!("Failed to emit stopped event: {}", e);
                }
            }
        }
    }).await.map_err(|e| format!("Agent execution failed: {}", e))?;

    // Save assistant response to session
    let tool_calls_json = if current_tool_calls.is_empty() {
        None
    } else {
        serde_json::to_value(&current_tool_calls).ok()
    };
    let tool_results_json = if current_tool_results.is_empty() {
        None
    } else {
        serde_json::to_value(&current_tool_results).ok()
    };

    manager.record_message(&session_id, daily_sessions::SessionMessage {
        id: format!("msg_{}", chrono::Utc::now().timestamp_millis()),
        session_id: session_id.clone(),
        role: "assistant".to_string(),
        content: final_response.clone(),
        created_at: chrono::Utc::now(),
        token_count: final_response.len() as i64,
        tool_calls: tool_calls_json,
        tool_results: tool_results_json,
    }).await.map_err(|e| format!("Failed to record assistant message: {}", e))?;

    // Index assistant message for search (fire and forget)
    let db_path = AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();
    let session_id_clone = session_id.clone();
    let agent_id_clone = agent_id.clone();
    tokio::spawn(async move {
        let _ = index_message_from_db(&db_path, &session_id_clone, &agent_id_clone).await;
    });

    Ok(serde_json::json!({
        "session_id": session_id,
        "agent_id": agent_id,
        "response": final_response,
        "tool_calls": current_tool_calls,
        "done": true
    }))
}

/// Get agent execution configuration
///
/// Returns agent config for display purposes
#[tauri::command]
pub async fn get_agent_execution_config(
    agent_id: String,
) -> Result<Value, String> {
    // Load agent config
    let dirs = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?;
    let agent_dir = dirs.config_dir.join("agents").join(&agent_id);
    let config_file = agent_dir.join("config.yaml");

    if !config_file.exists() {
        return Ok(serde_json::json!({
            "agent_id": agent_id,
            "config_loaded": false
        }));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read agent config: {}", e))?;

    let agent_config: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse agent config: {}", e))?;

    Ok(serde_json::json!({
        "agent_id": agent_id,
        "config_loaded": true,
        "provider_id": agent_config.get("providerId").and_then(|v| v.as_str()),
        "model": agent_config.get("model").and_then(|v| v.as_str()),
        "temperature": agent_config.get("temperature").and_then(|v| v.as_f64()),
        "max_tokens": agent_config.get("maxTokens").and_then(|v| v.as_u64()),
        "mcps": agent_config.get("mcps").and_then(|v| as_vec(v))
    }))
}

fn as_vec(value: &serde_yaml::Value) -> Option<Vec<String>> {
    value.as_sequence()?.iter().map(|v| v.as_str().map(|s| s.to_string())).collect()
}

/// DEPRECATED: Use get_or_create_today_session instead
/// Kept for backward compatibility during transition
#[tauri::command]
pub async fn create_agent_conversation(
    agent_id: String,
    _title: Option<String>,
) -> Result<Value, String> {
    // For now, create a new session (today's session will be used)
    let db_path = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();

    let repo = Arc::new(SqliteSessionRepository::new(db_path)?);
    let manager = DailySessionManager::new(repo);

    let session = manager.get_or_create_today(&agent_id).await
        .map_err(|e| format!("Failed to create session: {}", e))?;

    serde_json::to_value(&session)
        .map_err(|e| format!("Failed to serialize: {}", e))
}

/// DEPRECATED: Use get_or_create_today_session instead
/// Kept for backward compatibility during transition
#[tauri::command]
pub async fn get_or_create_conversation(
    agent_id: String,
    _conversation_id: Option<String>,
) -> Result<Value, String> {
    // The Agent Channel model doesn't use conversation_id
    // Always return today's session for the agent
    let db_path = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();

    let repo = Arc::new(SqliteSessionRepository::new(db_path)?);
    let manager = DailySessionManager::new(repo);

    let session = manager.get_or_create_today(&agent_id).await
        .map_err(|e| format!("Failed to get session: {}", e))?;

    // For backward compatibility, return in old format
    Ok(serde_json::json!({
        "id": session.id,
        "agentId": session.agent_id,
        "title": format!("{} - {}", session.agent_id, session.session_date),
        "createdAt": session.created_at.to_rfc3339(),
        "updatedAt": session.updated_at.to_rfc3339(),
        // Map session_id to conversation_id for frontend compatibility
        "conversation_id": session.id,
        "session_id": session.id,
        "session_date": session.session_date,
    }))
}

/// Clear executor cache (for testing/debugging)
#[tauri::command]
pub async fn clear_executor_cache() -> Result<(), String> {
    clear_executor_cache_internal().await;
    Ok(())
}

/// Stop agent execution
///
/// This command sets the stop flag in the agent's session state,
/// causing the agent loop to stop at the next iteration.
///
/// # Arguments
/// * `agent_id` - The ID of the agent to stop
///
/// # Returns
/// Ok(()) if the stop was requested successfully
/// Err if the agent was not found in cache (not currently executing)
#[tauri::command]
pub async fn stop_agent_execution(agent_id: String) -> Result<Value, String> {
    tracing::info!("Stop requested for agent: {}", agent_id);

    // Find the executor in cache
    let cache = EXECUTOR_CACHE.read().await;

    // Find any executor for this agent (regardless of session_id)
    let executor = cache.iter()
        .find(|(key, _)| key.agent_id == agent_id)
        .map(|(_, exec)| exec.clone());

    drop(cache); // Release the lock

    if let Some(executor) = executor {
        executor.request_stop()?;

        Ok(serde_json::json!({
            "success": true,
            "agent_id": agent_id,
            "message": "Stop requested - agent will stop at next iteration"
        }))
    } else {
        // Agent not in cache - may not be executing
        // Try to set stop flag in session state directly
        let db_path = crate::settings::AppDirs::get()
            .map_err(|e| format!("Failed to get app dirs: {}", e))?
            .agent_channels_db_path();

        let repo = Arc::new(SqliteSessionRepository::new(db_path)?);
        let manager = DailySessionManager::new(repo.clone());

        // Get today's session for the agent
        if let Ok(session) = manager.get_or_create_today(&agent_id).await {
            // Set stop flag in the database session state
            let stop_key = "execution_control::stop";
            let _ = repo.save_session_state_key(&agent_id, stop_key, &serde_json::json!(true)).await;

            Ok(serde_json::json!({
                "success": true,
                "agent_id": agent_id,
                "session_id": session.id,
                "message": "Stop flag set in session state"
            }))
        } else {
            Err(format!("Agent {} not found in cache and no active session", agent_id))
        }
    }
}

/// Get agent execution status
///
/// Returns the current execution status including iteration count and stop state
#[tauri::command]
pub async fn get_agent_execution_status(agent_id: String) -> Result<Value, String> {
    let cache = EXECUTOR_CACHE.read().await;

    // Find any executor for this agent
    let executor = cache.iter()
        .find(|(key, _)| key.agent_id == agent_id)
        .map(|(_, exec)| exec.clone());

    drop(cache);

    if let Some(executor) = executor {
        Ok(serde_json::json!({
            "agent_id": agent_id,
            "is_executing": true,
            "iteration": executor.get_iteration(),
            "stop_requested": executor.is_stop_requested(),
        }))
    } else {
        Ok(serde_json::json!({
            "agent_id": agent_id,
            "is_executing": false,
            "iteration": 0,
            "stop_requested": false,
        }))
    }
}

// ============================================================================
// TODO LIST COMMANDS
// ============================================================================

/// Get TODO list for an agent
///
/// Returns the TODO list from session state
#[tauri::command]
pub async fn get_agent_todos(agent_id: String) -> Result<Value, String> {
    let db_path = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;

    // Load session state
    if let Some(state) = repo.load_session_state(&agent_id).await
        .map_err(|e| format!("Failed to load session state: {}", e))? {

        // Get TODO list from state
        if let Some(todos) = state.get("app:todo_list") {
            return Ok(todos.clone());
        }
    }

    // Return empty TODO list if not found
    Ok(serde_json::json!({
        "items": [],
        "last_updated": chrono::Utc::now().to_rfc3339()
    }))
}

/// Save TODO list for an agent
///
/// Saves the TODO list to session state
#[tauri::command]
pub async fn save_agent_todos(agent_id: String, todos: Value) -> Result<Value, String> {
    let db_path = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;

    // Save TODO list to session state
    repo.save_session_state_key(&agent_id, "app:todo_list", &todos).await?;

    // If there's an active executor, also update its session state
    let cache = EXECUTOR_CACHE.read().await;
    if let Some(executor) = cache.iter()
        .find(|(key, _)| key.agent_id == agent_id)
        .map(|(_, exec)| exec.clone())
    {
        drop(cache);
        if let Ok(mut session) = executor.session().lock() {
            let _ = session.state_mut().set("app:todo_list".to_string(), todos.clone());
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "message": "TODO list saved"
    }))
}

/// Update a single TODO item
///
/// Convenience command to update just one TODO item's completion status
#[tauri::command]
pub async fn update_agent_todo(
    agent_id: String,
    todo_id: String,
    completed: bool,
) -> Result<Value, String> {
    let db_path = crate::settings::AppDirs::get()
        .map_err(|e| format!("Failed to get app dirs: {}", e))?
        .agent_channels_db_path();

    let repo = SqliteSessionRepository::new(db_path)?;

    // Load session state
    let mut state = repo.load_session_state(&agent_id).await
        .map_err(|e| format!("Failed to load session state: {}", e))?
        .unwrap_or_default();

    // Get and update TODO list
    let mut todos: Value = state.get("app:todo_list")
        .cloned()
        .unwrap_or(serde_json::json!({
            "items": [],
            "last_updated": chrono::Utc::now().to_rfc3339()
        }));

    // Find and update the item
    let mut found = false;
    if let Some(items) = todos.get_mut("items").and_then(|v| v.as_array_mut()) {
        for item in items.iter_mut() {
            if item.get("id").and_then(|v| v.as_str()) == Some(&todo_id) {
                item["completed"] = serde_json::json!(completed);
                item["completed_at"] = if completed {
                    serde_json::json!(chrono::Utc::now().to_rfc3339())
                } else {
                    serde_json::json!(null)
                };
                found = true;
                break;
            }
        }
    }

    if !found {
        return Err(format!("TODO item not found: {}", todo_id));
    }

    // Update last_updated
    todos["last_updated"] = serde_json::json!(chrono::Utc::now().to_rfc3339());

    // Save back
    state.insert("app:todo_list".to_string(), todos.clone());
    repo.save_session_state(&agent_id, &state).await?;

    // Update active executor session if exists
    let cache = EXECUTOR_CACHE.read().await;
    if let Some(executor) = cache.iter()
        .find(|(key, _)| key.agent_id == agent_id)
        .map(|(_, exec)| exec.clone())
    {
        drop(cache);
        if let Ok(mut session) = executor.session().lock() {
            let _ = session.state_mut().set("app:todo_list".to_string(), todos);
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "todo_id": todo_id,
        "completed": completed
    }))
}

// ============================================================================
// AGENT CREATOR COMMAND (Reserved System Agent)
// SEARCH INDEXING HELPER
// ============================================================================

/// Helper to index the most recent message from a session
/// This is called after messages are recorded to ensure they're searchable
async fn index_message_from_db(
    db_path: &std::path::PathBuf,
    session_id: &str,
    agent_id: &str,
) -> Result<(), String> {
    use rusqlite::params;

    // Get the most recent message for this session
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    let result: Result<Option<(String, String, String, String, i64, String)>, String> = (|| {
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, m.created_at, a.name as agent_name
             FROM messages m
             INNER JOIN agents a ON a.id = ?
             WHERE m.session_id = ?
             ORDER BY m.created_at DESC
             LIMIT 1"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let result = stmt.query_row(params![agent_id, session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,   // id
                row.get::<_, String>(1)?,   // session_id
                row.get::<_, String>(2)?,   // role
                row.get::<_, String>(3)?,   // content
                row.get::<_, String>(4)?,   // created_at
                row.get::<_, String>(5)?,   // agent_name
            ))
        });

        match result {
            Ok((id, session_id, role, content, created_at, agent_name)) => {
                let timestamp = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);
                Ok(Some((id, session_id, role, content, timestamp, agent_name)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to query message: {}", e))
        }
    })();

    // Now call the indexing function (async is fine here since we've dropped Statement)
    if let Ok(Some((id, session_id, role, content, timestamp, agent_name))) = result {
        crate::commands::search::index_message_internal(
            id,
            session_id,
            agent_id.to_string(),
            agent_name,
            role,
            content,
            timestamp,
        ).await;
    }

    Ok(())
}
