// ============================================================================
// SET SESSION TITLE TOOL
// Allows the agent to set a human-readable title for the current session.
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use zero_core::{Result, Tool, ToolContext, ZeroError};

// ============================================================================
// SET SESSION TITLE TOOL
// ============================================================================

/// Tool that sets a human-readable title for the current session.
/// Returns a JSON payload with a `__session_title_changed__` marker
/// so the executor can emit a `SessionTitleChanged` stream event.
pub struct SetSessionTitleTool;

impl SetSessionTitleTool {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SetSessionTitleTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SetSessionTitleTool {
    fn name(&self) -> &str {
        "set_session_title"
    }

    fn description(&self) -> &str {
        "Set a human-readable title for the current session. Call this early so the UI shows a meaningful name instead of a session ID."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Concise title (2-8 words) describing the task"
                }
            },
            "required": ["title"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'title' string parameter".to_string()))?;

        if title.trim().is_empty() {
            return Err(ZeroError::Tool("Title cannot be empty".to_string()));
        }

        // Truncate overly long titles
        let title = if title.len() > 120 {
            &title[..120]
        } else {
            title
        };

        let session_id = ctx.session_id().to_string();
        tracing::debug!(
            "Setting session title: session={}, title={}",
            session_id,
            title
        );

        // Store title in session state so the stream handler can persist it
        ctx.set_state(
            "app:session_title".to_string(),
            json!({ "session_id": session_id, "title": title }),
        );

        // Return with marker so the executor emits a SessionTitleChanged stream event
        Ok(json!({
            "__session_title_changed__": true,
            "title": title,
            "message": format!("Session title set to: {}", title)
        }))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_session_title_schema() {
        let tool = SetSessionTitleTool::new();
        assert_eq!(tool.name(), "set_session_title");
        let schema = tool.parameters_schema().unwrap();
        assert!(schema.get("properties").unwrap().get("title").is_some());
    }
}
