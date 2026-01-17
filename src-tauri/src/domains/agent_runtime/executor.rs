// ============================================================================
// AGENT EXECUTOR
// Simplified agent executor with tool support
// ============================================================================

use std::sync::Arc;
use serde_json::{json, Value};

use crate::domains::agent_runtime::llm::{LlmClient, ChatMessage, OpenAiClient};
use crate::domains::agent_runtime::tools::{ToolRegistry, ToolContext};
use crate::domains::agent_runtime::mcp_manager::{McpManager, McpTool};
use crate::domains::agent_runtime::middleware::{MiddlewarePipeline, MiddlewareContext};
use crate::domains::agent_runtime::middleware::token_counter::estimate_total_tokens;
use crate::settings::AppDirs;

// Template embedding
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "templates/"]
struct Assets;

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
    pub thinking_enabled: bool,
    pub system_instruction: Option<String>,
    pub tools_enabled: bool,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
    pub conversation_id: Option<String>,
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
    #[serde(rename = "reasoning")]
    Reasoning {
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
    // ========================================================================
    // GENERATIVE UI EVENTS
    // ========================================================================
    #[serde(rename = "show_content")]
    ShowContent {
        timestamp: u64,
        content_type: String,  // "pdf", "ppt", "html", "image", "text"
        title: String,
        content: String,       // Filename or Base64 encoded data (for backwards compatibility)
        metadata: Option<Value>,
        file_path: Option<String>,  // Path to attachment file (e.g., "conv_id/attachments/filename")
        is_attachment: Option<bool>,  // true if content is saved to attachments
        base64: Option<bool>,   // true if content is base64 encoded
    },
    #[serde(rename = "request_input")]
    RequestInput {
        timestamp: u64,
        form_id: String,
        form_type: String,     // "json_schema", "dynamic_form"
        title: String,
        description: Option<String>,
        schema: Value,         // JSON Schema for form
        submit_button: Option<String>,
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
    middleware_pipeline: Arc<MiddlewarePipeline>,
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
            middleware_pipeline: Arc::new(MiddlewarePipeline::new()),
        })
    }

    /// Set the middleware pipeline for this executor
    pub fn set_middleware_pipeline(&mut self, pipeline: Arc<MiddlewarePipeline>) {
        self.middleware_pipeline = pipeline;
    }

    /// Get a reference to the middleware pipeline
    pub fn middleware_pipeline(&self) -> &Arc<MiddlewarePipeline> {
        &self.middleware_pipeline
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
            thinking_enabled: config.thinking_enabled,
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
            .await?;

        // Get tools schema if enabled
        let tools_schema = if self.config.tools_enabled {
            Some(self.build_tools_schema().await?)
        } else {
            None
        };

        eprintln!("About to call execute_with_tools_loop");
        // Execute with tool calling loop
        let result = self.execute_with_tools_loop(processed_messages, tools_schema, &mut on_event).await;
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
        let mut max_iterations = 50; // Increased from 10 to allow more complex tool workflows
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

            // Emit reasoning event if available (for DeepSeek, GLM, etc.)
            if let Some(reasoning) = &response.reasoning_content {
                eprintln!("Emitting reasoning event, length: {}", reasoning.len());
                on_event(StreamEvent::Reasoning {
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    content: reasoning.clone(),
                });
            }

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

                        // Check for generative UI markers
                        if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                            // Check for show_content marker
                            if parsed.get("__show_content").and_then(|v| v.as_bool()).unwrap_or(false) {
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
                                let file_path = parsed.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let is_attachment = parsed.get("is_attachment").and_then(|v| v.as_bool());
                                let base64 = parsed.get("base64").and_then(|v| v.as_bool());

                                eprintln!("Emitting ShowContent event: {} (attachment: {})", title, is_attachment.unwrap_or(false));
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

                            // Check for request_input marker (for future RequestInput tool)
                            if parsed.get("__request_input").and_then(|v| v.as_bool()).unwrap_or(false) {
                                let form_id = parsed.get("form_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&format!("form_{}", chrono::Utc::now().timestamp())).to_string();
                                let form_type = parsed.get("form_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("json_schema").to_string();
                                let title = parsed.get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Input Required").to_string();
                                let description = parsed.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let schema = parsed.get("schema").cloned().unwrap_or_else(|| json!({}));
                                let submit_button = parsed.get("submit_button").and_then(|v| v.as_str()).map(|s| s.to_string());

                                eprintln!("Emitting RequestInput event: {}", title);
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
            // Create tool context with conversation ID and skills if available
            let ctx = match (&self.config.conversation_id, !self.config.skills.is_empty()) {
                (Some(conv_id), true) => Arc::new(ToolContext::with_conversation_and_skills(
                    conv_id.clone(),
                    self.config.skills.clone(),
                )),
                (Some(conv_id), false) => Arc::new(ToolContext::with_conversation(conv_id.clone())),
                (None, true) => Arc::new(ToolContext::with_skills(self.config.skills.clone())),
                (None, false) => Arc::new(ToolContext::new()),
            };
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
// SYSTEM PROMPT BUILDER
// ============================================================================

/// Build the system prompt from template with skills, tools, and MCPs
async fn build_system_prompt(
    agent_id: &str,
    skills: &[String],
    mcps: &[String],
    conversation_id: &Option<String>,
) -> Result<Option<String>, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let agent_dir = dirs.config_dir.join("agents").join(agent_id);

    // Read base instructions from AGENTS.md
    let agents_md_path = agent_dir.join("AGENTS.md");
    let base_instructions = if agents_md_path.exists() {
        std::fs::read_to_string(&agents_md_path)
            .map_err(|e| format!("Failed to read AGENTS.md: {}", e))?
    } else {
        String::new()
    };

    // Build available_skills XML
    let available_skills_xml = if skills.is_empty() {
        "<!-- No skills configured -->".to_string()
    } else {
        let mut skills_xml = String::from("<available_skills>\n");
        for skill_id in skills {
            let skill_dir = dirs.skills_dir.join(skill_id);
            let skill_md_path = skill_dir.join("SKILL.md");
            if skill_md_path.exists() {
                let content = std::fs::read_to_string(&skill_md_path)
                    .map_err(|e| format!("Failed to read SKILL.md for {}: {}", skill_id, e))?;

                // Parse frontmatter to get metadata
                if let Ok((frontmatter, _body)) = parse_skill_frontmatter_for_prompt(&content) {
                    let location = skill_md_path
                        .to_str()
                        .unwrap_or(&skill_dir.join("SKILL.md").to_string_lossy())
                        .to_string();
                    skills_xml.push_str(&format!(
                        "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n    <location>{}</location>\n  </skill>\n",
                        frontmatter.name, frontmatter.description, location
                    ));
                }
            }
        }
        skills_xml.push_str("</available_skills>");
        skills_xml
    };

    // Build available_tools XML (built-in tools)
    let tool_registry = ToolRegistry::default();
    let available_tools_xml = {
        let mut tools_xml = String::from("<available_tools>\n");
        for tool in tool_registry.get_all() {
            tools_xml.push_str(&format!(
                "  <tool>\n    <name>{}</name>\n    <description>{}</description>\n  </tool>\n",
                tool.name(),
                tool.description()
            ));
        }
        tools_xml.push_str("</available_tools>");
        tools_xml
    };

    // Build available_mcp_tools XML
    let available_mcp_tools_xml = if mcps.is_empty() {
        "<!-- No MCP servers configured -->".to_string()
    } else {
        let mut mcp_manager = McpManager::default();
        let _ = mcp_manager.load_servers(mcps).await;

        let mut mcp_tools_xml = String::from("<available_mcp_tools>\n");
        for mcp_id in mcps {
            if let Some(client) = mcp_manager.get_client(mcp_id).await {
                if let Ok(mcp_tools) = client.list_tools().await {
                    for mcp_tool in mcp_tools {
                        mcp_tools_xml.push_str(&format!(
                            "  <tool>\n    <name>{}__{}</name>\n    <description>{}</description>\n  </tool>\n",
                            mcp_id.replace('-', "_"),
                            mcp_tool.name,
                            mcp_tool.description
                        ));
                    }
                }
            }
        }
        mcp_tools_xml.push_str("</available_mcp_tools>");
        mcp_tools_xml
    };

    // Replace {CONV_ID} in template
    let conv_id = conversation_id.as_ref().map(|s| s.as_str()).unwrap_or("current");

    // Load template
    let template = Assets::get("system_prompt.md")
        .and_then(|f| String::from_utf8(f.data.into()).ok())
        .unwrap_or_else(||
            // Fallback template if embedded file not found
            "{BASE_INSTRUCTIONS}\n\n---\n\n## Available Skills\n\n{AVAILABLE_SKILLS_XML}\n\n---\n\n## Available Tools\n\n{AVAILABLE_TOOLS_XML}\n\n---\n\n## Available MCP Tools\n\n{AVAILABLE_MCP_TOOLS_XML}".to_string()
        );

    // Build final system prompt
    let system_prompt = template
        .replace("{BASE_INSTRUCTIONS}", &base_instructions)
        .replace("{AVAILABLE_SKILLS_XML}", &available_skills_xml)
        .replace("{AVAILABLE_TOOLS_XML}", &available_tools_xml)
        .replace("{AVAILABLE_MCP_TOOLS_XML}", &available_mcp_tools_xml)
        .replace("{CONV_ID}", conv_id);

    Ok(Some(system_prompt))
}

/// Parse skill frontmatter (simplified version for system prompt building)
fn parse_skill_frontmatter_for_prompt(content: &str) -> Result<(SkillFrontmatterSimple, String), String> {
    use regex::Regex;

    let frontmatter_regex = Regex::new(r"^---\n([\s\S]*?)\n---\n([\s\S]*)$")
        .map_err(|e| format!("Failed to create regex: {}", e))?;

    let captures = frontmatter_regex.captures(content)
        .ok_or_else(|| "Invalid SKILL.md format: missing frontmatter".to_string())?;

    let yaml_content = captures.get(1).unwrap().as_str();
    let body = captures.get(2).unwrap().as_str();

    let frontmatter: SkillFrontmatterSimple = serde_yaml::from_str(yaml_content)
        .map_err(|e| format!("Failed to parse frontmatter: {}", e))?;

    Ok((frontmatter, body.to_string()))
}

/// Simplified skill frontmatter for system prompt generation
#[derive(Debug, Clone, serde::Deserialize)]
struct SkillFrontmatterSimple {
    name: String,
    description: String,
}

// ============================================================================
// FACTORY FUNCTIONS
// ============================================================================

/// Create an agent executor from an agent ID
pub async fn create_executor(agent_id: &str, conversation_id: Option<String>) -> Result<AgentExecutor, String> {
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

    // Get skills
    let skills = agent_config.get("skills")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Build system prompt with skills, tools, and MCPs
    let system_instruction = build_system_prompt(agent_id, &skills, &mcps, &conversation_id).await?;

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
            .unwrap_or(16000) as u32,  // Default 16K, DeepSeek supports up to 128K
        thinking_enabled: agent_config.get("thinkingEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        system_instruction,
        tools_enabled: true,
        mcps,
        skills,
        conversation_id,
    };

    // Create executor
    let mut executor = AgentExecutor::new(config).await?;

    // Parse and create middleware if configured
    if let Some(middleware_value) = agent_config.get("middleware") {
        // Middleware is stored as a YAML string, extract and parse it
        let middleware_yaml = middleware_value
            .as_str()
            .ok_or_else(|| format!("Middleware config must be a string, found: {:?}", middleware_value))?;

        let middleware_config: crate::domains::agent_runtime::middleware::MiddlewareConfig =
            serde_yaml::from_str(middleware_yaml)
                .map_err(|e| format!("Failed to parse middleware config: {}", e))?;

        // Build middleware pipeline
        let mut pipeline = MiddlewarePipeline::new();

        // Add summarization middleware if configured and enabled
        if let Some(summarization_config) = middleware_config.summarization {
            if summarization_config.enabled {
                // Use agent's provider/model if not specified in config
                let agent_provider_id = agent_config.get("providerId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("Agent missing providerId"))?;

                let summary_provider_id = summarization_config.provider
                    .clone()
                    .unwrap_or_else(|| agent_provider_id.to_string());

                let agent_model = agent_config.get("model")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("Agent missing model"))?;

                // If model not specified, use agent's model (can be overridden in config)
                let summary_model = summarization_config.model.clone();

                let (api_key, base_url) = load_provider_credentials(&summary_provider_id).await?;

                let middleware = crate::domains::agent_runtime::middleware::SummarizationMiddleware::from_config(
                    summarization_config,
                    &summary_provider_id,
                    summary_model,
                    agent_model,
                    api_key,
                    base_url,
                ).await?;

                pipeline = pipeline.add_pre_processor(Box::new(middleware));
            }
        }

        // Add context editing middleware if configured and enabled
        if let Some(context_editing_config) = middleware_config.context_editing {
            if context_editing_config.enabled {
                let middleware = crate::domains::agent_runtime::middleware::ContextEditingMiddleware::new(
                    context_editing_config,
                );
                pipeline = pipeline.add_pre_processor(Box::new(middleware));
            }
        }

        // Set the middleware pipeline on the executor
        executor.set_middleware_pipeline(Arc::new(pipeline));
    }

    Ok(executor)
}

/// Load provider credentials for middleware LLM clients
async fn load_provider_credentials(provider_id: &str) -> Result<(String, String), String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let providers_file = dirs.config_dir.join("providers.json");

    let content = std::fs::read_to_string(&providers_file)
        .map_err(|e| format!("Failed to read providers file: {}", e))?;

    let providers: Vec<Value> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse providers: {}", e))?;

    let provider = providers
        .into_iter()
        .find(|p| p.get("id").and_then(|i| i.as_str()) == Some(provider_id))
        .ok_or_else(|| format!("Provider not found: {}", provider_id))?;

    let api_key = provider.get("apiKey")
        .and_then(|k| k.as_str())
        .ok_or_else(|| format!("Provider missing apiKey"))?
        .to_string();

    let base_url = provider.get("baseUrl")
        .and_then(|u| u.as_str())
        .ok_or_else(|| format!("Provider missing baseUrl"))?
        .to_string();

    Ok((api_key, base_url))
}
