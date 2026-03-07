//! # Concrete Tool Context Implementation
//!
//! A concrete implementation of ToolContext that can be used by tools.

use std::sync::Arc;
use zero_core::{ToolContext, CallbackContext, ReadonlyContext};
use zero_core::{Content, EventActions};
use serde_json::Value;

/// Concrete implementation of ToolContext for tool execution
pub struct ToolContextImpl {
    /// Function call ID for this tool execution
    function_call_id: String,

    /// File system context
    pub fs: Arc<dyn zero_core::FileSystemContext>,

    /// Current event actions
    actions: std::sync::Mutex<EventActions>,
}

impl ToolContextImpl {
    /// Create a new tool context
    pub fn new(
        function_call_id: String,
        fs: Arc<dyn zero_core::FileSystemContext>,
    ) -> Self {
        Self {
            function_call_id,
            fs,
            actions: std::sync::Mutex::new(EventActions::default()),
        }
    }

    /// Create with default function call ID
    pub fn with_fs(fs: Arc<dyn zero_core::FileSystemContext>) -> Self {
        Self::new("tool_call".to_string(), fs)
    }
}

impl ReadonlyContext for ToolContextImpl {
    fn invocation_id(&self) -> &str {
        "invocation"
    }

    fn agent_name(&self) -> &str {
        "agent"
    }

    fn user_id(&self) -> &str {
        "user"
    }

    fn app_name(&self) -> &str {
        "zbot"
    }

    fn session_id(&self) -> &str {
        "session"
    }

    fn branch(&self) -> &str {
        "main"
    }

    fn user_content(&self) -> &Content {
        static EMPTY_CONTENT: std::sync::LazyLock<Content> = std::sync::LazyLock::new(|| {
            Content {
                role: "user".to_string(),
                parts: vec![],
            }
        });
        &EMPTY_CONTENT
    }
}

impl CallbackContext for ToolContextImpl {
    fn get_state(&self, _key: &str) -> Option<Value> {
        None
    }

    fn set_state(&self, _key: String, _value: Value) {
        // No-op for simple tool context
    }
}

impl ToolContext for ToolContextImpl {
    fn function_call_id(&self) -> String {
        self.function_call_id.clone()
    }

    fn actions(&self) -> EventActions {
        // Handle poisoned locks gracefully by taking the inner value
        self.actions.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_actions(&self, actions: EventActions) {
        // Handle poisoned locks gracefully by taking the inner value
        *self.actions.lock().unwrap_or_else(|e| e.into_inner()) = actions;
    }
}

unsafe impl Send for ToolContextImpl {}
unsafe impl Sync for ToolContextImpl {}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::NoFileSystemContext;

    #[test]
    fn test_tool_context_impl() {
        let ctx = ToolContextImpl::with_fs(Arc::new(NoFileSystemContext));
        assert_eq!(ctx.function_call_id(), "tool_call");
        assert_eq!(ctx.invocation_id(), "invocation");
        assert_eq!(ctx.agent_name(), "agent");
    }
}
