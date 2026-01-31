//! # WebSocket Messages
//!
//! Message types for WebSocket communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Messages from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Start or continue a conversation.
    Invoke {
        /// Agent ID to invoke
        agent_id: String,
        /// Conversation ID for tracking
        conversation_id: String,
        /// User message
        message: String,
        /// Optional metadata
        #[serde(default)]
        metadata: Option<Value>,
    },

    /// Stop the current execution.
    Stop { conversation_id: String },

    /// Continue after iteration limit.
    Continue {
        conversation_id: String,
        /// Additional iterations to allow (default: 25)
        #[serde(default = "default_additional_iterations")]
        additional_iterations: u32,
    },

    /// Pause a running session.
    Pause { session_id: String },

    /// Resume a paused or crashed session.
    Resume { session_id: String },

    /// Cancel a session.
    Cancel { session_id: String },

    /// Ping for keepalive.
    Ping,
}

fn default_additional_iterations() -> u32 {
    25
}

/// Messages from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Agent started executing.
    AgentStarted {
        agent_id: String,
        conversation_id: String,
    },

    /// Agent completed execution.
    AgentCompleted {
        agent_id: String,
        conversation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
    },

    /// Agent was stopped.
    AgentStopped {
        agent_id: String,
        conversation_id: String,
        iteration: u32,
    },

    /// Streaming text token.
    Token {
        conversation_id: String,
        delta: String,
    },

    /// Thinking/reasoning content.
    Thinking {
        conversation_id: String,
        content: String,
    },

    /// Tool call started.
    ToolCall {
        conversation_id: String,
        tool_call_id: String,
        tool: String,
        args: Value,
    },

    /// Tool call result.
    ToolResult {
        conversation_id: String,
        tool_call_id: String,
        result: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// Turn completed.
    TurnComplete {
        conversation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        final_message: Option<String>,
    },

    /// Error occurred.
    Error {
        conversation_id: Option<String>,
        code: String,
        message: String,
    },

    /// Iteration update.
    Iteration {
        conversation_id: String,
        current: u32,
        max: u32,
    },

    /// Continuation prompt (max iterations reached).
    ContinuationPrompt {
        conversation_id: String,
        iteration: u32,
        message: String,
    },

    /// Pong response.
    Pong,

    /// Connected successfully.
    Connected { session_id: String },

    /// New message added to conversation (for delegation callbacks, system messages).
    /// Frontend should refresh conversation to show the new message.
    MessageAdded {
        conversation_id: String,
        role: String,
        content: String,
    },

    /// Token usage update for real-time metrics.
    TokenUsage {
        conversation_id: String,
        session_id: String,
        tokens_in: u64,
        tokens_out: u64,
    },

    /// Session paused.
    SessionPaused { session_id: String },

    /// Session resumed.
    SessionResumed { session_id: String },

    /// Session cancelled.
    SessionCancelled { session_id: String },
}

impl ServerMessage {
    /// Create an error message.
    pub fn error(conversation_id: Option<String>, code: &str, message: &str) -> Self {
        Self::Error {
            conversation_id,
            code: code.to_string(),
            message: message.to_string(),
        }
    }

    /// Create a token message.
    pub fn token(conversation_id: &str, delta: &str) -> Self {
        Self::Token {
            conversation_id: conversation_id.to_string(),
            delta: delta.to_string(),
        }
    }

    /// Create a turn complete message.
    pub fn turn_complete(conversation_id: &str, final_message: Option<String>) -> Self {
        Self::TurnComplete {
            conversation_id: conversation_id.to_string(),
            final_message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_deserialize() {
        let json = r#"{"type": "invoke", "conversation_id": "123", "message": "Hello"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Invoke {
                conversation_id,
                message,
                ..
            } => {
                assert_eq!(conversation_id, "123");
                assert_eq!(message, "Hello");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_server_message_serialize() {
        let msg = ServerMessage::token("conv-1", "Hello");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("token"));
        assert!(json.contains("conv-1"));
    }
}
