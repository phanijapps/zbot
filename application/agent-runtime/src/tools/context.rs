// ============================================================================
// TOOL CONTEXT
// Execution context for tool operations
// ============================================================================

use std::path::PathBuf;
use std::sync::Arc;

/// Context for tool execution
///
/// Provides tools with necessary information about the execution environment
/// including conversation scoping and available resources.
#[derive(Clone, Default)]
pub struct ToolContext {
    /// Optional conversation ID for scoping file operations
    pub conversation_id: Option<String>,

    /// Skills available to the current agent (for load_skill tool)
    pub available_skills: Vec<String>,

    /// Agent ID for this execution
    pub agent_id: Option<String>,
}

impl ToolContext {
    /// Create a new empty context
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with conversation ID
    #[must_use]
    pub fn with_conversation(conversation_id: String) -> Self {
        Self {
            conversation_id: Some(conversation_id),
            ..Default::default()
        }
    }

    /// Create a context with skills
    #[must_use]
    pub fn with_skills(available_skills: Vec<String>) -> Self {
        Self {
            available_skills,
            ..Default::default()
        }
    }

    /// Create a context with conversation and skills
    #[must_use]
    pub fn with_conversation_and_skills(
        conversation_id: String,
        available_skills: Vec<String>,
    ) -> Self {
        Self {
            conversation_id: Some(conversation_id),
            available_skills,
            ..Default::default()
        }
    }

    /// Create a context with agent ID
    #[must_use]
    pub fn with_agent_id(agent_id: String) -> Self {
        Self {
            agent_id: Some(agent_id),
            ..Default::default()
        }
    }

    /// Get the conversation directory if conversation_id is set
    ///
    /// This returns None in the library context. The application layer
    /// should provide the actual path resolution.
    #[must_use]
    pub fn conversation_dir(&self) -> Option<PathBuf> {
        // In the library, we don't have access to the actual file system
        // The application layer should override or extend this
        self.conversation_id.as_ref().map(|id| PathBuf::from(format!("/conversations/{}", id)))
    }
}
