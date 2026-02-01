//! # Setup Module
//!
//! Provider resolution and agent loading utilities for execution setup.

use crate::services::providers::Provider;
use crate::services::{AgentService, ProviderService, SettingsService};
use agent_tools::ToolSettings;
use std::path::PathBuf;
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
    /// Configuration directory
    pub config_dir: PathBuf,
}

impl SetupContext {
    /// Create a new setup context.
    pub fn new(
        agent_service: Arc<AgentService>,
        provider_service: Arc<ProviderService>,
        config_dir: PathBuf,
    ) -> Self {
        Self {
            agent_service,
            provider_service,
            config_dir,
        }
    }

    /// Get tool settings from the configuration directory.
    pub fn tool_settings(&self) -> ToolSettings {
        let settings_service = SettingsService::new(self.config_dir.clone());
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
}

impl<'a> AgentLoader<'a> {
    /// Create a new agent loader.
    pub fn new(agent_service: &'a AgentService, provider_service: &'a ProviderService) -> Self {
        Self {
            agent_service,
            provider_resolver: ProviderResolver::new(provider_service),
        }
    }

    /// Load an agent by ID.
    ///
    /// Returns the agent and its resolved provider.
    pub async fn load(
        &self,
        agent_id: &str,
    ) -> Result<(crate::services::agents::Agent, Provider), String> {
        let agent = self
            .agent_service
            .get(agent_id)
            .await
            .map_err(|e| format!("Failed to load agent {}: {}", agent_id, e))?;

        let provider = self.provider_resolver.get_or_default(&agent.provider_id)?;

        Ok((agent, provider))
    }

    /// Load an agent by ID, creating a default root agent if the ID is "root" and not found.
    pub async fn load_or_create_root(
        &self,
        agent_id: &str,
    ) -> Result<(crate::services::agents::Agent, Provider), String> {
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

                let agent = crate::services::agents::Agent {
                    id: "root".to_string(),
                    name: "root".to_string(),
                    display_name: "Root Agent".to_string(),
                    description: "System root agent that handles all conversations".to_string(),
                    agent_type: Some("orchestrator".to_string()),
                    provider_id: provider.id.clone().unwrap_or_default(),
                    model,
                    temperature: 0.7,
                    max_tokens: 4096,
                    thinking_enabled: false,
                    voice_recording_enabled: false,
                    system_instruction: None,
                    instructions: crate::templates::default_system_prompt(),
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

    /// Get the provider resolver.
    pub fn provider_resolver(&self) -> &ProviderResolver<'a> {
        &self.provider_resolver
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here but require mocking services
}
