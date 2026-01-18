// ============================================================================
// AGENT RUNTIME COMMANDS
// Commands for executing AI agents with streaming support
// ============================================================================

use std::sync::Arc;
use std::collections::HashMap;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use crate::domains::conversation_runtime::{get_database, repository};
use crate::domains::conversation_runtime::repository::{MessageRole, ToolCall, ToolResult};
use crate::domains::agent_runtime::executor_v2::{create_zero_executor, ZeroAppStreamEvent};

// ============================================================================
// EXECUTOR CACHE
// ============================================================================

/// Cache key for executors (agent_id, conversation_id)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    agent_id: String,
    conversation_id: String,
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
/// 1. Loads agent configuration
/// 2. Creates/reuses an executor
/// 3. Executes the agent with tool calling
/// 4. Emits events to frontend in real-time
/// 5. Returns the final response
#[tauri::command]
pub async fn execute_agent_stream(
    app: AppHandle,
    conversation_id: String,
    agent_id: String,
    message: String,
) -> Result<Value, String> {
    // Verify conversation exists
    let db = get_database()?;
    let conversation = db.transaction(|conn| {
        repository::get_conversation(conn, &conversation_id)
    })
    .map_err(|e| format!("Database error: {}", e))?;

    if conversation.is_none() {
        return Err(format!("Conversation not found: {}", conversation_id));
    }

    // Save user message to database
    let user_msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
    db.transaction(|conn| {
        repository::create_message(conn, repository::CreateMessage {
            id: user_msg_id.clone(),
            conversation_id: conversation_id.clone(),
            role: MessageRole::User,
            content: message.clone(),
            token_count: None,
            tool_calls: None,
            tool_results: None,
        })
    })
    .map_err(|e| format!("Failed to save user message: {}", e))?;

    // Get or create zero-app executor from cache
    let cache_key = CacheKey {
        agent_id: agent_id.clone(),
        conversation_id: conversation_id.clone(),
    };

    // Check cache first
    {
        let cache_read = EXECUTOR_CACHE.read().await;
        if let Some(_executor) = cache_read.get(&cache_key) {
            tracing::info!("Using cached executor for agent: {} (conversation: {})", agent_id, conversation_id);
        }
    }

    // Try to get from cache, or create new executor
    let executor = {
        let cache_read = EXECUTOR_CACHE.read().await;
        cache_read.get(&cache_key).cloned()
    };

    let executor = if let Some(exec) = executor {
        exec
    } else {
        // Create new executor (cache miss)
        tracing::info!("Creating new executor for agent: {} (conversation: {})", agent_id, conversation_id);
        let exec = Arc::new(create_zero_executor(&agent_id, Some(conversation_id.clone())).await?);

        // Add to cache
        let mut cache_write = EXECUTOR_CACHE.write().await;
        cache_write.insert(cache_key, exec.clone());

        exec
    };

    // Execute with streaming
    let mut final_response = String::new();
    let mut current_tool_calls: Vec<ToolCall> = Vec::new();
    let mut current_tool_results: Vec<ToolResult> = Vec::new();

    executor.run_stream(message, |stream_event| {
        let event_name = format!("agent-stream://{}", conversation_id);

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
                tracing::info!("ToolResponse: toolId={}, response.len={}", id, response.len());

                // Add to tool results for database
                current_tool_results.push(ToolResult {
                    tool_call_id: id.clone(),
                    output: response.clone(),
                    error: None,
                });

                // Check for special UI markers in the response
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                    tracing::info!("ToolResponse parsed: toolId={}, has___show_content={}, has___request_input={}",
                        id,
                        parsed.get("__show_content").is_some(),
                        parsed.get("__request_input").is_some());

                    // Check for show_content marker
                    if parsed.get("__show_content").and_then(|v| v.as_bool()).unwrap_or(false) {
                        tracing::info!("Detected show_content marker, emitting show_content event");
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
                        return;
                    }

                    // Check for request_input marker
                    if parsed.get("__request_input").and_then(|v| v.as_bool()).unwrap_or(false) {
                        tracing::info!("Detected request_input marker, emitting request_input event");
                        if let Err(e) = app.emit(&event_name, serde_json::json!({
                            "type": "request_input",
                            "timestamp": chrono::Utc::now().timestamp_millis(),
                            "formId": parsed.get("form_id").and_then(|v| v.as_str()),
                            "title": parsed.get("title").and_then(|v| v.as_str()),
                            "description": parsed.get("description").and_then(|v| v.as_str()),
                            "schema": parsed.get("schema"),
                            "submitButton": parsed.get("submit_button").and_then(|v| v.as_str()),
                        })) {
                            eprintln!("Failed to emit request_input event to frontend: {}", e);
                        }
                        return;
                    }
                } else {
                    tracing::warn!("Failed to parse tool response as JSON: {}", response.chars().take(200).collect::<String>());
                }

                // Regular tool result - emit tool_result event
                tracing::info!("Emitting regular tool_result: toolId={}, result.preview={}",
                    id, response.chars().take(200).collect::<String>());
                if let Err(e) = app.emit(&event_name, serde_json::json!({
                    "type": "tool_result",
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                    "toolId": id,
                    "result": response
                })) {
                    eprintln!("Failed to emit event to frontend: {}", e);
                } else {
                    tracing::info!("Successfully emitted tool_result event");
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
        }
    }).await.map_err(|e| format!("Agent execution failed: {}", e))?;

    // Save assistant response to database
    let assistant_msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
    let tool_calls_for_db = if current_tool_calls.is_empty() {
        None
    } else {
        Some(current_tool_calls.clone())
    };
    let tool_results_for_db = if current_tool_results.is_empty() {
        None
    } else {
        Some(current_tool_results.clone())
    };

    db.transaction(|conn| {
        repository::create_message(conn, repository::CreateMessage {
            id: assistant_msg_id,
            conversation_id: conversation_id.clone(),
            role: MessageRole::Assistant,
            content: final_response.clone(),
            token_count: Some(final_response.len() as i64),
            tool_calls: tool_calls_for_db,
            tool_results: tool_results_for_db,
        })
    })
    .map_err(|e| format!("Failed to save assistant message: {}", e))?;

    Ok(serde_json::json!({
        "conversation_id": conversation_id,
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

/// Clear executor cache (for testing/debugging)
#[tauri::command]
pub async fn clear_executor_cache() -> Result<(), String> {
    clear_executor_cache_internal().await;
    Ok(())
}
