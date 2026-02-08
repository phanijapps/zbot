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

use std::collections::{HashSet, VecDeque};
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

    /// Maximum characters for a tool result in context (default: 30000 chars ≈ 7500 tokens).
    /// Results exceeding this are truncated to head + tail with a notice.
    /// Set to 0 to disable truncation.
    pub max_tool_result_chars: usize,

    /// Offload large tool results to filesystem instead of keeping in context.
    pub offload_large_results: bool,

    /// Character threshold for offloading (default: 20000 chars ≈ 5000 tokens).
    pub offload_threshold_chars: usize,

    /// Directory to save offloaded tool results.
    pub offload_dir: Option<std::path::PathBuf>,

    /// Maximum LLM loop iterations before checking for progress (default: 50).
    pub max_iterations: u32,

    /// Maximum times auto-extension can be granted (default: 3, so 50 + 3*25 = 125 max).
    pub max_extensions: u32,

    /// Additional iterations granted per auto-extension (default: 25).
    pub extension_size: u32,
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
            max_tool_result_chars: 30_000, // ~7500 tokens
            offload_large_results: false,
            offload_threshold_chars: 20_000, // ~5000 tokens
            offload_dir: None,
            max_iterations: 50,
            max_extensions: 3,
            extension_size: 25,
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
        let mut remaining_iterations = self.config.max_iterations;
        #[allow(unused_assignments)] // Initialized here, assigned in loop exit condition
        let mut full_response = String::new();

        // Track cumulative token usage across the session
        let mut total_tokens_in: u64 = 0;
        let mut total_tokens_out: u64 = 0;

        // Progress tracker for intelligent auto-extension
        let mut progress_tracker = ProgressTracker::new(self.config.max_extensions);

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
            // Early termination if agent is clearly stuck (mid-window check)
            if progress_tracker.is_clearly_stuck() {
                let diagnosis = progress_tracker.diagnosis();
                tracing::warn!(
                    total_iterations = progress_tracker.total_iterations,
                    score = progress_tracker.score,
                    diagnosis = %diagnosis,
                    "Early termination: agent clearly stuck"
                );
                return Err(ExecutorError::MaxIterationsNeedsIntervention {
                    iterations_used: progress_tracker.total_iterations,
                    reason: diagnosis,
                });
            }

            if remaining_iterations == 0 {
                if progress_tracker.should_extend() {
                    let reason = progress_tracker.extension_reason();
                    let iterations_used = progress_tracker.total_iterations;
                    let extension_size = self.config.extension_size;
                    remaining_iterations = extension_size;
                    progress_tracker.grant_extension();

                    on_event(StreamEvent::IterationsExtended {
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        iterations_used,
                        iterations_added: extension_size,
                        reason,
                    });

                    tracing::info!(
                        iterations_used = iterations_used,
                        extension = extension_size,
                        extensions_granted = progress_tracker.extensions_granted,
                        "Auto-extending iterations: agent making progress"
                    );

                    // Inject budget awareness into conversation
                    let extensions_remaining = self
                        .config
                        .max_extensions
                        .saturating_sub(progress_tracker.extensions_granted);
                    current_messages.push(ChatMessage::user(format!(
                        "[SYSTEM: You have used {} iterations and were granted {} more. \
                         You have {} extension(s) remaining. \
                         If you are repeating similar actions, stop and use the `respond` tool to \
                         summarize what you've accomplished so far and what remains.]",
                        iterations_used, extension_size, extensions_remaining,
                    )));
                } else {
                    let diagnosis = progress_tracker.diagnosis();
                    tracing::warn!(
                        total_iterations = progress_tracker.total_iterations,
                        diagnosis = %diagnosis,
                        "Max iterations reached, no progress detected"
                    );
                    return Err(ExecutorError::MaxIterationsNeedsIntervention {
                        iterations_used: progress_tracker.total_iterations,
                        reason: diagnosis,
                    });
                }
            }
            remaining_iterations -= 1;
            progress_tracker.tick();

            // Warn agent when approaching final limit (last 10 iterations of final window)
            let is_final_window =
                progress_tracker.extensions_granted >= self.config.max_extensions;
            if is_final_window && remaining_iterations == 10 {
                current_messages.push(ChatMessage::user(
                    "[SYSTEM: You have 10 iterations remaining and no more extensions. \
                     Wrap up your current task. Use the `respond` tool to deliver your results. \
                     Do not start new work.]"
                        .to_string(),
                ));
            }

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

            // Process chunks as they arrive — emit Token events in real-time.
            // Uses tokio::select! with a 10s heartbeat interval so that during
            // extended silent phases (e.g., LLM reasoning), heartbeat events keep
            // WebSocket connections alive (client PONG_TIMEOUT is 30s).
            let mut streamed_content = String::new();
            let mut heartbeat_interval = tokio::time::interval(std::time::Duration::from_secs(10));
            heartbeat_interval.tick().await; // consume immediate first tick

            loop {
                tokio::select! {
                    chunk = rx.recv() => {
                        match chunk {
                            Some(StreamChunk::Token(text)) => {
                                streamed_content.push_str(&text);
                                on_event(StreamEvent::Token {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    content: text,
                                });
                                heartbeat_interval.reset();
                            }
                            Some(StreamChunk::Reasoning(text)) => {
                                on_event(StreamEvent::Reasoning {
                                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                    content: text,
                                });
                                heartbeat_interval.reset();
                            }
                            Some(StreamChunk::ToolCall(_)) => {
                                // Tool call chunks are accumulated by the streaming impl
                                // and returned in the final ChatResponse. No action needed here.
                                heartbeat_interval.reset();
                            }
                            None => break, // channel closed
                        }
                    }
                    _ = heartbeat_interval.tick() => {
                        on_event(StreamEvent::Heartbeat {
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        });
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

            // Emit ToolCallStart events for all tools before execution
            for tool_call in &tool_calls {
                on_event(StreamEvent::ToolCallStart {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tool_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    args: tool_call.arguments.clone(),
                });
            }

            // Execute all tools concurrently
            let tool_futures: Vec<_> = tool_calls.iter().map(|tc| {
                let ctx = shared_tool_context.clone();
                let tool_id = tc.id.clone();
                let tool_name = tc.name.clone();
                let args = tc.arguments.clone();
                async move {
                    tracing::debug!("Executing tool: {} with args: {}", tool_name, args);
                    self.execute_tool(&ctx, &tool_id, &tool_name, &args).await
                }
            }).collect();

            let results = futures::future::join_all(tool_futures).await;

            // Process results in original order
            for (tool_call, result) in tool_calls.iter().zip(results) {
                let tool_name = &tool_call.name;

                match result {
                    Ok(tool_result) => {
                        let output = tool_result.output;
                        let actions = tool_result.actions;

                        tracing::debug!("Tool result: {}", output);

                        // Track progress: tool succeeded
                        progress_tracker.record_tool_call(tool_name, &tool_call.arguments, true);

                        // Check for respond action
                        if let Some(respond) = &actions.respond {
                            on_event(StreamEvent::ActionRespond {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                message: respond.message.clone(),
                                format: respond.format.clone(),
                                conversation_id: respond.conversation_id.clone(),
                                session_id: respond.session_id.clone(),
                            });
                            should_stop_after_respond = true;
                            progress_tracker.record_respond();
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

                            // Check for ward_changed marker (from ward tool)
                            if parsed.get("__ward_changed__")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                if let Some(ward_id) = parsed.get("ward_id").and_then(|v| v.as_str()) {
                                    on_event(StreamEvent::WardChanged {
                                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                        ward_id: ward_id.to_string(),
                                    });
                                }
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

                        // Truncate if still over budget (safety net when offload is disabled)
                        let processed_output = truncate_tool_result(
                            processed_output,
                            self.config.max_tool_result_chars,
                        );

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

                        // Track progress: tool failed
                        progress_tracker.record_tool_call(tool_name, &tool_call.arguments, false);
                        progress_tracker.record_error(&e);

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
                    args: tool_call.arguments.clone(),
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

            // Clear actions before execution so we capture only this tool's actions
            shared_ctx.set_actions(EventActions::default());

            let result = tool.execute(shared_ctx.clone(), arguments.clone()).await
                .map_err(|e| format!("Tool execution failed: {:?}", e))?;

            // Atomically take any actions that were set by the tool
            let actions = shared_ctx.take_actions();

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

    /// Harden a tool parameter schema for stricter LLM compliance.
    /// Adds "additionalProperties": false if not already present.
    /// Ensures "required" array exists (empty if missing).
    fn harden_tool_schema(mut schema: Value) -> Value {
        if let Some(obj) = schema.as_object_mut() {
            if obj.get("type").and_then(|v| v.as_str()) == Some("object") {
                obj.entry("additionalProperties")
                    .or_insert(Value::Bool(false));
                obj.entry("required")
                    .or_insert_with(|| json!([]));
            }
        }
        schema
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
            let schema = tool.parameters_schema()
                .map(Self::harden_tool_schema)
                .unwrap_or_else(|| json!({"type": "object", "properties": {}, "additionalProperties": false, "required": []}));

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

                    // Normalize parameters to OpenAI format and harden schema
                    let parameters = Self::harden_tool_schema(
                        Self::normalize_mcp_parameters(mcp_tool.parameters)
                    );

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

// ============================================================================
// PROGRESS TRACKER
// ============================================================================

/// Tracks execution progress to distinguish productive work from stuck loops.
///
/// Used by the executor to decide whether to auto-extend iterations when
/// `max_iterations` is reached. Scores each iteration based on tool diversity,
/// success rate, and repetition patterns.
struct ProgressTracker {
    /// Recent tool calls as (name, args_hash) for repetition detection
    recent_tool_calls: VecDeque<(String, u64)>,
    /// Recent error messages for repeated-error detection
    recent_errors: VecDeque<String>,
    /// Unique tool names used during this scoring window
    unique_tools_used: HashSet<String>,
    /// Cumulative progress score for the current window
    score: i32,
    /// Number of auto-extensions granted so far
    extensions_granted: u32,
    /// Maximum extensions allowed
    max_extensions: u32,
    /// Total iterations consumed across all windows
    total_iterations: u32,
    /// Rolling window of tool names (last 20 calls) for diversity tracking
    tool_name_window: VecDeque<String>,
    /// Count of tool calls in current scoring window (for periodic diversity scoring)
    window_tool_calls: u32,
}

impl ProgressTracker {
    fn new(max_extensions: u32) -> Self {
        Self {
            recent_tool_calls: VecDeque::with_capacity(10),
            recent_errors: VecDeque::with_capacity(5),
            unique_tools_used: HashSet::new(),
            score: 0,
            extensions_granted: 0,
            max_extensions,
            total_iterations: 0,
            tool_name_window: VecDeque::with_capacity(20),
            window_tool_calls: 0,
        }
    }

    fn hash_args(args: &Value) -> u64 {
        let s = serde_json::to_string(args).unwrap_or_default();
        let mut hash: u64 = 0;
        for b in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        hash
    }

    /// Record a tool call and update the progress score.
    fn record_tool_call(&mut self, name: &str, args: &Value, succeeded: bool) {
        let args_hash = Self::hash_args(args);

        // Exact repetition detection — same tool+args in last 5 calls
        let is_exact_repeat = self
            .recent_tool_calls
            .iter()
            .any(|(n, h)| n == name && *h == args_hash);
        if is_exact_repeat {
            self.score -= 3;
        }

        // Tool diversity scoring via rolling window
        self.tool_name_window.push_back(name.to_string());
        if self.tool_name_window.len() > 20 {
            self.tool_name_window.pop_front();
        }

        // Score diversity every 10 calls once we have enough data
        self.window_tool_calls += 1;
        if self.window_tool_calls % 10 == 0 && self.tool_name_window.len() >= 10 {
            let distinct: HashSet<&str> = self.tool_name_window.iter().map(|s| s.as_str()).collect();
            let ratio = distinct.len() as f32 / self.tool_name_window.len() as f32;

            if ratio <= 0.15 {
                // 1-2 unique tools in 20 calls — definitely stuck
                self.score -= 8;
            } else if ratio <= 0.25 {
                // 3-5 unique tools in 20 calls — suspicious
                self.score -= 3;
            } else {
                // Good diversity
                self.score += 2;
            }
        }

        // First-ever use of a tool gets a small bonus
        if self.unique_tools_used.insert(name.to_string()) {
            self.score += 1;
        }

        if succeeded {
            self.score += 1;
        }

        // Track for exact-repetition detection (keep last 5)
        self.recent_tool_calls.push_back((name.to_string(), args_hash));
        if self.recent_tool_calls.len() > 5 {
            self.recent_tool_calls.pop_front();
        }
    }

    /// Record a tool error for repeated-error detection.
    fn record_error(&mut self, error: &str) {
        // Check if this exact error appeared 3+ times recently
        let repeat_count = self.recent_errors.iter().filter(|e| e.as_str() == error).count();
        if repeat_count >= 2 {
            self.score -= 5; // Definitely stuck
        }

        self.recent_errors.push_back(error.to_string());
        if self.recent_errors.len() > 5 {
            self.recent_errors.pop_front();
        }
    }

    /// Record that a respond action was emitted — agent is finishing.
    fn record_respond(&mut self) {
        self.score += 10;
    }

    /// Record one iteration consumed.
    fn tick(&mut self) {
        self.total_iterations += 1;
    }

    /// Whether an auto-extension should be granted.
    fn should_extend(&self) -> bool {
        self.score > 0 && self.extensions_granted < self.max_extensions
    }

    /// Check if the agent is clearly stuck and should stop early (before window boundary).
    /// Returns true if score has gone deeply negative after at least 15 iterations in this window.
    fn is_clearly_stuck(&self) -> bool {
        self.window_tool_calls >= 15 && self.score <= -10
    }

    /// Grant an extension: reset the score window and increment counter.
    /// NOTE: tool_name_window is NOT cleared — diversity tracking spans full session.
    fn grant_extension(&mut self) {
        self.extensions_granted += 1;
        self.score = 0;
        self.unique_tools_used.clear();
        self.recent_tool_calls.clear();
        self.recent_errors.clear();
        self.window_tool_calls = 0;
    }

    /// Build a human-readable diagnosis of the current state.
    fn diagnosis(&self) -> String {
        if self.score <= -10 {
            format!(
                "Stuck in loop: {} repeated tool calls detected (score: {})",
                self.recent_tool_calls.len(),
                self.score
            )
        } else if self.score <= 0 {
            format!(
                "No progress detected after {} iterations (score: {})",
                self.total_iterations, self.score
            )
        } else {
            format!(
                "Making progress: {} unique tools used (score: {})",
                self.unique_tools_used.len(),
                self.score
            )
        }
    }

    /// Build a reason string for the extension event.
    fn extension_reason(&self) -> String {
        format!(
            "Making progress: {} unique tools used, score {} (extension {}/{})",
            self.unique_tools_used.len(),
            self.score,
            self.extensions_granted + 1,
            self.max_extensions
        )
    }
}

/// Executor errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    /// Maximum iterations reached with no progress detected.
    #[error("Maximum iterations reached")]
    MaxIterationsReached,

    /// Maximum iterations reached but agent needs user intervention.
    #[error("Max iterations reached after {iterations_used} iterations: {reason}")]
    MaxIterationsNeedsIntervention {
        /// Total iterations consumed
        iterations_used: u32,
        /// Diagnosis of why the agent stopped
        reason: String,
    },

    /// LLM API error.
    #[error("LLM error: {0}")]
    LlmError(String),

    /// Tool execution error.
    #[error("Tool error: {0}")]
    ToolError(String),

    /// MCP server error.
    #[error("MCP error: {0}")]
    McpError(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Middleware pipeline error.
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

/// Truncate a tool result string if it exceeds max_chars.
///
/// Keeps the first ~80% and last ~20% of the budget with a truncation notice.
/// Returns the original string if within limits or if max_chars is 0 (disabled).
fn truncate_tool_result(result: String, max_chars: usize) -> String {
    if max_chars == 0 || result.len() <= max_chars {
        return result;
    }

    let notice = format!(
        "\n\n--- TRUNCATED ({} chars total, showing first and last portions) ---\n\n",
        result.len()
    );
    let budget = max_chars.saturating_sub(notice.len());
    let head_size = (budget * 4) / 5; // 80%
    let tail_size = budget - head_size; // 20%

    let head = &result[..head_size];
    let tail = &result[result.len() - tail_size..];

    format!("{}{}{}", head, notice, tail)
}

#[cfg(test)]
mod truncation_tests {
    use super::*;

    #[test]
    fn test_truncate_tool_result_under_limit() {
        let result = "hello world".to_string();
        assert_eq!(truncate_tool_result(result.clone(), 100), result);
    }

    #[test]
    fn test_truncate_tool_result_disabled() {
        let result = "a".repeat(50_000);
        assert_eq!(truncate_tool_result(result.clone(), 0), result);
    }

    #[test]
    fn test_truncate_tool_result_over_limit() {
        let result = "a".repeat(1000) + &"b".repeat(1000);
        let truncated = truncate_tool_result(result, 500);
        assert!(truncated.len() <= 500);
        assert!(truncated.contains("TRUNCATED"));
        assert!(truncated.starts_with("aaa"));
        assert!(truncated.ends_with("bbb"));
    }

    #[test]
    fn test_truncate_tool_result_preserves_head_tail_ratio() {
        let result = "H".repeat(10_000) + &"T".repeat(10_000);
        let truncated = truncate_tool_result(result, 1000);
        // Head should be ~80%, tail ~20% of budget
        let head_h = truncated.matches('H').count();
        let tail_t = truncated.matches('T').count();
        assert!(head_h > tail_t, "head ({}) should be larger than tail ({})", head_h, tail_t);
    }

    #[test]
    fn test_truncate_tool_args_small() {
        let args = json!({"key": "value"});
        let result = truncate_tool_args(&args, 500);
        assert_eq!(result, args);
    }

    #[test]
    fn test_truncate_tool_args_large_string() {
        let args = json!({"content": "x".repeat(500)});
        let result = truncate_tool_args(&args, 100);
        let content = result.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("truncated"));
    }
}

#[cfg(test)]
mod progress_tracker_tests {
    use super::*;

    #[test]
    fn test_new_tracker_no_extension() {
        let tracker = ProgressTracker::new(3);
        assert!(!tracker.should_extend(), "Empty tracker should not extend");
    }

    #[test]
    fn test_unique_tools_grant_extension() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("read", &json!({"path": "/a"}), true);
        tracker.record_tool_call("write", &json!({"path": "/b"}), true);
        tracker.record_tool_call("shell", &json!({"cmd": "ls"}), true);
        // 3 unique tools: +1 each = 3, 3 succeeded: +1 each = 3, total = 6
        assert!(tracker.should_extend());
    }

    #[test]
    fn test_repeated_calls_prevent_extension() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        // Same tool+args 5 times
        for _ in 0..5 {
            tracker.record_tool_call("read", &args, true);
        }
        // First call: +1 (unique) +1 (success) = 2
        // Subsequent 4 calls: -3 (repeat) +1 (success) = -2 each = -8
        // Total: 2 + (-8) = -6
        assert!(!tracker.should_extend());
    }

    #[test]
    fn test_repeated_errors_prevent_extension() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("shell", &json!({"cmd": "fail"}), false);
        tracker.record_error("connection refused");
        tracker.record_error("connection refused");
        tracker.record_error("connection refused"); // 3rd time: -5
        // tool call: +1 (unique) +0 (failed) = 1
        // errors: -5
        // total: 1 - 5 = -4
        assert!(!tracker.should_extend());
    }

    #[test]
    fn test_respond_boosts_score() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_respond();
        // +10 from respond
        assert!(tracker.should_extend());
    }

    #[test]
    fn test_max_extensions_respected() {
        let mut tracker = ProgressTracker::new(2);
        tracker.record_respond(); // +10
        assert!(tracker.should_extend());
        tracker.grant_extension();

        tracker.record_respond(); // +10 (fresh window)
        assert!(tracker.should_extend());
        tracker.grant_extension();

        tracker.record_respond(); // +10 (fresh window)
        assert!(!tracker.should_extend(), "Should not extend beyond max_extensions=2");
    }

    #[test]
    fn test_grant_extension_resets_window() {
        let mut tracker = ProgressTracker::new(3);
        tracker.record_tool_call("read", &json!({}), true); // +2
        tracker.grant_extension();
        // After grant, score=0, unique_tools cleared, window_tool_calls reset
        assert!(!tracker.should_extend(), "Score reset to 0 after grant");
        assert_eq!(tracker.extensions_granted, 1);
        assert_eq!(tracker.window_tool_calls, 0);
    }

    #[test]
    fn test_diagnosis_stuck() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        for _ in 0..6 {
            tracker.record_tool_call("read", &args, true);
        }
        let diagnosis = tracker.diagnosis();
        assert!(
            diagnosis.contains("loop") || diagnosis.contains("No progress"),
            "Got: {}",
            diagnosis
        );
    }

    #[test]
    fn test_diagnosis_progress() {
        let mut tracker = ProgressTracker::new(3);
        // Use enough diverse tools to stay positive
        tracker.record_tool_call("read", &json!({}), true);
        tracker.record_tool_call("write", &json!({}), true);
        tracker.record_tool_call("shell", &json!({}), true);
        let diagnosis = tracker.diagnosis();
        assert!(diagnosis.contains("progress"), "Got: {}", diagnosis);
    }

    #[test]
    fn test_executor_config_defaults() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.max_extensions, 3);
        assert_eq!(config.extension_size, 25);
    }

    #[test]
    fn test_low_diversity_loop_detected() {
        let mut tracker = ProgressTracker::new(3);
        // Simulate write+shell loop for 20 iterations (different args each time)
        for i in 0..20 {
            let tool = if i % 2 == 0 { "write" } else { "shell" };
            tracker.record_tool_call(tool, &json!({"i": i}), true);
        }
        // After 20 calls: 2 unique tools (+1 each = +2), 20 successes (+20),
        // At call 10: diversity = 2/10 = 0.20 <= 0.15? No. <= 0.25? Yes → -3
        // At call 20: diversity = 2/20 = 0.10 <= 0.15 → -8
        // Total: +2 + 20 - 3 - 8 = 11 — but no exact repeats since args differ
        // The diversity penalties bring it down
        assert!(
            tracker.score < 15,
            "Low-diversity loop should have reduced score, got: {}",
            tracker.score
        );
    }

    #[test]
    fn test_high_diversity_extends() {
        let mut tracker = ProgressTracker::new(3);
        // Use 7 unique tools in 10 calls
        let tools = ["read", "write", "shell", "edit", "grep", "glob", "memory", "todo", "ward", "respond"];
        for (i, tool) in tools.iter().enumerate() {
            tracker.record_tool_call(tool, &json!({"i": i}), true);
        }
        // 10 unique tools: +1 each = 10, 10 successes = +10
        // At call 10: diversity = 10/10 = 1.0 > 0.25 → +2
        // Total: 10 + 10 + 2 = 22
        assert!(tracker.score > 0, "High diversity should produce positive score, got: {}", tracker.score);
        assert!(tracker.should_extend(), "High diversity should allow extension");
    }

    #[test]
    fn test_early_stop_deeply_stuck() {
        let mut tracker = ProgressTracker::new(3);
        let args = json!({"path": "/same"});
        // Same exact tool+args repeated — triggers both repetition penalty and diversity penalty
        for _ in 0..20 {
            tracker.record_tool_call("read", &args, true);
        }
        // With 15+ window_tool_calls and deeply negative score, should be stuck
        assert!(
            tracker.window_tool_calls >= 15,
            "Should have 20 window_tool_calls, got: {}",
            tracker.window_tool_calls
        );
        assert!(
            tracker.score <= -10,
            "Score should be <= -10 with exact-repeat loop, got: {}",
            tracker.score
        );
        assert!(
            tracker.is_clearly_stuck(),
            "Should be clearly stuck with score {} after {} calls",
            tracker.score,
            tracker.window_tool_calls
        );
    }

    #[test]
    fn test_tool_name_window_preserved_across_extensions() {
        let mut tracker = ProgressTracker::new(3);
        // Add some tool calls to fill the name window
        for i in 0..10 {
            let tool = if i % 2 == 0 { "write" } else { "shell" };
            tracker.record_tool_call(tool, &json!({"i": i}), true);
        }
        assert_eq!(tracker.tool_name_window.len(), 10);

        // Grant extension
        tracker.grant_extension();

        // tool_name_window should be preserved
        assert_eq!(
            tracker.tool_name_window.len(),
            10,
            "tool_name_window should survive grant_extension"
        );
        // But window_tool_calls should reset
        assert_eq!(tracker.window_tool_calls, 0);
        // And score should reset
        assert_eq!(tracker.score, 0);
    }
}
