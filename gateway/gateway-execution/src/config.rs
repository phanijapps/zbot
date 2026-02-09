//! # Execution Configuration
//!
//! Configuration types for agent execution.

use gateway_events::HookContext;
use std::path::PathBuf;
use zero_core::FileSystemContext;

// ============================================================================
// FILE SYSTEM CONTEXT
// ============================================================================

/// File system context for gateway execution.
///
/// Provides paths to the agent tools based on the vault directory structure.
#[derive(Debug, Clone)]
pub struct GatewayFileSystem {
    /// Base vault/config directory
    vault_dir: PathBuf,
}

impl GatewayFileSystem {
    /// Create a new gateway file system context.
    pub fn new(vault_dir: PathBuf) -> Self {
        Self { vault_dir }
    }
}

impl FileSystemContext for GatewayFileSystem {
    fn conversation_dir(&self, conversation_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("conversations").join(conversation_id))
    }

    fn outputs_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("outputs"))
    }

    fn skills_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("skills"))
    }

    fn agents_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("agents"))
    }

    fn agent_data_dir(&self, agent_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("agents_data").join(agent_id))
    }

    fn python_executable(&self) -> Option<PathBuf> {
        // Use system Python - could be made configurable
        None
    }

    fn vault_path(&self) -> Option<PathBuf> {
        Some(self.vault_dir.clone())
    }

    fn session_code_dir(&self, session_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("code").join(session_id))
    }

    fn session_data_dir(&self, session_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("agent_data").join(session_id))
    }

    fn wards_root_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("wards"))
    }

    fn ward_dir(&self, ward_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("wards").join(ward_id))
    }
}

// ============================================================================
// EXECUTION CONFIG
// ============================================================================

/// Configuration for agent execution.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Agent ID to execute
    pub agent_id: String,
    /// Conversation ID for tracking (legacy, used for message persistence)
    pub conversation_id: String,
    /// Configuration directory (vault path)
    pub config_dir: PathBuf,
    /// Maximum iterations before prompting for continuation
    pub max_iterations: u32,
    /// Optional hook context for routing responses
    pub hook_context: Option<HookContext>,
    /// Optional session ID for continuing an existing session.
    /// - None: create a new session
    /// - Some(id): continue the existing session with this ID
    pub session_id: Option<String>,
    /// Optional connector IDs to route the final response to.
    /// - None/empty: response goes to WebSocket subscribers only (default)
    /// - Some([...]): response also dispatched to listed connectors
    pub respond_to: Option<Vec<String>>,
}

impl ExecutionConfig {
    /// Create a new execution config.
    pub fn new(agent_id: String, conversation_id: String, config_dir: PathBuf) -> Self {
        Self {
            agent_id,
            conversation_id,
            config_dir,
            max_iterations: 25,
            hook_context: None,
            session_id: None,
            respond_to: None,
        }
    }

    /// Set the hook context for routing responses.
    #[must_use]
    pub fn with_hook_context(mut self, hook_context: HookContext) -> Self {
        self.hook_context = Some(hook_context);
        self
    }

    /// Set the session ID to continue an existing session.
    #[must_use]
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set the connector IDs to route the final response to.
    #[must_use]
    pub fn with_respond_to(mut self, connector_ids: Vec<String>) -> Self {
        self.respond_to = Some(connector_ids);
        self
    }
}
