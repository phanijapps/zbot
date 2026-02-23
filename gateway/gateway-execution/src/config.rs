//! # Execution Configuration
//!
//! Configuration types for agent execution.

use execution_state::TriggerSource;
use gateway_events::HookContext;
use serde_json::Value;
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
        // Agent data is stored in wards/{agent_id}/
        Some(self.vault_dir.join("wards").join(agent_id))
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
        // Session data is stored in wards/{session_id}/
        Some(self.vault_dir.join("wards").join(session_id))
    }

    fn wards_root_dir(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("wards"))
    }

    fn ward_dir(&self, ward_id: &str) -> Option<PathBuf> {
        Some(self.vault_dir.join("wards").join(ward_id))
    }

    fn mcps_config(&self) -> Option<PathBuf> {
        Some(self.vault_dir.join("config").join("mcps.json"))
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
    /// Thread ID for conversation threading with external connectors.
    pub thread_id: Option<String>,
    /// Connector ID that triggered this session.
    pub connector_id: Option<String>,
    /// Trigger source for this execution (web, cli, connector, etc.)
    pub source: TriggerSource,
    /// Metadata from the request (e.g., plugin context, sender info)
    pub metadata: Option<Value>,
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
            thread_id: None,
            connector_id: None,
            source: TriggerSource::default(),
            metadata: None,
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

    /// Set the thread ID for conversation threading.
    #[must_use]
    pub fn with_thread_id(mut self, thread_id: String) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Set the connector ID that triggered this session.
    #[must_use]
    pub fn with_connector_id(mut self, connector_id: String) -> Self {
        self.connector_id = Some(connector_id);
        self
    }

    /// Set the trigger source for this execution.
    #[must_use]
    pub fn with_source(mut self, source: TriggerSource) -> Self {
        self.source = source;
        self
    }

    /// Set the metadata from the request.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn execution_config_new_defaults() {
        let config = ExecutionConfig::new(
            "root".to_string(),
            "conv-123".to_string(),
            PathBuf::from("/tmp"),
        );

        assert_eq!(config.agent_id, "root");
        assert_eq!(config.conversation_id, "conv-123");
        assert_eq!(config.source, TriggerSource::Web); // default
        assert!(config.metadata.is_none());
        assert!(config.session_id.is_none());
        assert!(config.respond_to.is_none());
    }

    #[test]
    fn execution_config_with_source() {
        let config = ExecutionConfig::new(
            "root".to_string(),
            "conv-123".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_source(TriggerSource::Connector);

        assert_eq!(config.source, TriggerSource::Connector);
    }

    #[test]
    fn execution_config_with_metadata() {
        let metadata = serde_json::json!({
            "thread_id": "C123:1234567890.123456",
            "sender": "U12345",
        });

        let config = ExecutionConfig::new(
            "root".to_string(),
            "conv-123".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_metadata(metadata.clone());

        assert_eq!(config.metadata, Some(metadata));
    }

    #[test]
    fn execution_config_builder_chain() {
        let metadata = serde_json::json!({"key": "value"});
        let config = ExecutionConfig::new(
            "root".to_string(),
            "conv-123".to_string(),
            PathBuf::from("/tmp"),
        )
        .with_source(TriggerSource::Connector)
        .with_metadata(metadata.clone())
        .with_session_id("sess-456".to_string())
        .with_thread_id("thread-789".to_string())
        .with_connector_id("slack".to_string())
        .with_respond_to(vec!["slack".to_string()]);

        assert_eq!(config.source, TriggerSource::Connector);
        assert_eq!(config.metadata, Some(metadata));
        assert_eq!(config.session_id, Some("sess-456".to_string()));
        assert_eq!(config.thread_id, Some("thread-789".to_string()));
        assert_eq!(config.connector_id, Some("slack".to_string()));
        assert_eq!(config.respond_to, Some(vec!["slack".to_string()]));
    }
}
