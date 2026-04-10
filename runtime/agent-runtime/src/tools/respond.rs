//! # Respond Tool
//!
//! Tool for sending responses back to the originating hook.
//!
//! This tool routes messages to the correct channel (WebSocket, webhook, etc)
//! based on the HookContext that was set when the agent was invoked.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{Tool, ToolContext};

/// Tool for sending responses back to the originating hook.
///
/// When an agent is invoked through a hook (WebSocket, webhook, CLI, etc),
/// the hook context is stored in the execution state. This tool reads that
/// context and routes the response back to the correct channel.
///
/// # Example
///
/// ```json
/// {
///   "message": "Hello! How can I help you today?",
///   "format": "markdown"
/// }
/// ```
pub struct RespondTool;

impl RespondTool {
    /// Create a new respond tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RespondTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RespondTool {
    fn name(&self) -> &str {
        "respond"
    }

    fn description(&self) -> &str {
        "Send a response message back to the user through the originating channel. \
         Use this to communicate with the user who initiated the conversation."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The response message to send to the user"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown", "html"],
                    "default": "text",
                    "description": "Format of the message (text, markdown, or html)"
                },
                "artifacts": {
                    "type": "array",
                    "description": "Files produced by this execution. Include any outputs the user would want to see or download.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "File path relative to the current ward"
                            },
                            "label": {
                                "type": "string",
                                "description": "Human-readable label for this artifact"
                            }
                        },
                        "required": ["path"]
                    }
                }
            },
            "required": ["message"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> zero_core::Result<Value> {
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("message is required".to_string()))?;

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let artifacts: Vec<zero_core::event::ArtifactDeclaration> = args
            .get("artifacts")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // Get hook context from state
        let hook_context = ctx.get_state("hook_context");

        // Get conversation ID for response routing
        let conversation_id = ctx
            .get_state("conversation_id")
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        // If we have hook context, use it to determine response routing
        let hook_type = hook_context
            .as_ref()
            .and_then(|v| v.get("hook_type"))
            .and_then(|v| v.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let session_id = hook_context
            .as_ref()
            .and_then(|v| v.get("hook_type"))
            .and_then(|v| v.get("session_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Set response in actions for the executor to pick up
        let mut actions = ctx.actions();
        actions.respond = Some(zero_core::event::RespondAction {
            message: message.to_string(),
            format: format.to_string(),
            conversation_id: conversation_id.clone(),
            session_id: session_id.clone(),
            artifacts,
        });
        ctx.set_actions(actions);

        Ok(json!({
            "status": "sent",
            "hook_type": hook_type,
            "conversation_id": conversation_id,
            "session_id": session_id,
            "message_length": message.len()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_respond_tool_schema() {
        let tool = RespondTool::new();
        assert_eq!(tool.name(), "respond");

        let schema = tool.parameters_schema().unwrap();
        assert!(schema.get("properties").is_some());
        assert!(schema
            .get("properties")
            .unwrap()
            .get("message")
            .is_some());
    }
}
