// ============================================================================
// AGENT EXECUTOR
// Simplified agent executor with tool support
// ============================================================================

use std::sync::Arc;
use serde_json::{json, Value};

use crate::domains::agent_runtime::llm::{LlmClient, ChatMessage, OpenAiClient};
use crate::domains::agent_runtime::tools::{ToolRegistry, ToolContext};
use crate::domains::agent_runtime::mcp_manager::McpManager;
use crate::settings::AppDirs;

// ============================================================================
// AGENT EXECOR CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    pub agent_id: String,
    pub provider_id: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub system_instruction: Option<String>,
    pub tools_enabled: bool,
    pub mcps: Vec<String>,
}

// ============================================================================
// STREAM EVENTS
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "metadata")]
    Metadata {
        timestamp: u64,
        agent_id: String,
        model: String,
        provider: String,
    },
    #[serde(rename = "token")]
    Token {
        timestamp: u64,
        content: String,
    },
    #[serde(rename = "tool_call_start")]
    ToolCallStart {
        timestamp: u64,
        tool_id: String,
        tool_name: String,
        args: Value,
    },
    #[serde(rename = "tool_call_end")]
    ToolCallEnd {
        timestamp: u64,
        tool_id: String,
        tool_name: String,
        args: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        timestamp: u64,
        tool_id: String,
        result: String,
        error: Option<String>,
    },
    #[serde(rename = "done")]
    Done {
        timestamp: u64,
        final_message: String,
        token_count: usize,
    },
    #[serde(rename = "error")]
    Error {
        timestamp: u64,
        error: String,
        recoverable: bool,
    },
}

// ============================================================================
// AGENT EXECUTOR
// ============================================================================

pub struct AgentExecutor {
    config: ExecutorConfig,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    mcp_manager: Arc<McpManager>,
}

impl AgentExecutor {
    pub async fn new(config: ExecutorConfig) -> Result<Self, String> {
        // Create LLM client
        let llm_client = Self::create_llm_client(&config).await?;

        // Create tool registry
        let tool_registry = Arc::new(ToolRegistry::default());

        // Create MCP manager
        let mcp_manager = Arc::new(McpManager::default());

        // Load MCP servers if configured
        if !config.mcps.is_empty() {
            eprintln!("Loading MCP servers: {:?}", config.mcps);
            if let Err(e) = mcp_manager.load_servers(&config.mcps).await {
                eprintln!("Failed to load MCP servers: {}", e);
            }
        }

        Ok(Self {
            config,
            llm_client,
            tool_registry,
            mcp_manager,
        })
    }

    async fn create_llm_client(config: &ExecutorConfig) -> Result<Arc<dyn LlmClient>, String> {
        let dirs = AppDirs::get().map_err(|e| e.to_string())?;
        let providers_file = dirs.config_dir.join("providers.json");

        let content = std::fs::read_to_string(&providers_file)
            .map_err(|e| format!("Failed to read providers file: {}", e))?;

        let providers: Vec<Value> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse providers: {}", e))?;

        let provider = providers
            .into_iter()
            .find(|p| p.get("id").and_then(|i| i.as_str()) == Some(config.provider_id.as_str()))
            .ok_or_else(|| format!("Provider not found: {}", config.provider_id))?;

        let api_key = provider.get("apiKey")
            .and_then(|k| k.as_str())
            .ok_or_else(|| format!("Provider missing apiKey"))?
            .to_string();

        let base_url = provider.get("baseUrl")
            .and_then(|u| u.as_str())
            .ok_or_else(|| format!("Provider missing baseUrl"))?
            .to_string();

        let llm_config = crate::domains::agent_runtime::llm::LlmConfig {
            provider_id: config.provider_id.clone(),
            api_key,
            base_url,
            model: config.model.clone(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
        };

        Ok(Arc::new(OpenAiClient::new(llm_config)))
    }

    /// Execute agent with streaming support
    pub async fn execute_stream(
        &self,
        user_message: &str,
        history: &[ChatMessage],
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<(), String> {
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

        // Get tools schema if enabled
        let tools_schema = if self.config.tools_enabled {
            Some(self.build_tools_schema().await?)
        } else {
            None
        };

        eprintln!("About to call execute_with_tools_loop");
        // Execute with tool calling loop
        let result = self.execute_with_tools_loop(messages, tools_schema, &mut on_event).await;
        eprintln!("execute_with_tools_loop returned: {:?}", result);
        result
    }

    async fn execute_with_tools_loop(
        &self,
        messages: Vec<ChatMessage>,
        tools_schema: Option<Value>,
        on_event: &mut impl FnMut(StreamEvent),
    ) -> Result<(), String> {
        eprintln!("=== execute_with_tools_loop starting ===");
        eprintln!("Messages count: {}", messages.len());
        eprintln!("Tools schema: {}", tools_schema.is_some());

        let mut current_messages = messages;
        let mut max_iterations = 10; // Prevent infinite loops
        let mut full_response = String::new();

        loop {
            eprintln!("=== Loop iteration, remaining: {} ===", max_iterations);
            if max_iterations == 0 {
                return Err("Maximum tool call iterations reached".to_string());
            }
            max_iterations -= 1;

            eprintln!("Calling LLM with {} messages", current_messages.len());
            // Make LLM call
            let response = self.llm_client.chat(current_messages.clone(), tools_schema.clone()).await?;

            eprintln!("LLM response - content: '{}', tool_calls: {}", response.content, response.tool_calls.len());

            // Check for tool calls
            if response.tool_calls.is_empty() {
                // No tool calls, this is the final response
                full_response = response.content.clone();
                eprintln!("No tool calls, final response: {}", full_response);

                // Stream token events with small delay for visual effect
                for ch in response.content.chars() {
                    on_event(StreamEvent::Token {
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        content: ch.to_string(),
                    });
                    // Small delay to allow frontend to render each token
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }

                break;
            }

            // Handle tool calls
            // First, add the assistant message with all tool calls
            if !response.tool_calls.is_empty() {
                current_messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: response.content.clone(),
                    tool_calls: Some(response.tool_calls.clone()),
                    tool_call_id: None,
                });
            }

            // Then execute each tool and add its result
            for tool_call in &response.tool_calls {
                let tool_name = tool_call.name();
                let args = tool_call.arguments().unwrap_or_else(|_| json!({}));

                eprintln!("Executing tool: {} with args: {}", tool_name, args);

                on_event(StreamEvent::ToolCallStart {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    tool_id: tool_call.id.clone(),
                    tool_name: tool_name.to_string(),
                    args: args.clone(),
                });

                let result = self.execute_tool(tool_name, &args).await;

                match result {
                    Ok(output) => {
                        eprintln!("Tool result: {}", output);
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
                        eprintln!("Tool error: {}", e);
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
                    tool_name: tool_name.to_string(),
                    args,
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
            let ctx = Arc::new(ToolContext::new());
            let result = tool.execute(ctx, arguments.clone()).await
                .map_err(|e| format!("Tool execution failed: {:?}", e))?;
            return Ok(serde_json::to_string(&result)
                .unwrap_or_else(|_| "null".to_string()));
        }

        // Try MCP tools
        // MCP tools have format: {normalized_server_id}__{tool_name}
        // e.g., "time_server__get_current_time" -> "time-server" + "get_current_time"
        if tool_name.contains("__") {
            let parts: Vec<&str> = tool_name.splitn(2, "__").collect();
            if parts.len() == 2 {
                let normalized_server_id = parts[0];
                let actual_tool = parts[1];
                // Convert normalized ID back to original (replace _ with -)
                let server_id = normalized_server_id.replace('_', "-");
                eprintln!("Executing MCP tool: server={}, tool={}", server_id, actual_tool);
                return self.mcp_manager.execute_tool(&server_id, actual_tool, arguments.clone()).await
                    .and_then(|v| Ok(serde_json::to_string(&v)
                        .unwrap_or_else(|_| "null".to_string())));
            }
        }

        Err(format!("Tool not found: {}", tool_name))
    }

    async fn build_tools_schema(&self) -> Result<Value, String> {
        let mut tools = Vec::new();

        for tool in self.tool_registry.get_all() {
            let schema = tool.parameters_schema().unwrap_or_else(|| json!(null));

            tools.push(json!({
                "type": "function",
                "function": {
                    "name": tool.name(),
                    "description": tool.description(),
                    "parameters": schema
                }
            }));
        }

        eprintln!("Built-in tools: {}", tools.len());

        // Add MCP tools
        eprintln!("Looking for MCP clients: {:?}", self.config.mcps);
        for mcp_id in &self.config.mcps {
            eprintln!("Getting client for: {}", mcp_id);
            if let Some(client) = self.mcp_manager.get_client(mcp_id).await {
                eprintln!("Found client for: {}", mcp_id);
                let mcp_tools = client.list_tools().await
                    .map_err(|e| format!("Failed to list MCP tools: {}", e))?;

                eprintln!("MCP tools for {}: {}", mcp_id, mcp_tools.len());

                for mcp_tool in mcp_tools {
                    // Convert MCP ID and tool name to valid OpenAI tool name format
                    // Replace hyphens with underscores and use double underscore as separator
                    let mcp_id_normalized = mcp_id.replace('-', "_");
                    let tool_name = format!("{}__{}", mcp_id_normalized, mcp_tool.name);
                    eprintln!("Adding MCP tool: {} (from {}:{})", tool_name, mcp_id, mcp_tool.name);
                    tools.push(json!({
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "description": mcp_tool.description,
                            "parameters": mcp_tool.parameters.unwrap_or_else(|| json!(null))
                        }
                    }));
                }
            } else {
                eprintln!("No client found for: {}", mcp_id);
            }
        }

        eprintln!("Total tools: {}", tools.len());
        Ok(json!(tools))
    }

    /// Execute agent without streaming (simpler API)
    pub async fn execute(
        &self,
        user_message: &str,
        history: &[ChatMessage],
    ) -> Result<String, String> {
        let mut final_response = String::new();

        self.execute_stream(user_message, history, |event| {
            if let StreamEvent::Token { content, .. } = event {
                final_response.push_str(&content);
            }
        }).await?;

        Ok(final_response)
    }
}

// ============================================================================
// FACTORY FUNCTIONS
// ============================================================================

/// Create an agent executor from an agent ID
pub async fn create_executor(agent_id: &str) -> Result<AgentExecutor, String> {
    // Load agent configuration
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let agent_dir = dirs.config_dir.join("agents").join(agent_id);

    let config_file = agent_dir.join("config.yaml");
    if !config_file.exists() {
        return Err(format!("Agent config not found: {}", config_file.display()));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read agent config: {}", e))?;

    let agent_config: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse agent config: {}", e))?;

    // Read AGENTS.md for instructions
    let agents_md = agent_dir.join("AGENTS.md");
    let system_instruction = if agents_md.exists() {
        let content = std::fs::read_to_string(&agents_md)
            .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?;
        Some(content)
    } else {
        None
    };

    // Get MCPs
    let mcps = agent_config.get("mcps")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let config = ExecutorConfig {
        agent_id: agent_id.to_string(),
        provider_id: agent_config.get("providerId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Agent missing providerId"))?
            .to_string(),
        model: agent_config.get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Agent missing model"))?
            .to_string(),
        temperature: agent_config.get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7),
        max_tokens: agent_config.get("maxTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000) as u32,
        system_instruction,
        tools_enabled: true,
        mcps,
    };

    AgentExecutor::new(config).await
}
