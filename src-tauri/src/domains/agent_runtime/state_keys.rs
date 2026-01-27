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

    /// State key for root agent ID (orchestrator)
    /// For subagents, this is the parent orchestrator's ID
    /// Used for determining the data directory (subagents share parent's data dir)
    pub const ROOT_AGENT_ID: &str = "app:root_agent_id";

    /// State key for provider ID
    /// The LLM provider being used for the current conversation
    pub const PROVIDER_ID: &str = "app:provider_id";

    /// State key for database path
    /// Path to the agent_channels.db for knowledge graph access
    pub const DB_PATH: &str = "app:db_path";

    // ============================================================================
    // EXECUTION CONTROL STATE KEYS
    // ============================================================================

    /// State key for stop execution flag
    /// When set to true, the agent loop should stop at the next iteration
    pub const EXECUTION_STOP: &str = "execution_control::stop";

    /// State key for current iteration count
    /// Tracks the current iteration number in the agent loop
    pub const EXECUTION_ITERATION: &str = "execution_control::iteration";

    /// State key for max iterations override
    /// Allows dynamic control of max iterations per execution
    pub const EXECUTION_MAX_ITERATIONS: &str = "execution_control::max_iterations";

    // ============================================================================
    // TODO LIST STATE KEYS
    // ============================================================================

    /// State key for TODO list
    /// Stores the agent's TODO list as JSON
    pub const TODO_LIST: &str = "app:todo_list";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_keys_have_app_prefix() {
        assert!(state_keys::CONVERSATION_ID.starts_with("app:"));
        assert!(state_keys::USER_ID.starts_with("app:"));
        assert!(state_keys::AGENT_ID.starts_with("app:"));
        assert!(state_keys::ROOT_AGENT_ID.starts_with("app:"));
        assert!(state_keys::PROVIDER_ID.starts_with("app:"));
        assert!(state_keys::DB_PATH.starts_with("app:"));
        assert!(state_keys::TODO_LIST.starts_with("app:"));
    }

    #[test]
    fn test_execution_control_keys_have_prefix() {
        assert!(state_keys::EXECUTION_STOP.starts_with("execution_control::"));
        assert!(state_keys::EXECUTION_ITERATION.starts_with("execution_control::"));
        assert!(state_keys::EXECUTION_MAX_ITERATIONS.starts_with("execution_control::"));
    }
}
