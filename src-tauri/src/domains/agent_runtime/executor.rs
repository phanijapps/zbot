// ============================================================================
// TAURI AGENT EXECUTOR FACTORY
// Loads config and creates executor using agent-runtime library
// ============================================================================

//! Tauri-specific executor factory that handles file-based config loading
//!
//! # DEPRECATED
//!
//! This executor is deprecated and will be removed once the zero-app integration is complete.
//! Use `executor_v2::create_zero_executor` instead.

use std::sync::Arc;
use serde_json::{json, Value};
use rust_embed::RustEmbed;

use crate::settings::AppDirs;
use crate::domains::agent_runtime::filesystem::TauriFileSystemContext;
use agent_runtime::{
    ExecutorConfig, ExecutorError,
    LlmClient, LlmConfig, OpenAiClient,
    ToolRegistry, McpManager, MiddlewarePipeline,
    MiddlewareConfig, SummarizationMiddleware, ContextEditingMiddleware,
    PreProcessMiddleware,
};

// Re-export AgentExecutor from library
pub use agent_runtime::AgentExecutor;

// Template embedding
#[derive(RustEmbed)]
#[folder = "templates/"]
struct Assets;

// ============================================================================
// TAURI EXECUTOR FACTORY
// ============================================================================

/// Create an agent executor from an agent ID
///
/// This function loads configuration from:
/// - `config_dir/agents/{agent_id}/config.yaml` - Agent configuration
/// - `config_dir/providers.json` - LLM provider credentials
/// - `config_dir/agents/{agent_id}/AGENTS.md` - System instructions
/// - `config_dir/skills/{skill_id}/SKILL.md` - Skill definitions
pub async fn create_executor(agent_id: &str, conversation_id: Option<String>) -> Result<AgentExecutor, String> {
    // Load AppDirs
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;

    // Load agent configuration
    let agent_dir = dirs.config_dir.join("agents").join(agent_id);
    let config_file = agent_dir.join("config.yaml");
    if !config_file.exists() {
        return Err(format!("Agent config not found: {}", config_file.display()));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| format!("Failed to read agent config: {}", e))?;

    let agent_config: serde_yaml::Value = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse agent config: {}", e))?;

    // Extract configuration
    let provider_id = agent_config.get("providerId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Agent missing providerId"))?
        .to_string();

    let model = agent_config.get("model")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Agent missing model"))?
        .to_string();

    let temperature = agent_config.get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let max_tokens = agent_config.get("maxTokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(16000) as u32;

    let thinking_enabled = agent_config.get("thinkingEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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

    // Create tool registry with Tauri file system context first
    // Note: TauriFileSystemContext wraps dirs in Arc, so we can still use dirs later
    tracing::info!("Creating executor for agent {} with conversation_id: {:?}", agent_id, conversation_id);
    let fs_context = TauriFileSystemContext::new(dirs);
    let fs_context = if let Some(conv_id) = &conversation_id {
        fs_context.with_conversation(conv_id.clone())
    } else {
        tracing::warn!("No conversation_id provided - tools will not be scoped to a conversation directory");
        fs_context
    };

    // Get dirs Arc before wrapping fs_context in Arc (cheap clone since Arc)
    let dirs_arc = fs_context.dirs_arc();

    // DEPRECATED: Old executor is not compatible with new zero-app tools
    // Use executor_v2::create_zero_executor instead
    // For now, create an empty tool registry
    let tool_registry = Arc::new(ToolRegistry::default());

    // Build system prompt with skills, tools, and MCPs (pass tool_registry)
    let system_instruction = build_system_prompt(agent_id, &skills, &mcps, &conversation_id, &dirs_arc, &tool_registry).await?;

    // Create executor config
    let config = ExecutorConfig {
        agent_id: agent_id.to_string(),
        provider_id: provider_id.clone(),
        model,
        temperature,
        max_tokens,
        thinking_enabled,
        system_instruction,
        tools_enabled: true,
        mcps,
        skills,
        conversation_id: conversation_id.clone(),
    };

    // Create LLM client
    let llm_client = create_llm_client(&provider_id, &config).await?;

    // Create MCP manager and load servers
    let mcp_manager = Arc::new(McpManager::default());
    if !config.mcps.is_empty() {
        tracing::info!("Loading MCP servers: {:?}", config.mcps);
        // Load MCP server configs from file
        if let Err(e) = load_and_start_mcp_servers(&mcp_manager, &config.mcps, &dirs_arc).await {
            tracing::warn!("Failed to load MCP servers: {}", e);
        }
    }

    // Create middleware pipeline
    let mut pipeline = MiddlewarePipeline::new();

    // Parse and create middleware if configured
    if let Some(middleware_value) = agent_config.get("middleware") {
        let middleware_yaml = middleware_value
            .as_str()
            .ok_or_else(|| format!("Middleware config must be a string, found: {:?}", middleware_value))?;

        let middleware_config: MiddlewareConfig = serde_yaml::from_str(middleware_yaml)
            .map_err(|e| format!("Failed to parse middleware config: {}", e))?;

        // Add summarization middleware if configured and enabled
        if let Some(summarization_config) = middleware_config.summarization {
            if summarization_config.enabled {
                let summary_provider_id = summarization_config.provider
                    .clone()
                    .unwrap_or_else(|| provider_id.clone());

                let (api_key, base_url) = load_provider_credentials(&summary_provider_id).await?;

                let middleware = SummarizationMiddleware::from_config(
                    summarization_config,
                    &summary_provider_id,
                    None, // Use agent's model by default
                    &config.model,
                    api_key,
                    base_url,
                ).await?;

                pipeline = pipeline.add_pre_processor(Box::new(middleware));
            }
        }

        // Add context editing middleware if configured and enabled
        if let Some(context_editing_config) = middleware_config.context_editing {
            if context_editing_config.enabled {
                let middleware = ContextEditingMiddleware::new(context_editing_config);
                pipeline = pipeline.add_pre_processor(Box::new(middleware));
            }
        }
    }

    // Create executor
    AgentExecutor::new(
        config,
        llm_client,
        tool_registry,
        mcp_manager,
        Arc::new(pipeline),
    ).map_err(|e| match e {
        ExecutorError::MaxIterationsReached => e.to_string(),
        ExecutorError::LlmError(s) => s,
        ExecutorError::ToolError(s) => s,
        ExecutorError::McpError(s) => s,
        ExecutorError::ConfigError(s) => s,
        ExecutorError::MiddlewareError(s) => s,
    })
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

async fn create_llm_client(provider_id: &str, config: &ExecutorConfig) -> Result<Arc<dyn LlmClient>, String> {
    let (api_key, base_url) = load_provider_credentials(provider_id).await?;

    let llm_config = LlmConfig {
        provider_id: provider_id.to_string(),
        api_key,
        base_url,
        model: config.model.clone(),
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        thinking_enabled: config.thinking_enabled,
    };

    let client = OpenAiClient::new(llm_config)
        .map_err(|e| format!("Failed to create LLM client: {}", e))?;

    Ok(Arc::new(client))
}

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

async fn build_system_prompt(
    agent_id: &str,
    skills: &[String],
    mcps: &[String],
    conversation_id: &Option<String>,
    dirs: &Arc<AppDirs>,
    tool_registry: &ToolRegistry,
) -> Result<Option<String>, String> {
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

    // Build available_tools XML (built-in tools) - use the provided tool_registry
    let available_tools_xml = {
        let mut tools_xml = String::from("<available_tools>\n");
        let all_tools = tool_registry.get_all();
        tracing::info!("Building system prompt with {} built-in tools", all_tools.len());
        for tool in all_tools {
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
// MCP SERVER LOADING
// ============================================================================

/// Load and start MCP servers from the config file
async fn load_and_start_mcp_servers(
    mcp_manager: &McpManager,
    agent_mcps: &[String],
    dirs: &Arc<AppDirs>,
) -> Result<(), String> {
    let mcp_file = dirs.config_dir.join("mcps.json");

    tracing::info!("Loading MCP servers from: {:?}", mcp_file);
    tracing::info!("Agent MCPs: {:?}", agent_mcps);

    if !mcp_file.exists() {
        tracing::info!("MCP servers file does not exist");
        return Ok(()); // No MCP servers configured
    }

    let content = std::fs::read_to_string(&mcp_file)
        .map_err(|e| format!("Failed to read MCP servers file: {}", e))?;

    // Support both array format and single object format
    let servers: Vec<agent_runtime::McpServerConfig> = if content.trim().starts_with('[') {
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP servers array: {}", e))?
    } else {
        // Single object - wrap in array
        let server: agent_runtime::McpServerConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse MCP server: {}", e))?;
        vec![server]
    };

    tracing::info!("Found {} MCP servers in config", servers.len());

    for server_config in servers {
        let id = server_config.id();
        let name = server_config.name().to_string();
        let enabled = server_config.enabled();

        tracing::info!("Checking server: id={}, name={}, enabled={}", id, name, enabled);

        if agent_mcps.contains(&id) {
            // Start this MCP server since the agent explicitly uses it
            tracing::info!("Starting MCP server: {} (required by agent)", id);
            mcp_manager.start_server(server_config).await
                .map_err(|e| format!("Failed to start MCP server {}: {}", id, e))?;
        } else if enabled {
            // Also start if it's globally enabled
            tracing::info!("Starting MCP server: {} (globally enabled)", id);
            mcp_manager.start_server(server_config).await
                .map_err(|e| format!("Failed to start MCP server {}: {}", id, e))?;
        } else {
            tracing::info!("Skipping MCP server {} (not used by agent and not enabled)", id);
        }
    }

    Ok(())
}
