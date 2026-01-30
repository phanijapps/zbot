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

use crate::types::{ChatMessage, StreamEvent};
use crate::llm::LlmClient;
use crate::tools::ToolRegistry;
use crate::tools::context::ToolContext;
use crate::mcp::McpManager;
use crate::middleware::MiddlewarePipeline;
use crate::middleware::traits::MiddlewareContext;
use crate::middleware::token_counter::estimate_total_tokens;

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
        }
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
        let middleware_context = MiddlewareContext::new(
            self.config.agent_id.clone(),
            self.config.conversation_id.clone(),
            self.config.provider_id.clone(),
            self.config.model.clone(),
        )
        .with_counts(message_count, estimated_tokens);

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

        loop {
            if max_iterations == 0 {
                return Err(ExecutorError::MaxIterationsReached);
            }
            max_iterations -= 1;

            // Make LLM call
            let response = self.llm_client
                .chat(current_messages.clone(), tools_schema.clone())
                .await
                .map_err(|e| ExecutorError::LlmError(e.to_string()))?;

            tracing::debug!("LLM response - content: '{}', tool_calls: {}",
                response.content, response.tool_calls.as_ref().map_or(0, |v| v.len()));

            // Emit reasoning event if available (for DeepSeek, GLM, etc.)
            if let Some(reasoning) = &response.reasoning {
                tracing::debug!("Emitting reasoning event, length: {}", reasoning.len());
                on_event(StreamEvent::Reasoning {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    content: reasoning.clone(),
                });
            }

            // Check for tool calls
            let tool_calls = response.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                // No tool calls, this is the final response
                full_response = response.content.clone();
                tracing::debug!("No tool calls, final response: {}", full_response);

                // Stream token events
                for ch in response.content.chars() {
                    on_event(StreamEvent::Token {
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        content: ch.to_string(),
                    });
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }

                break;
            }

            // Handle tool calls
            // Add the assistant message with all tool calls
            current_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: response.content.clone(),
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            });

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

                let result = self.execute_tool(tool_name, args).await;

                match result {
                    Ok(output) => {
                        tracing::debug!("Tool result: {}", output);

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

                        // Add tool result message
                        current_messages.push(ChatMessage {
                            role: "tool".to_string(),
                            content: output,
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
        }

        // Emit done event
        on_event(StreamEvent::Done {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            final_message: full_response.clone(),
            token_count: full_response.len(),
        });

        Ok(())
    }

    async fn execute_tool(&self, tool_name: &str, arguments: &Value) -> Result<String, String> {
        // First try built-in tools
        if let Some(tool) = self.tool_registry.find(tool_name) {
            // Create tool context with agent_id, conversation_id, and skills
            // This ensures tools like memory can find the agent's data directory
            let ctx = Arc::new(ToolContext::full(
                self.config.agent_id.clone(),
                self.config.conversation_id.clone(),
                self.config.skills.clone(),
            ));
            let result = tool.execute(ctx, arguments.clone()).await
                .map_err(|e| format!("Tool execution failed: {:?}", e))?;
            return Ok(serde_json::to_string(&result)
                .unwrap_or_else(|_| "null".to_string()));
        }

        // Try MCP tools
        // MCP tools have format: {normalized_server_id}__{tool_name}
        if tool_name.contains("__") {
            let parts: Vec<&str> = tool_name.splitn(2, "__").collect();
            if parts.len() == 2 {
                let normalized_server_id = parts[0];
                let actual_tool = parts[1];
                // Convert normalized ID back to original (replace _ with -)
                let server_id = normalized_server_id.replace('_', "-");
                tracing::debug!("Executing MCP tool: server={}, tool={}", server_id, actual_tool);
                return self.mcp_manager.execute_tool(&server_id, actual_tool, arguments.clone()).await
                    .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "null".to_string()))
                    .map_err(|e| e.to_string());
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
                    let mcp_id_normalized = mcp_id.replace('-', "_");
                    let tool_name = format!("{}__{}", mcp_id_normalized, mcp_tool.name);

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
