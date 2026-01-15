// ============================================================================
// AGENT RUNTIME COMMANDS
// Commands for executing AI agents with streaming support
// ============================================================================

use std::sync::Arc;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use serde::Serialize;

use crate::domains::conversation_runtime::{get_database, repository};
use crate::domains::conversation_runtime::repository::{MessageRole, ToolCall};
use crate::domains::agent_runtime::{
    AgentExecutor, create_executor, ChatMessage, StreamEvent
};

// Note: Executor caching removed - each execution gets a fresh executor
// with the correct conversation_id for scoped file operations

/// Remove a specific agent from the executor cache (no-op, kept for API compatibility)
pub async fn invalidate_executor_cache(_agent_id: &str) {
    // No-op: executors are now created per execution
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

    // Get or create executor
    // Note: We create a new executor per execution to include conversation_id for scoped file operations
    let executor = Arc::new(create_executor(&agent_id, Some(conversation_id.clone())).await?);

    // Load conversation history
    let messages = db.transaction(|conn| {
        repository::list_messages(conn, &conversation_id, None, None)
    })
    .map_err(|e| format!("Failed to load messages: {}", e))?;

    // Convert to ChatMessage format
    let history: Vec<ChatMessage> = messages.into_iter()
        .filter(|msg| !matches!(msg.role, MessageRole::System)) // Skip system messages for history
        .map(|msg| ChatMessage {
            role: msg.role.as_str().to_string(),
            content: msg.content,
            tool_calls: None,
            tool_call_id: None,
        })
        .collect();

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

    // Execute agent
    let mut final_response = String::new();
    let mut current_tool_calls: Vec<ToolCall> = Vec::new();

    // Convert StreamEvent to a serializable format for emitting
    #[derive(Clone, Serialize)]
    #[serde(tag = "type")]
    enum FrontendEvent {
        #[serde(rename = "metadata")]
        Metadata {
            timestamp: u64,
            #[serde(rename = "agentId")]
            agent_id: String,
            model: String,
            provider: String,
        },
        #[serde(rename = "token")]
        Token { timestamp: u64, content: String },
        #[serde(rename = "tool_call_start")]
        ToolCallStart {
            timestamp: u64,
            #[serde(rename = "toolId")]
            tool_id: String,
            #[serde(rename = "toolName")]
            tool_name: String,
            args: Value,
        },
        #[serde(rename = "tool_call_end")]
        ToolCallEnd {
            timestamp: u64,
            #[serde(rename = "toolId")]
            tool_id: String,
            #[serde(rename = "toolName")]
            tool_name: String,
            args: Value,
        },
        #[serde(rename = "tool_result")]
        ToolResult {
            timestamp: u64,
            #[serde(rename = "toolId")]
            tool_id: String,
            result: String,
            error: Option<String>,
        },
        #[serde(rename = "done")]
        Done {
            timestamp: u64,
            #[serde(rename = "finalMessage")]
            final_message: String,
            #[serde(rename = "tokenCount")]
            token_count: usize,
        },
        #[serde(rename = "error")]
        Error { timestamp: u64, error: String, recoverable: bool },
    }

    executor.execute_stream(&message, &history, |event| {
        let frontend_event = match event.clone() {
            StreamEvent::Metadata { timestamp, agent_id, model, provider } => {
                FrontendEvent::Metadata {
                    timestamp,
                    agent_id,
                    model,
                    provider,
                }
            }
            StreamEvent::Token { timestamp, content } => {
                final_response.push_str(&content);
                FrontendEvent::Token {
                    timestamp,
                    content,
                }
            }
            StreamEvent::ToolCallStart { timestamp, tool_id, tool_name, args } => {
                // Track tool calls for saving later
                current_tool_calls.push(ToolCall {
                    id: tool_id.clone(),
                    name: tool_name.clone(),
                    arguments: args.clone(),
                });
                FrontendEvent::ToolCallStart {
                    timestamp,
                    tool_id,
                    tool_name,
                    args,
                }
            }
            StreamEvent::ToolCallEnd { timestamp, tool_id, tool_name, args } => {
                FrontendEvent::ToolCallEnd {
                    timestamp,
                    tool_id,
                    tool_name,
                    args,
                }
            }
            StreamEvent::ToolResult { timestamp, tool_id, result, error } => {
                FrontendEvent::ToolResult {
                    timestamp,
                    tool_id,
                    result,
                    error,
                }
            }
            StreamEvent::Done { timestamp, final_message, token_count } => {
                FrontendEvent::Done {
                    timestamp,
                    final_message,
                    token_count,
                }
            }
            StreamEvent::Error { timestamp, error, recoverable } => {
                FrontendEvent::Error {
                    timestamp,
                    error,
                    recoverable,
                }
            }
        };

        // Emit event to frontend
        let event_name = format!("agent-stream://{}", conversation_id);
        if let Err(e) = app.emit(&event_name, frontend_event) {
            eprintln!("Failed to emit event to frontend: {}", e);
        }
    }).await
    .map_err(|e| format!("Agent execution failed: {}", e))?;

    // Save assistant response to database
    let assistant_msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
    let tool_calls_for_db = if current_tool_calls.is_empty() {
        None
    } else {
        Some(current_tool_calls.clone())
    };

    db.transaction(|conn| {
        repository::create_message(conn, repository::CreateMessage {
            id: assistant_msg_id,
            conversation_id: conversation_id.clone(),
            role: MessageRole::Assistant,
            content: final_response.clone(),
            token_count: Some(final_response.len() as i64),
            tool_calls: tool_calls_for_db,
            tool_results: None,
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

/// Clear executor cache (no-op, kept for API compatibility)
#[tauri::command]
pub async fn clear_executor_cache() -> Result<(), String> {
    // No-op: executors are now created per execution
    Ok(())
}
