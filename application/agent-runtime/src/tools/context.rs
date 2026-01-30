// ============================================================================
// TOOL CONTEXT
// Execution context for tool operations
// ============================================================================

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use serde_json::Value;
use zero_core::event::EventActions;
use zero_core::types::Content;

/// Context for tool execution
///
/// Provides tools with necessary information about the execution environment
/// including conversation scoping and available resources.
///
/// Implements `zero_core::ToolContext` trait for compatibility with tools.
pub struct ToolContext {
    /// Optional conversation ID for scoping file operations
    pub conversation_id: Option<String>,

    /// Skills available to the current agent (for load_skill tool)
    pub available_skills: Vec<String>,

    /// Agent ID for this execution
    pub agent_id: Option<String>,

    /// Function call ID for this tool execution
    pub function_call_id: String,

    /// Key-value state storage
    state: RwLock<HashMap<String, Value>>,

    /// Event actions
    actions: RwLock<EventActions>,

    /// Static empty content for user_content()
    empty_content: Content,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            conversation_id: None,
            available_skills: Vec::new(),
            agent_id: None,
            function_call_id: String::new(),
            state: RwLock::new(HashMap::new()),
            actions: RwLock::new(EventActions::default()),
            empty_content: Content {
                role: "user".to_string(),
                parts: vec![],
            },
        }
    }
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

    /// Set the function call ID
    #[must_use]
    pub fn with_function_call_id(mut self, id: String) -> Self {
        self.function_call_id = id;
        self
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

// ============================================================================
// IMPLEMENT ZERO_CORE TRAITS
// ============================================================================

impl zero_core::ReadonlyContext for ToolContext {
    fn invocation_id(&self) -> &str {
        self.conversation_id.as_deref().unwrap_or("unknown")
    }

    fn agent_name(&self) -> &str {
        self.agent_id.as_deref().unwrap_or("root")
    }

    fn user_id(&self) -> &str {
        "default"
    }

    fn app_name(&self) -> &str {
        "agentzero"
    }

    fn session_id(&self) -> &str {
        self.conversation_id.as_deref().unwrap_or("unknown")
    }

    fn branch(&self) -> &str {
        "main"
    }

    fn user_content(&self) -> &Content {
        &self.empty_content
    }
}

impl zero_core::CallbackContext for ToolContext {
    fn get_state(&self, key: &str) -> Option<Value> {
        self.state.read().ok()?.get(key).cloned()
    }

    fn set_state(&self, key: String, value: Value) {
        if let Ok(mut state) = self.state.write() {
            state.insert(key, value);
        }
    }
}

impl zero_core::ToolContext for ToolContext {
    fn function_call_id(&self) -> &str {
        &self.function_call_id
    }

    fn actions(&self) -> EventActions {
        self.actions.read().map(|a| a.clone()).unwrap_or_default()
    }

    fn set_actions(&self, actions: EventActions) {
        if let Ok(mut a) = self.actions.write() {
            *a = actions;
        }
    }
}
