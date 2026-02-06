// ============================================================================
// EXECUTOR MODULE
// Core agent execution engine
// ============================================================================

//! # Executor Module
//!
//! Core agent execution engine that coordinates LLM calls, tool execution,
//! and middleware processing.
//!
//! The executor is the main orchestrator that:
//! 1. Creates LLM client with provided configuration
//! 2. Uses provided tool registry and MCP manager
//! 3. Processes messages through middleware pipeline
//! 4. Executes LLM calls with streaming support
//! 5. Handles tool execution and result collection
//! 6. Emits events for real-time feedback

#![warn(missing_docs)]
#![warn(clippy::all)]

use std::sync::Arc;
use serde_json::{json, Value};

use crate::types::{ChatMessage, StreamEvent, ToolCall};
use crate::llm::LlmClient;
use crate::llm::client::StreamChunk;
use crate::tools::ToolRegistry;
use crate::tools::context::ToolContext;
use crate::mcp::McpManager;
use crate::middleware::MiddlewarePipeline;
use crate::middleware::traits::MiddlewareContext;
use crate::middleware::token_counter::estimate_total_tokens;
use zero_core::event::EventActions;
use zero_core::ToolContext as ZeroToolContext;

/// Result from tool execution including any actions set by the tool
struct ToolExecutionResult {
    output: String,
    actions: EventActions,
}

// ============================================================================
// EXECUTOR CONFIGURATION
// ============================================================================

/// Configuration for agent executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Agent identifier
    pub agent_id: String,

    /// Provider identifier
    pub provider_id: String,

    /// Model to use
    pub model: String,

    /// Temperature for generation (0.0 - 1.0)
    pub temperature: f64,

    /// Maximum tokens to generate
    pub max_tokens: u32,

    /// Enable reasoning/thinking
    pub thinking_enabled: bool,

    /// System instruction
    pub system_instruction: Option<String>,

    /// Enable tools
    pub tools_enabled: bool,

    /// MCP servers to use
    pub mcps: Vec<String>,

    /// Skills to use
    pub skills: Vec<String>,

    /// Conversation ID for scoping
    pub conversation_id: Option<String>,

    /// Initial state to inject into tool context.
    /// This allows passing hook context, delegation context, etc.
    #[allow(dead_code)]
    pub initial_state: std::collections::HashMap<String, Value>,

    /// Offload large tool results to filesystem instead of keeping in context.
    pub offload_large_results: bool,

    /// Character threshold for offloading (default: 20000 chars ≈ 5000 tokens).
    pub offload_threshold_chars: usize,

    /// Directory to save offloaded tool results.
    pub offload_dir: Option<std::path::PathBuf>,
}

impl ExecutorConfig {
    /// Create a new executor config
    #[must_use]
    pub fn new(agent_id: String, provider_id: String, model: String) -> Self {
        Self {
            agent_id,
            provider_id,
            model,
            temperature: 0.7,
            max_tokens: 2000,
            thinking_enabled: false,
            system_instruction: None,
            tools_enabled: true,
            mcps: Vec::new(),
            skills: Vec::new(),
            conversation_id: None,
            initial_state: std::collections::HashMap::new(),
            offload_large_results: false,
            offload_threshold_chars: 20_000, // ~5000 tokens
            offload_dir: None,
        }
    }

    /// Add initial state that will be injected into tool context
    #[must_use]
    pub fn with_initial_state(mut self, key: impl Into<String>, value: Value) -> Self {
        self.initial_state.insert(key.into(), value);
        self
    }
}

// ============================================================================
// AGENT EXECUTOR
// ============================================================================

/// Main agent executor
///
/// Coordinates LLM calls, tool execution, and middleware processing.
pub struct AgentExecutor {
    config: ExecutorConfig,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    mcp_manager: Arc<McpManager>,
    middleware_pipeline: Arc<MiddlewarePipeline>,
}

impl AgentExecutor {
    /// Create a new agent executor
    ///
    /// # Arguments
    /// * `config` - Executor configuration
    /// * `llm_client` - LLM client for making API calls
    /// * `tool_registry` - Registry of available tools
    /// * `mcp_manager` - MCP manager for external tools
    /// * `middleware_pipeline` - Middleware pipeline for preprocessing
    pub fn new(
        config: ExecutorConfig,
        llm_client: Arc<dyn LlmClient>,
        tool_registry: Arc<ToolRegistry>,
        mcp_manager: Arc<McpManager>,
        middleware_pipeline: Arc<MiddlewarePipeline>,
    ) -> Result<Self, ExecutorError> {
        Ok(Self {
            config,
            llm_client,
            tool_registry,
            mcp_manager,
            middleware_pipeline,
        })
    }

    /// Set the middleware pipeline
    pub fn set_middleware_pipeline(&mut self, pipeline: Arc<MiddlewarePipeline>) {
        self.middleware_pipeline = pipeline;
    }

    /// Get the middleware pipeline
    #[must_use]
    pub fn middleware_pipeline(&self) -> &Arc<MiddlewarePipeline> {
        &self.middleware_pipeline
    }

    /// Get the configuration
    #[must_use]
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Execute the agent with streaming
    ///
    /// The callback receives events as they occur during execution.
    pub async fn execute_stream(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<(), ExecutorError> {
        // Emit metadata event
        on_event(StreamEvent::Metadata {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            agent_id: self.config.agent_id.clone(),
            model: self.config.model.clone(),
            provider: self.config.provider_id.clone(),
        });

        // Build messages array
        let mut messages = Vec::new();

        // Add system instruction if available
        if let Some(instruction) = &self.config.system_instruction {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: instruction.clone(),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Add conversation history
        messages.extend(history.iter().cloned());

        // Add current user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
            tool_calls: None,
            tool_call_id: None,
        });

        // Create middleware context
        let message_count = messages.len();
        let estimated_tokens = estimate_total_tokens(&messages);

        // Build execution state from message history.
        // This extracts skill information from previous tool calls so middleware
        // can make skill-aware decisions during context compaction.
        let execution_state = crate::middleware::traits::ExecutionState::from_messages(&messages);

        let middleware_context = MiddlewareContext::new(
            self.config.agent_id.clone(),
            self.config.conversation_id.clone(),
            self.config.provider_id.clone(),
            self.config.model.clone(),
        )
        .with_counts(message_count, estimated_tokens)
        .with_execution_state(execution_state);

        // Process messages through middleware pipeline
        let processed_messages = self.middleware_pipeline
            .process_messages(messages, &middleware_context, &mut on_event)
            .await
            .map_err(|e| ExecutorError::MiddlewareError(e))?;

        // Get tools schema if enabled
        let tools_schema = if self.config.tools_enabled {
            Some(self.build_tools_schema().await?)
        } else {
            None
        };

        tracing::debug!("Starting execute_with_tools_loop");

        // Execute with tool calling loop
        self.execute_with_tools_loop(processed_messages, tools_schema, &mut on_event).await
    }

    async fn execute_with_tools_loop(
        &self,
        messages: Vec<ChatMessage>,
        tools_schema: Option<Value>,
        on_event: &mut impl FnMut(StreamEvent),
    ) -> Result<(), ExecutorError> {
        tracing::debug!("=== execute_with_tools_loop starting ===");
        tracing::debug!("Messages count: {}", messages.len());
        tracing::debug!("Tools schema: {}", tools_schema.is_some());

        let mut current_messages = messages;
        let mut max_iterations = 50;
        #[allow(unused_assignments)] // Initialized here, assigned in loop exit condition
        let mut full_response = String::new();

        // Track cumulative token usage across the session
        let mut total_tokens_in: u64 = 0;
        let mut total_tokens_out: u64 = 0;

        // Create shared tool context that persists across all tool calls in this execution.
        // This allows tools like load_skill to maintain state (e.g., loaded skills, resources)
        // that other tools and middleware can access throughout the execution loop.
        let shared_tool_context = Arc::new(ToolContext::full_with_state(
            self.config.agent_id.clone(),
            self.config.conversation_id.clone(),
            self.config.skills.clone(),
            self.config.initial_state.clone(),
        ));

        loop {
            if max_iterations == 0 {
                return Err(ExecutorError::MaxIterationsReached);
            }
            max_iterations -= 1;

            // Real streaming via chat_stream() with mpsc channel bridge.
            // Tokens are emitted to the user IMMEDIATELY as they arrive from the LLM,
            // including intermediate text that accompanies tool calls.
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamChunk>();

            let llm_client = self.llm_client.clone();
            let messages_for_stream = current_messages.clone();
            let tools_for_stream = tools_schema.clone();

            // Spawn the streaming LLM call in a separate task
            let stream_handle = tokio::spawn(async move {
                llm_client.chat_stream(
                    messages_for_stream,
                    tools_for_stream,
                    Box::new(move |chunk| {
                        let _ = tx.send(chunk);
                    }),
                ).await
            });

            // Process chunks as they arrive — emit Token events in real-time
            let mut streamed_content = String::new();
            while let Some(chunk) = rx.recv().await {
                match chunk {
                    StreamChunk::Token(text) => {
                        streamed_content.push_str(&text);
                        on_event(StreamEvent::Token {
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                            content: text,
                        });
                    }
                    StreamChunk::Reasoning(text) => {
                        on_event(StreamEvent::Reasoning {
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                            content: text,
                        });
                    }
                    StreamChunk::ToolCall(_) => {
                        // Tool call chunks are accumulated by the streaming impl
                        // and returned in the final ChatResponse. No action needed here.
                    }
                }
            }

            // Await the final response (channel closed = stream complete)
            let response = stream_handle.await
                .map_err(|e| ExecutorError::LlmError(format!("Stream task panicked: {}", e)))?
                .map_err(|e| ExecutorError::LlmError(e.to_string()))?;

            // Update cumulative token counts and emit event
            if let Some(usage) = &response.usage {
                total_tokens_in += usage.prompt_tokens as u64;
                total_tokens_out += usage.completion_tokens as u64;

                on_event(StreamEvent::TokenUpdate {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tokens_in: total_tokens_in,
                    tokens_out: total_tokens_out,
                });
            }

            tracing::debug!("LLM response - content: '{}', tool_calls: {}",
                response.content, response.tool_calls.as_ref().map_or(0, |v| v.len()));

            // Check for tool calls
            let tool_calls = response.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                // No tool calls, this is the final response
                // Text was already streamed in real-time above
                full_response = response.content.clone();
                tracing::debug!("No tool calls, final response length: {}", full_response.len());
                break;
            }

            // Handle tool calls
            // Add the assistant message with TRUNCATED tool calls to prevent context explosion
            let truncated_tool_calls: Vec<ToolCall> = tool_calls.iter().map(|tc| {
                // Truncate arguments to prevent exponential context growth
                let truncated_args = truncate_tool_args(&tc.arguments, 500);
                ToolCall::new(tc.id.clone(), tc.name.clone(), truncated_args)
            }).collect();

            current_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: response.content.clone(),
                tool_calls: Some(truncated_tool_calls),
                tool_call_id: None,
            });

            // Track if respond tool was called - signals we should stop after this batch
            let mut should_stop_after_respond = false;

            // Execute each tool and add its result
            for tool_call in &tool_calls {
                let tool_name = &tool_call.name;
                let args = &tool_call.arguments;

                tracing::debug!("Executing tool: {} with args: {}", tool_name, args);

                on_event(StreamEvent::ToolCallStart {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tool_id: tool_call.id.clone(),
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                });

                let result = self.execute_tool(&shared_tool_context, &tool_call.id, tool_name, args).await;

                match result {
                    Ok(tool_result) => {
                        let output = tool_result.output;
                        let actions = tool_result.actions;
                        
                        tracing::debug!("Tool result: {}", output);
                        
                        // Check for respond action
                        if let Some(respond) = &actions.respond {
                            on_event(StreamEvent::ActionRespond {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                message: respond.message.clone(),
                                format: respond.format.clone(),
                                conversation_id: respond.conversation_id.clone(),
                                session_id: respond.session_id.clone(),
                            });
                            // Signal to stop after processing this batch of tool calls
                            should_stop_after_respond = true;
                            tracing::debug!("Respond action detected, will stop after current tool batch");
                        }
                        
                        // Check for delegate action
                        if let Some(delegate) = &actions.delegate {
                            on_event(StreamEvent::ActionDelegate {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                agent_id: delegate.agent_id.clone(),
                                task: delegate.task.clone(),
                                context: delegate.context.clone(),
                                wait_for_result: delegate.wait_for_result,
                            });
                        }

                        // Check for generative UI markers
                        if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                            // Check for show_content marker
                            if parsed.get("__show_content")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                let content_type = parsed.get("content_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("text").to_string();
                                let title = parsed.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Content").to_string();
                                let content = parsed.get("content")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("").to_string();
                                let metadata = parsed.get("metadata").cloned();
                                let file_path = parsed.get("file_path")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let is_attachment = parsed.get("is_attachment")
                                    .and_then(|v| v.as_bool());
                                let base64 = parsed.get("base64")
                                    .and_then(|v| v.as_bool());

                                on_event(StreamEvent::ShowContent {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    content_type,
                                    title,
                                    content,
                                    metadata,
                                    file_path,
                                    is_attachment,
                                    base64,
                                });
                            }

                            // Check for request_input marker
                            if parsed.get("__request_input")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                let form_id = parsed.get("form_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&format!("form_{}", chrono::Utc::now().timestamp()))
                                    .to_string();
                                let form_type = parsed.get("form_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("json_schema").to_string();
                                let title = parsed.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Input Required").to_string();
                                let description = parsed.get("description")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let schema = parsed.get("schema")
                                    .cloned()
                                    .unwrap_or_else(|| json!({}));
                                let submit_button = parsed.get("submit_button")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());

                                on_event(StreamEvent::RequestInput {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    form_id,
                                    form_type,
                                    title,
                                    description,
                                    schema,
                                    submit_button,
                                });
                            }
                        }

                        on_event(StreamEvent::ToolResult {
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                            tool_id: tool_call.id.clone(),
                            result: output.clone(),
                            error: None,
                        });

                        // Process tool result (potentially offload large results to filesystem)
                        let processed_output = self.process_tool_result(tool_name, output);

                        // Add tool result message
                        current_messages.push(ChatMessage {
                            role: "tool".to_string(),
                            content: processed_output,
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                        });
                    }
                    Err(e) => {
                        tracing::debug!("Tool error: {}", e);
                        on_event(StreamEvent::ToolResult {
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                            tool_id: tool_call.id.clone(),
                            result: String::new(),
                            error: Some(e.clone()),
                        });

                        // Add error result message
                        current_messages.push(ChatMessage {
                            role: "tool".to_string(),
                            content: json!({"error": e}).to_string(),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                        });
                    }
                }

                on_event(StreamEvent::ToolCallEnd {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tool_id: tool_call.id.clone(),
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                });
            }

            // If respond tool was called, stop the loop - agent has finished responding
            if should_stop_after_respond {
                tracing::debug!("Stopping execution loop after respond action");
                break;
            }
        }

        // Emit done event
        on_event(StreamEvent::Done {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            final_message: full_response.clone(),
            token_count: full_response.len(),
        });

        // Emit context state for checkpoint persistence
        // This includes skill tracking (graph), loaded skills, and other tool context state
        // that should be persisted for session resumption.
        on_event(StreamEvent::ContextState {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            state: shared_tool_context.export_state(),
        });

        Ok(())
    }

    async fn execute_tool(
        &self,
        shared_ctx: &Arc<ToolContext>,
        tool_call_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<ToolExecutionResult, String> {
        // First try built-in tools
        if let Some(tool) = self.tool_registry.find(tool_name) {
            // Use shared context that persists across all tool calls in this execution.
            // Set the function_call_id for this specific tool call so tools can track
            // their position in the conversation (e.g., for skill loading).
            shared_ctx.set_function_call_id(tool_call_id.to_string());

            let result = tool.execute(shared_ctx.clone(), arguments.clone()).await
                .map_err(|e| format!("Tool execution failed: {:?}", e))?;

            // Get any actions that were set by the tool
            let actions = shared_ctx.actions();

            return Ok(ToolExecutionResult {
                output: serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string()),
                actions,
            });
        }

        // Try MCP tools
        // MCP tools have format: {normalized_server_id}__{normalized_tool_name}
        if tool_name.contains("__") {
            let parts: Vec<&str> = tool_name.splitn(2, "__").collect();
            if parts.len() == 2 {
                let normalized_server_id = parts[0];
                let normalized_tool = parts[1];

                // Find the original server ID by checking which one matches when normalized
                let mut original_server_id: Option<String> = None;
                for mcp_id in &self.config.mcps {
                    if normalize_tool_name(mcp_id) == normalized_server_id {
                        original_server_id = Some(mcp_id.clone());
                        break;
                    }
                }

                let server_id = match original_server_id {
                    Some(id) => id,
                    None => {
                        // Fallback: try the normalized ID directly (old behavior)
                        normalized_server_id.replace('_', "-")
                    }
                };

                // Find the original tool name by listing tools from this server
                let actual_tool = if let Some(client) = self.mcp_manager.get_client(&server_id).await {
                    if let Ok(tools) = client.list_tools().await {
                        tools.into_iter()
                            .find(|t| normalize_tool_name(&t.name) == normalized_tool)
                            .map(|t| t.name)
                            .unwrap_or_else(|| normalized_tool.to_string())
                    } else {
                        normalized_tool.to_string()
                    }
                } else {
                    normalized_tool.to_string()
                };

                tracing::debug!("Executing MCP tool: server={}, tool={}", server_id, actual_tool);
                let output = self.mcp_manager.execute_tool(&server_id, &actual_tool, arguments.clone()).await
                    .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "null".to_string()))
                    .map_err(|e| e.to_string())?;

                // MCP tools don't support actions (yet)
                return Ok(ToolExecutionResult {
                    output,
                    actions: EventActions::default(),
                });
            }
        }

        Err(format!("Tool not found: {}", tool_name))
    }

    /// Normalize MCP tool parameters to OpenAI format
    ///
    /// OpenAI requires parameters to have `type: "object"` at the root.
    /// MCP tools may return parameters without this wrapper.
    fn normalize_mcp_parameters(params: Option<Value>) -> Value {
        match params {
            None => json!({"type": "object", "properties": {}}),
            Some(p) => {
                // If parameters already has "type: object", use as-is
                if p.get("type").is_some() {
                    p
                } else {
                    // Otherwise, wrap it with type: object
                    json!({
                        "type": "object",
                        "properties": p
                    })
                }
            }
        }
    }

    async fn build_tools_schema(&self) -> Result<Value, ExecutorError> {
        let mut tools = Vec::new();

        for tool in self.tool_registry.get_all() {
            let tool_name = tool.name();
            let tool_desc = tool.description();
            let schema = tool.parameters_schema().unwrap_or_else(|| json!(null));

            // Validate tool name and description aren't empty
            if tool_name.is_empty() {
                tracing::error!("Tool has empty name, skipping");
                continue;
            }
            if tool_desc.is_empty() {
                tracing::error!("Tool '{}' has empty description, skipping", tool_name);
                continue;
            }

            tools.push(json!({
                "type": "function",
                "function": {
                    "name": tool_name,
                    "description": tool_desc,
                    "parameters": schema
                }
            }));
        }

        tracing::info!("Registered {} built-in tools for LLM", tools.len());

        // Add MCP tools
        for mcp_id in &self.config.mcps {
            if let Some(client) = self.mcp_manager.get_client(mcp_id).await {
                let mcp_tools = client.list_tools().await
                    .map_err(|e| ExecutorError::McpError(format!("Failed to list MCP tools: {}", e)))?;

                tracing::info!("Loaded {} MCP tools from server {}", mcp_tools.len(), mcp_id);

                for mcp_tool in mcp_tools {
                    // Convert MCP ID and tool name to valid OpenAI tool name format
                    // Pattern must match: ^[a-zA-Z0-9_-]+$
                    let mcp_id_normalized = normalize_tool_name(mcp_id);
                    let tool_name_normalized = normalize_tool_name(&mcp_tool.name);
                    let tool_name = format!("{}__{}", mcp_id_normalized, tool_name_normalized);

                    // Normalize parameters to OpenAI format (must have type: "object")
                    let parameters = Self::normalize_mcp_parameters(mcp_tool.parameters);

                    tools.push(json!({
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "description": mcp_tool.description,
                            "parameters": parameters
                        }
                    }));
                }
            }
        }

        tracing::info!("Total tools available to LLM: {}", tools.len());
        Ok(json!(tools))
    }

    /// Execute agent without streaming (simpler API)
    pub async fn execute(
        &self,
        user_message: &str,
        history: &[ChatMessage],
    ) -> Result<String, ExecutorError> {
        let mut final_response = String::new();

        self.execute_stream(user_message, history, |event| {
            if let StreamEvent::Token { content, .. } = event {
                final_response.push_str(&content);
            }
        }).await?;

        Ok(final_response)
    }

    /// Process tool result, potentially offloading large results to filesystem.
    ///
    /// If offload is enabled and the result exceeds the threshold, saves to a temp file
    /// and returns instructions for the agent to read it with a CLI tool.
    fn process_tool_result(&self, tool_name: &str, result: String) -> String {
        // Check if offload is enabled and result exceeds threshold
        if !self.config.offload_large_results {
            return result;
        }

        if result.len() <= self.config.offload_threshold_chars {
            return result;
        }

        // Get offload directory
        let offload_dir = match &self.config.offload_dir {
            Some(dir) => dir.clone(),
            None => {
                // Default to ~/Documents/agentzero/temp
                if let Some(home) = dirs::home_dir() {
                    home.join("Documents").join("agentzero").join("temp")
                } else {
                    tracing::warn!("Could not determine home directory for offload");
                    return result;
                }
            }
        };

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&offload_dir) {
            tracing::warn!("Failed to create offload directory: {}", e);
            return result;
        }

        // Generate unique filename
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let sanitized_tool = tool_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let filename = format!("{}_{}.txt", sanitized_tool, timestamp);
        let file_path = offload_dir.join(&filename);

        // Write result to file
        if let Err(e) = std::fs::write(&file_path, &result) {
            tracing::warn!("Failed to write offloaded result: {}", e);
            return result;
        }

        let result_size = result.len();
        let result_tokens = result_size / 4; // rough estimate

        tracing::info!(
            "Offloaded large tool result ({} chars, ~{} tokens) to: {}",
            result_size,
            result_tokens,
            file_path.display()
        );

        // Return instructions for the agent
        format!(
            "Tool result was too large for context ({} chars, ~{} tokens).\n\
            Result saved to: {}\n\n\
            To access the full result, use the `read` tool:\n\
            ```json\n\
            {{\"path\": \"{}\"}}\n\
            ```\n\n\
            Or use shell: `head -100 \"{}\"` to preview, `grep \"pattern\" \"{}\"` to search.",
            result_size,
            result_tokens,
            file_path.display(),
            file_path.display(),
            file_path.display(),
            file_path.display()
        )
    }
}

/// Normalize a string to be a valid OpenAI tool name.
///
/// OpenAI requires tool names to match: ^[a-zA-Z0-9_-]+$
/// This function replaces any invalid characters with underscores.
fn normalize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Executor errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Maximum iterations reached")]
    MaxIterationsReached,

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("MCP error: {0}")]
    McpError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Middleware error: {0}")]
    MiddlewareError(String),
}

/// Factory function to create an executor
///
/// This is a convenience function that creates an executor with default
/// components. For more control, create components separately and use
/// `AgentExecutor::new()`.
pub async fn create_executor(
    config: ExecutorConfig,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    mcp_manager: Arc<McpManager>,
) -> Result<AgentExecutor, ExecutorError> {
    // Create default middleware pipeline
    let middleware_pipeline = Arc::new(MiddlewarePipeline::new());

    AgentExecutor::new(
        config,
        llm_client,
        tool_registry,
        mcp_manager,
        middleware_pipeline,
    )
}

/// Truncate tool arguments to prevent context explosion.
///
/// When LLMs generate tool calls with massive arguments (e.g., including
/// full conversation context), storing these in message history causes
/// exponential growth. This function truncates arguments to a reasonable size.
fn truncate_tool_args(args: &Value, max_chars: usize) -> Value {
    let args_str = serde_json::to_string(args).unwrap_or_default();
    if args_str.len() <= max_chars {
        return args.clone();
    }

    // For objects, try to truncate string values
    if let Some(obj) = args.as_object() {
        let mut truncated = serde_json::Map::new();
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                if s.len() > 200 {
                    truncated.insert(
                        key.clone(),
                        Value::String(format!("{}... [truncated, {} chars]", &s[..200], s.len())),
                    );
                } else {
                    truncated.insert(key.clone(), value.clone());
                }
            } else {
                truncated.insert(key.clone(), value.clone());
            }
        }
        return Value::Object(truncated);
    }

    // Fallback: return a placeholder
    json!({"_truncated": true, "_original_size": args_str.len()})
}
