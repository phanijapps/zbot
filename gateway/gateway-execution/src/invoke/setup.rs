//! # Setup Module
//!
//! Provider resolution and agent loading utilities for execution setup.

use gateway_services::providers::Provider;
use gateway_services::{AgentService, ProviderService, SettingsService, SharedVaultPaths};
use agent_tools::ToolSettings;
use std::sync::Arc;

// ============================================================================
// SETUP CONTEXT
// ============================================================================

/// Context for setting up agent execution.
///
/// Contains all services and configuration needed to prepare an agent
/// for execution.
#[derive(Clone)]
pub struct SetupContext {
    /// Agent service for loading agent configurations
    pub agent_service: Arc<AgentService>,
    /// Provider service for loading provider configurations
    pub provider_service: Arc<ProviderService>,
    /// Vault paths for accessing configuration and data directories
    pub paths: SharedVaultPaths,
}

impl SetupContext {
    /// Create a new setup context.
    pub fn new(
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        paths: SharedVaultPaths,
    ) -> Self {
        Self {
            agent_service,
            provider_service,
            paths,
        }
    }

    /// Get tool settings from the configuration directory.
    pub fn tool_settings(&self) -> ToolSettings {
        let settings_service = SettingsService::new(self.paths.clone());
        settings_service.get_tool_settings().unwrap_or_default()
    }
}

// ============================================================================
// PROVIDER RESOLVER
// ============================================================================

/// Resolves providers for agent execution.
pub struct ProviderResolver<'a> {
    provider_service: &'a ProviderService,
}

impl<'a> ProviderResolver<'a> {
    /// Create a new provider resolver.
    pub fn new(provider_service: &'a ProviderService) -> Self {
        Self { provider_service }
    }

    /// Get the default provider (marked as is_default) or fall back to first.
    pub fn get_default(&self) -> Result<Provider, String> {
        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("Failed to list providers: {}", e))?;

        // First try to find the provider marked as default
        if let Some(default_provider) = providers.iter().find(|p| p.is_default).cloned() {
            return Ok(default_provider);
        }

        // Fall back to first provider
        providers
            .into_iter()
            .next()
            .ok_or_else(|| "No providers configured. Add a provider in Integrations.".to_string())
    }

    /// Get provider by ID, falling back to default if not found.
    pub fn get_or_default(&self, provider_id: &str) -> Result<Provider, String> {
        if !provider_id.is_empty() {
            match self.provider_service.get(provider_id) {
                Ok(provider) => return Ok(provider),
                Err(_) => {
                    tracing::warn!(
                        "Provider {} not found, falling back to default",
                        provider_id
                    );
                }
            }
        }
        self.get_default()
    }
}

// ============================================================================
// AGENT LOADER
// ============================================================================

/// Loads and prepares agent configurations.
pub struct AgentLoader<'a> {
    agent_service: &'a AgentService,
    provider_resolver: ProviderResolver<'a>,
    paths: SharedVaultPaths,
}

impl<'a> AgentLoader<'a> {
    /// Create a new agent loader.
    pub fn new(
        agent_service: &'a AgentService,
        provider_service: &'a ProviderService,
        paths: SharedVaultPaths,
    ) -> Self {
        Self {
            agent_service,
            provider_resolver: ProviderResolver::new(provider_service),
            paths,
        }
    }

    /// Load an agent by ID.
    ///
    /// Returns the agent and its resolved provider.
    pub async fn load(
        &self,
        agent_id: &str,
    ) -> Result<(gateway_services::agents::Agent, Provider), String> {
        let mut agent = self
            .agent_service
            .get(agent_id)
            .await
            .map_err(|e| format!("Failed to load agent {}: {}", agent_id, e))?;

        // Append OS context and shards to agent instructions
        // so subagents know platform commands and tool syntax
        agent.instructions = append_system_context(&agent.instructions, &self.paths);

        let provider = self.provider_resolver.get_or_default(&agent.provider_id)?;

        Ok((agent, provider))
    }

    /// Load an agent by ID, creating a default root agent if the ID is "root" and not found.
    pub async fn load_or_create_root(
        &self,
        agent_id: &str,
    ) -> Result<(gateway_services::agents::Agent, Provider), String> {
        match self.agent_service.get(agent_id).await {
            Ok(agent) => {
                let provider = self.provider_resolver.get_or_default(&agent.provider_id)?;
                Ok((agent, provider))
            }
            Err(_) if agent_id == "root" => {
                // Create a default root agent using the default provider
                let provider = self.provider_resolver.get_default()?;

                // Use first model from provider or default
                let model = provider
                    .models
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "gpt-4o".to_string());

                let agent = gateway_services::agents::Agent {
                    id: "root".to_string(),
                    name: "root".to_string(),
                    display_name: "Root Agent".to_string(),
                    description: "System root agent that handles all conversations".to_string(),
                    agent_type: Some("orchestrator".to_string()),
                    provider_id: provider.id.clone().unwrap_or_default(),
                    model,
                    temperature: 0.7,
                    max_tokens: 8192,
                    thinking_enabled: false,
                    voice_recording_enabled: false,
                    system_instruction: None,
                    instructions: gateway_templates::load_system_prompt_from_paths(&self.paths),
                    mcps: vec![],
                    skills: vec![],
                    middleware: None,
                    created_at: None,
                };

                Ok((agent, provider))
            }
            Err(e) => Err(e),
        }
    }

    /// Load an agent by ID. If not found, auto-create a specialist agent
    /// with the same provider/model as root and role-specific instructions.
    pub async fn load_or_create_specialist(
        &self,
        agent_id: &str,
    ) -> Result<(gateway_services::agents::Agent, Provider), String> {
        // Try loading existing agent first
        match self.agent_service.get(agent_id).await {
            Ok(agent) => {
                let provider = self.provider_resolver.get_or_default(&agent.provider_id)?;
                Ok((agent, provider))
            }
            Err(_) => {
                // Agent doesn't exist — auto-create with default provider
                let provider = self.provider_resolver.get_default()?;
                let model = provider
                    .models
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "gpt-4o".to_string());

                let instructions = build_specialist_instructions(agent_id, &self.paths);

                tracing::info!(
                    agent_id = %agent_id,
                    "Auto-creating specialist agent (not found in config)"
                );

                let agent = gateway_services::agents::Agent {
                    id: agent_id.to_string(),
                    name: agent_id.to_string(),
                    display_name: format_agent_display_name(agent_id),
                    description: format!("Auto-created specialist: {}", agent_id),
                    agent_type: Some("specialist".to_string()),
                    provider_id: provider.id.clone().unwrap_or_default(),
                    model,
                    temperature: 0.7,
                    max_tokens: 8192,
                    thinking_enabled: false,
                    voice_recording_enabled: false,
                    system_instruction: None,
                    instructions,
                    mcps: vec![],
                    skills: vec![],
                    middleware: None,
                    created_at: None,
                };

                Ok((agent, provider))
            }
        }
    }

    /// Get the provider resolver.
    pub fn provider_resolver(&self) -> &ProviderResolver<'a> {
        &self.provider_resolver
    }
}

// ============================================================================
// SPECIALIST AGENT HELPERS
// ============================================================================

/// Build specialist instructions from OS.md + tooling shard + role preamble.
/// Does NOT include orchestration/planning instructions — specialists execute, they don't orchestrate.
/// Append OS context and tooling shard to agent instructions.
/// This ensures ALL agents (pre-configured and auto-created) know:
/// - Platform commands (PowerShell vs bash)
/// - Tool syntax (apply_patch, shell, etc.)
fn append_system_context(instructions: &str, paths: &SharedVaultPaths) -> String {
    let os_context = std::fs::read_to_string(paths.vault_dir().join("config").join("OS.md"))
        .unwrap_or_default();

    let tooling = gateway_templates::Templates::get("shards/tooling_skills.md")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_default();

    let memory_shard = gateway_templates::Templates::get("shards/memory_learning.md")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_default();

    // Subagent discipline: execute directly, don't over-plan
    let subagent_note = "\n\n# --- SUBAGENT RULES ---\n\
        You are a specialist executing a specific task. Do NOT create complex plans.\n\
        Execute your task directly in as few tool calls as possible.\n\
        Use apply_patch for ALL file creation and editing.\n\
        If your task fails after 2 attempts, respond with what you accomplished and what failed.\n";

    format!(
        "{}\n\n# --- SYSTEM CONTEXT ---\n\n{}\n\n{}\n\n{}{}",
        instructions, os_context, tooling, memory_shard, subagent_note
    )
}

fn build_specialist_instructions(agent_id: &str, paths: &SharedVaultPaths) -> String {
    let role_preamble = generate_role_preamble(agent_id);

    // Load OS context for platform-native commands
    let os_context = std::fs::read_to_string(paths.vault_dir().join("config").join("OS.md"))
        .unwrap_or_default();

    // Load tooling shard for apply_patch syntax and tool docs
    let tooling = gateway_templates::Templates::get("shards/tooling_skills.md")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_default();

    format!(
        "{}\n\n{}\n\n{}",
        role_preamble, os_context, tooling
    )
}

/// Generate a role-specific preamble based on the agent name.
fn generate_role_preamble(agent_id: &str) -> String {
    let name_lower = agent_id.to_lowercase();

    let role_description = if name_lower.contains("coder") || name_lower.contains("code") || name_lower.contains("developer") || name_lower.contains("programmer") {
        "You are a coding specialist. Write clean, modular, reusable code.\n\
         Use apply_patch for all file creation and editing.\n\
         Follow the coding skill protocol: explore ward, plan, build core/ first, then task scripts.\n\
         Fix broken code — never create _v2 or _improved copies.\n\
         Load the 'coding' skill for detailed instructions."
    } else if name_lower.contains("research") || name_lower.contains("search") {
        "You are a research specialist. Gather, analyze, and synthesize information.\n\
         Use web search and available tools to find accurate, up-to-date information.\n\
         Save findings as structured JSON/markdown files in the ward.\n\
         Cite sources and cross-reference facts."
    } else if name_lower.contains("writ") || name_lower.contains("report") {
        "You are a writing specialist. Create clear, professional documents and reports.\n\
         Use apply_patch to create well-formatted HTML, markdown, or text files.\n\
         Put all output in the output/ directory of the ward.\n\
         Focus on clarity, structure, and visual presentation."
    } else if name_lower.contains("analy") || name_lower.contains("data") {
        "You are a data analysis specialist. Analyze data, compute metrics, and generate insights.\n\
         Write clean Python scripts that import from core/ modules.\n\
         Save results as JSON/CSV in the task subdirectory.\n\
         Load the 'coding' skill for file organization guidelines."
    } else {
        "You are a specialist agent. Execute the task you are given precisely.\n\
         Use apply_patch for file operations. Work in the ward specified in your task.\n\
         Read AGENTS.md before writing code. Follow existing patterns."
    };

    format!("You are **{}**.\n\n{}", agent_id, role_description)
}

/// Format agent ID as display name: "python-coder" → "Python Coder"
fn format_agent_display_name(agent_id: &str) -> String {
    agent_id
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.collect::<String>()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_agent_display_name() {
        assert_eq!(format_agent_display_name("python-coder"), "Python Coder");
        assert_eq!(format_agent_display_name("research-agent"), "Research Agent");
        assert_eq!(format_agent_display_name("analyst"), "Analyst");
    }

    #[test]
    fn test_generate_role_preamble_coder() {
        let preamble = generate_role_preamble("python-coder");
        assert!(preamble.contains("**python-coder**"));
        assert!(preamble.contains("coding specialist"));
    }

    #[test]
    fn test_generate_role_preamble_researcher() {
        let preamble = generate_role_preamble("research-agent");
        assert!(preamble.contains("research specialist"));
    }

    #[test]
    fn test_generate_role_preamble_writer() {
        let preamble = generate_role_preamble("writing-agent");
        assert!(preamble.contains("writing specialist"));
    }

    #[test]
    fn test_generate_role_preamble_analyst() {
        let preamble = generate_role_preamble("data-analyst");
        assert!(preamble.contains("data analysis specialist"));
    }

    #[test]
    fn test_generate_role_preamble_generic() {
        let preamble = generate_role_preamble("helper-bot");
        assert!(preamble.contains("specialist agent"));
    }
}
