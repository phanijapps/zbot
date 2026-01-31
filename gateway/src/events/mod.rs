//! # Events Module
//!
//! Event bus for broadcasting events to connected clients.

mod broadcast;

pub use broadcast::EventBus;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Gateway event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayEvent {
    /// Agent started executing.
    AgentStarted {
        agent_id: String,
        conversation_id: String,
    },

    /// Agent completed execution.
    AgentCompleted {
        agent_id: String,
        conversation_id: String,
        result: Option<String>,
    },

    /// Agent was stopped by user request.
    AgentStopped {
        agent_id: String,
        conversation_id: String,
        iteration: u32,
    },

    /// General error event.
    Error {
        agent_id: Option<String>,
        conversation_id: Option<String>,
        message: String,
    },

    /// Streaming token from agent.
    Token {
        agent_id: String,
        conversation_id: String,
        delta: String,
    },

    /// Thinking/reasoning content from agent.
    Thinking {
        agent_id: String,
        conversation_id: String,
        content: String,
    },

    /// Tool call started.
    ToolCall {
        agent_id: String,
        conversation_id: String,
        tool_id: String,
        tool_name: String,
        args: Value,
    },

    /// Tool call completed.
    ToolResult {
        agent_id: String,
        conversation_id: String,
        tool_id: String,
        result: String,
        error: Option<String>,
    },

    /// Turn complete (agent finished responding).
    TurnComplete {
        agent_id: String,
        conversation_id: String,
        message: String,
    },

    /// Iteration update for tracking progress.
    IterationUpdate {
        agent_id: String,
        conversation_id: String,
        current: u32,
        max: u32,
    },

    /// Continuation prompt when max iterations reached.
    ContinuationPrompt {
        agent_id: String,
        conversation_id: String,
        iteration: u32,
        message: String,
    },

    /// Response from the respond tool.
    ///
    /// This event is emitted when an agent uses the `respond` tool
    /// to send a message back to the originating hook.
    Respond {
        conversation_id: String,
        message: String,
        /// Session ID for web hooks (optional).
        session_id: Option<String>,
    },

    /// Delegation started event.
    ///
    /// Emitted when an agent delegates to a subagent.
    DelegationStarted {
        parent_agent_id: String,
        parent_conversation_id: String,
        child_agent_id: String,
        child_conversation_id: String,
        task: String,
    },

    /// Delegation completed event.
    ///
    /// Emitted when a delegated subagent completes.
    DelegationCompleted {
        parent_agent_id: String,
        parent_conversation_id: String,
        child_agent_id: String,
        child_conversation_id: String,
        result: Option<String>,
    },

    /// New message added to conversation.
    ///
    /// Emitted when a message is added outside of normal streaming
    /// (e.g., delegation callbacks, system messages).
    /// Frontend should refresh the conversation to show the new message.
    MessageAdded {
        conversation_id: String,
        role: String,
        content: String,
    },

    /// Token usage update for a session.
    ///
    /// Emitted after each LLM call with cumulative token counts.
    TokenUsage {
        conversation_id: String,
        session_id: String,
        tokens_in: u64,
        tokens_out: u64,
    },
}

impl GatewayEvent {
    /// Get the agent ID for this event (if available).
    pub fn agent_id(&self) -> Option<&str> {
        match self {
            Self::AgentStarted { agent_id, .. } => Some(agent_id),
            Self::AgentCompleted { agent_id, .. } => Some(agent_id),
            Self::AgentStopped { agent_id, .. } => Some(agent_id),
            Self::Error { agent_id, .. } => agent_id.as_deref(),
            Self::Token { agent_id, .. } => Some(agent_id),
            Self::Thinking { agent_id, .. } => Some(agent_id),
            Self::ToolCall { agent_id, .. } => Some(agent_id),
            Self::ToolResult { agent_id, .. } => Some(agent_id),
            Self::TurnComplete { agent_id, .. } => Some(agent_id),
            Self::IterationUpdate { agent_id, .. } => Some(agent_id),
            Self::ContinuationPrompt { agent_id, .. } => Some(agent_id),
            Self::Respond { .. } => None,
            Self::DelegationStarted {
                parent_agent_id, ..
            } => Some(parent_agent_id),
            Self::DelegationCompleted {
                parent_agent_id, ..
            } => Some(parent_agent_id),
            Self::MessageAdded { .. } => None,
            Self::TokenUsage { .. } => None,
        }
    }

    /// Get the conversation ID for this event (if available).
    pub fn conversation_id(&self) -> Option<&str> {
        match self {
            Self::AgentStarted { conversation_id, .. } => Some(conversation_id),
            Self::AgentCompleted { conversation_id, .. } => Some(conversation_id),
            Self::AgentStopped { conversation_id, .. } => Some(conversation_id),
            Self::Error { conversation_id, .. } => conversation_id.as_deref(),
            Self::Token { conversation_id, .. } => Some(conversation_id),
            Self::Thinking { conversation_id, .. } => Some(conversation_id),
            Self::ToolCall { conversation_id, .. } => Some(conversation_id),
            Self::ToolResult { conversation_id, .. } => Some(conversation_id),
            Self::TurnComplete { conversation_id, .. } => Some(conversation_id),
            Self::IterationUpdate { conversation_id, .. } => Some(conversation_id),
            Self::ContinuationPrompt { conversation_id, .. } => Some(conversation_id),
            Self::Respond { conversation_id, .. } => Some(conversation_id),
            Self::DelegationStarted {
                parent_conversation_id,
                ..
            } => Some(parent_conversation_id),
            Self::DelegationCompleted {
                parent_conversation_id,
                ..
            } => Some(parent_conversation_id),
            Self::MessageAdded { conversation_id, .. } => Some(conversation_id),
            Self::TokenUsage { conversation_id, .. } => Some(conversation_id),
        }
    }
}
