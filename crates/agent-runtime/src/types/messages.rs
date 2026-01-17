// ============================================================================
// CHAT MESSAGE TYPES
// Core message types for LLM communication
// ============================================================================

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A chat message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role (system, user, assistant, tool)
    pub role: String,

    /// Message content
    pub content: String,

    /// Tool calls made by the assistant (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// ID of the tool call this message is responding to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// Create a new user message
    #[must_use]
    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a new assistant message
    #[must_use]
    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a new system message
    #[must_use]
    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a new tool result message
    #[must_use]
    pub fn tool_result(tool_call_id: String, content: String) -> Self {
        Self {
            role: "tool".to_string(),
            content,
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
        }
    }
}

/// A tool call made by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,

    /// Name of the tool to call
    pub name: String,

    /// Arguments to pass to the tool (JSON object)
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new tool call
    #[must_use]
    pub fn new(id: String, name: String, arguments: Value) -> Self {
        Self { id, name, arguments }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let user_msg = ChatMessage::user("Hello".to_string());
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content, "Hello");
    }

    #[test]
    fn test_tool_call_creation() {
        let tool_call = ToolCall::new(
            "call_123".to_string(),
            "search".to_string(),
            serde_json::json!({"query": "test"}),
        );
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.name, "search");
    }
}
