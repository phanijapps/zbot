//! # WebSocket Messages
//!
//! Message types for WebSocket communication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Messages from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to a conversation's events.
    Subscribe { conversation_id: String },

    /// Unsubscribe from a conversation's events.
    Unsubscribe { conversation_id: String },

    /// Start or continue a conversation.
    Invoke {
        /// Agent ID to invoke
        agent_id: String,
        /// Conversation ID for tracking
        conversation_id: String,
        /// User message
        message: String,
        /// Optional session ID to continue (None = new session)
        #[serde(default)]
        session_id: Option<String>,
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

    /// End a session (mark as completed).
    /// Used when user types /end, /new, or clicks +new button.
    EndSession { session_id: String },

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
    // =========================================================================
    // SUBSCRIPTION RESPONSES
    // =========================================================================

    /// Subscription confirmed.
    Subscribed {
        conversation_id: String,
        current_sequence: u64,
    },

    /// Unsubscription confirmed.
    Unsubscribed { conversation_id: String },

    /// Subscription error.
    SubscriptionError {
        conversation_id: String,
        code: SubscriptionErrorCode,
        message: String,
    },

    // =========================================================================
    // CONVERSATION EVENTS (with sequence numbers)
    // =========================================================================

    /// Agent started executing.
    AgentStarted {
        agent_id: String,
        conversation_id: String,
        /// Session ID for this execution (for session continuity).
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Agent completed execution.
    AgentCompleted {
        agent_id: String,
        conversation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Agent was stopped.
    AgentStopped {
        agent_id: String,
        conversation_id: String,
        iteration: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Streaming text token.
    Token {
        conversation_id: String,
        delta: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Thinking/reasoning content.
    Thinking {
        conversation_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Tool call started.
    ToolCall {
        conversation_id: String,
        tool_call_id: String,
        tool: String,
        args: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Tool call result.
    ToolResult {
        conversation_id: String,
        tool_call_id: String,
        result: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Turn completed.
    TurnComplete {
        conversation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        final_message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Error occurred.
    Error {
        conversation_id: Option<String>,
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Iteration update.
    Iteration {
        conversation_id: String,
        current: u32,
        max: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Continuation prompt (max iterations reached).
    ContinuationPrompt {
        conversation_id: String,
        iteration: u32,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// New message added to conversation (for delegation callbacks, system messages).
    /// Frontend should refresh conversation to show the new message.
    MessageAdded {
        conversation_id: String,
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Token usage update for real-time metrics.
    TokenUsage {
        conversation_id: String,
        session_id: String,
        tokens_in: u64,
        tokens_out: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Delegation started - agent delegated work to a subagent.
    DelegationStarted {
        parent_agent_id: String,
        parent_conversation_id: String,
        child_agent_id: String,
        child_conversation_id: String,
        task: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    /// Delegation completed - subagent finished and returned result.
    DelegationCompleted {
        parent_agent_id: String,
        parent_conversation_id: String,
        child_agent_id: String,
        child_conversation_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },

    // =========================================================================
    // GLOBAL/CONNECTION MESSAGES (no sequence numbers)
    // =========================================================================

    /// Pong response.
    Pong,

    /// Connected successfully.
    Connected { session_id: String },

    /// Session paused.
    SessionPaused { session_id: String },

    /// Session resumed.
    SessionResumed { session_id: String },

    /// Session cancelled.
    SessionCancelled { session_id: String },

    /// Session ended (completed by user request).
    SessionEnded { session_id: String },
}

/// Error codes for subscription errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubscriptionErrorCode {
    NotFound,
    LimitExceeded,
    ServerError,
}

impl ServerMessage {
    /// Create an error message.
    pub fn error(conversation_id: Option<String>, code: &str, message: &str) -> Self {
        Self::Error {
            conversation_id,
            code: code.to_string(),
            message: message.to_string(),
            seq: None,
        }
    }

    /// Create a token message.
    pub fn token(conversation_id: &str, delta: &str) -> Self {
        Self::Token {
            conversation_id: conversation_id.to_string(),
            delta: delta.to_string(),
            seq: None,
        }
    }

    /// Create a turn complete message.
    pub fn turn_complete(conversation_id: &str, final_message: Option<String>) -> Self {
        Self::TurnComplete {
            conversation_id: conversation_id.to_string(),
            final_message,
            seq: None,
        }
    }

    /// Create a subscription error message.
    pub fn subscription_error(
        conversation_id: &str,
        code: SubscriptionErrorCode,
        message: &str,
    ) -> Self {
        Self::SubscriptionError {
            conversation_id: conversation_id.to_string(),
            code,
            message: message.to_string(),
        }
    }

    /// Get the conversation_id for this message, if any.
    pub fn conversation_id(&self) -> Option<&str> {
        match self {
            Self::AgentStarted { conversation_id, .. } => Some(conversation_id),
            Self::AgentCompleted { conversation_id, .. } => Some(conversation_id),
            Self::AgentStopped { conversation_id, .. } => Some(conversation_id),
            Self::Token { conversation_id, .. } => Some(conversation_id),
            Self::Thinking { conversation_id, .. } => Some(conversation_id),
            Self::ToolCall { conversation_id, .. } => Some(conversation_id),
            Self::ToolResult { conversation_id, .. } => Some(conversation_id),
            Self::TurnComplete { conversation_id, .. } => Some(conversation_id),
            Self::Error { conversation_id, .. } => conversation_id.as_deref(),
            Self::Iteration { conversation_id, .. } => Some(conversation_id),
            Self::ContinuationPrompt { conversation_id, .. } => Some(conversation_id),
            Self::MessageAdded { conversation_id, .. } => Some(conversation_id),
            Self::TokenUsage { conversation_id, .. } => Some(conversation_id),
            Self::DelegationStarted { parent_conversation_id, .. } => Some(parent_conversation_id),
            Self::DelegationCompleted { parent_conversation_id, .. } => Some(parent_conversation_id),
            Self::Subscribed { conversation_id, .. } => Some(conversation_id),
            Self::Unsubscribed { conversation_id, .. } => Some(conversation_id),
            Self::SubscriptionError { conversation_id, .. } => Some(conversation_id),
            Self::Pong | Self::Connected { .. } | Self::SessionPaused { .. }
            | Self::SessionResumed { .. } | Self::SessionCancelled { .. }
            | Self::SessionEnded { .. } => None,
        }
    }

    /// Return a copy of this message with the sequence number set.
    pub fn with_sequence(self, seq: u64) -> Self {
        match self {
            Self::AgentStarted { agent_id, conversation_id, session_id, .. } => {
                Self::AgentStarted { agent_id, conversation_id, session_id, seq: Some(seq) }
            }
            Self::AgentCompleted { agent_id, conversation_id, result, .. } => {
                Self::AgentCompleted { agent_id, conversation_id, result, seq: Some(seq) }
            }
            Self::AgentStopped { agent_id, conversation_id, iteration, .. } => {
                Self::AgentStopped { agent_id, conversation_id, iteration, seq: Some(seq) }
            }
            Self::Token { conversation_id, delta, .. } => {
                Self::Token { conversation_id, delta, seq: Some(seq) }
            }
            Self::Thinking { conversation_id, content, .. } => {
                Self::Thinking { conversation_id, content, seq: Some(seq) }
            }
            Self::ToolCall { conversation_id, tool_call_id, tool, args, .. } => {
                Self::ToolCall { conversation_id, tool_call_id, tool, args, seq: Some(seq) }
            }
            Self::ToolResult { conversation_id, tool_call_id, result, error, .. } => {
                Self::ToolResult { conversation_id, tool_call_id, result, error, seq: Some(seq) }
            }
            Self::TurnComplete { conversation_id, final_message, .. } => {
                Self::TurnComplete { conversation_id, final_message, seq: Some(seq) }
            }
            Self::Error { conversation_id, code, message, .. } => {
                Self::Error { conversation_id, code, message, seq: Some(seq) }
            }
            Self::Iteration { conversation_id, current, max, .. } => {
                Self::Iteration { conversation_id, current, max, seq: Some(seq) }
            }
            Self::ContinuationPrompt { conversation_id, iteration, message, .. } => {
                Self::ContinuationPrompt { conversation_id, iteration, message, seq: Some(seq) }
            }
            Self::MessageAdded { conversation_id, role, content, .. } => {
                Self::MessageAdded { conversation_id, role, content, seq: Some(seq) }
            }
            Self::TokenUsage { conversation_id, session_id, tokens_in, tokens_out, .. } => {
                Self::TokenUsage { conversation_id, session_id, tokens_in, tokens_out, seq: Some(seq) }
            }
            Self::DelegationStarted {
                parent_agent_id, parent_conversation_id, child_agent_id,
                child_conversation_id, task, ..
            } => Self::DelegationStarted {
                parent_agent_id, parent_conversation_id, child_agent_id,
                child_conversation_id, task, seq: Some(seq),
            },
            Self::DelegationCompleted {
                parent_agent_id, parent_conversation_id, child_agent_id,
                child_conversation_id, result, ..
            } => Self::DelegationCompleted {
                parent_agent_id, parent_conversation_id, child_agent_id,
                child_conversation_id, result, seq: Some(seq),
            },
            // Messages without sequence numbers pass through unchanged
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_deserialize() {
        let json = r#"{"type": "invoke", "agent_id": "root", "conversation_id": "123", "message": "Hello"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Invoke {
                agent_id,
                conversation_id,
                message,
                ..
            } => {
                assert_eq!(agent_id, "root");
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
