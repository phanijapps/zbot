// ============================================================================
// APPLICATION STATE KEYS
// ============================================================================
//!
//! State keys used by AgentZero application.
//! These are application-specific and live in the application layer,
//! separate from the zero-core framework.
//!
//! The framework provides the infrastructure (State trait, prefixes),
//! while the application defines its own state key semantics.

/// Application state keys
pub mod state_keys {
    /// State key for conversation ID
    /// Scoped to the current conversation, used by tools to resolve file paths
    pub const CONVERSATION_ID: &str = "app:conversation_id";

    /// State key for user ID
    /// Scoped to the current user session
    pub const USER_ID: &str = "app:user_id";

    /// State key for agent ID
    /// Scoped to the currently executing agent
    pub const AGENT_ID: &str = "app:agent_id";

    /// State key for provider ID
    /// The LLM provider being used for the current conversation
    pub const PROVIDER_ID: &str = "app:provider_id";

    /// State key for database path
    /// Path to the agent_channels.db for knowledge graph access
    pub const DB_PATH: &str = "app:db_path";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_keys_have_app_prefix() {
        assert!(state_keys::CONVERSATION_ID.starts_with("app:"));
        assert!(state_keys::USER_ID.starts_with("app:"));
        assert!(state_keys::AGENT_ID.starts_with("app:"));
        assert!(state_keys::PROVIDER_ID.starts_with("app:"));
        assert!(state_keys::DB_PATH.starts_with("app:"));
    }
}
