//! # Core Types
//!
//! Core data structures used across the Zero framework.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Content represents a message with role and parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    /// The role of the content creator (user, assistant, system, tool)
    pub role: String,

    /// The parts that make up this content
    pub parts: Vec<Part>,
}

impl Content {
    /// Create a new user content.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            parts: vec![Part::Text { text: text.into() }],
        }
    }

    /// Create a new assistant content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            parts: vec![Part::Text { text: text.into() }],
        }
    }

    /// Create a new system content.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            parts: vec![Part::Text { text: text.into() }],
        }
    }

    /// Create a new tool response content.
    pub fn tool_response(tool_call_id: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            parts: vec![Part::FunctionResponse {
                id: tool_call_id.into(),
                response: response.into(),
            }],
        }
    }

    /// Get the text content if this is a text part.
    pub fn text(&self) -> Option<&str> {
        self.parts.iter().find_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
    }
}

/// Part represents a single piece of content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Part {
    /// Plain text content
    #[serde(rename = "text")]
    Text { text: String },

    /// Function call (tool call)
    #[serde(rename = "function_call")]
    FunctionCall {
        name: String,
        args: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },

    /// Function response (tool result)
    #[serde(rename = "function_response")]
    FunctionResponse {
        id: String,
        response: String,
    },

    /// Binary data
    #[serde(rename = "binary")]
    Binary {
        mime_type: String,
        data: Vec<u8>,
    },
}

impl Part {
    /// Create a text part.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create a function call part.
    pub fn function_call(name: impl Into<String>, args: Value) -> Self {
        Self::FunctionCall {
            name: name.into(),
            args,
            id: None,
        }
    }

    /// Create a function call part with ID.
    pub fn function_call_with_id(
        name: impl Into<String>,
        args: Value,
        id: impl Into<String>,
    ) -> Self {
        Self::FunctionCall {
            name: name.into(),
            args,
            id: Some(id.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_user() {
        let content = Content::user("Hello");
        assert_eq!(content.role, "user");
        assert_eq!(content.text(), Some("Hello"));
    }

    #[test]
    fn test_content_assistant() {
        let content = Content::assistant("Hi there");
        assert_eq!(content.role, "assistant");
    }

    #[test]
    fn test_content_system() {
        let content = Content::system("You are helpful");
        assert_eq!(content.role, "system");
    }

    #[test]
    fn test_part_text() {
        let part = Part::text("Hello");
        match part {
            Part::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text part"),
        }
    }

    #[test]
    fn test_part_function_call() {
        let part = Part::function_call("search", serde_json::json!({"query": "test"}));
        match part {
            Part::FunctionCall { name, args, .. } => {
                assert_eq!(name, "search");
                assert_eq!(args["query"], "test");
            }
            _ => panic!("Expected FunctionCall part"),
        }
    }

    #[test]
    fn test_content_serialization() {
        let content = Content::user("Hello");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }
}
