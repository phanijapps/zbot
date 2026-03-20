// ============================================================================
// STREAM EVENT TYPES
// Events emitted during agent execution
// ============================================================================

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Events emitted during agent execution
///
/// These events are streamed to the frontend/application layer
/// to provide real-time feedback during agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Metadata about the execution (agent info, model, etc.)
    #[serde(rename = "metadata")]
    Metadata {
        timestamp: u64,
        agent_id: String,
        model: String,
        provider: String,
    },

    /// A token from the LLM response
    #[serde(rename = "token")]
    Token {
        timestamp: u64,
        content: String,
    },

    /// Reasoning/thinking content from the LLM
    #[serde(rename = "reasoning")]
    Reasoning {
        timestamp: u64,
        content: String,
    },

    /// A tool call has started
    #[serde(rename = "tool_call_start")]
    ToolCallStart {
        timestamp: u64,
        tool_id: String,
        tool_name: String,
        args: Value,
    },

    /// A tool call has completed
    #[serde(rename = "tool_call_end")]
    ToolCallEnd {
        timestamp: u64,
        tool_id: String,
        tool_name: String,
        args: Value,
    },

    /// Result from a tool execution
    #[serde(rename = "tool_result")]
    ToolResult {
        timestamp: u64,
        tool_id: String,
        result: String,
        error: Option<String>,
    },

    /// Execution is complete
    #[serde(rename = "done")]
    Done {
        timestamp: u64,
        final_message: String,
        token_count: usize,
    },

    /// An error occurred during execution
    #[serde(rename = "error")]
    Error {
        timestamp: u64,
        error: String,
        recoverable: bool,
    },

    // ========================================================================
    // GENERATIVE UI EVENTS
    // ========================================================================

    /// Request to display content to the user
    #[serde(rename = "show_content")]
    ShowContent {
        timestamp: u64,
        content_type: String,
        title: String,
        content: String,
        metadata: Option<Value>,
        file_path: Option<String>,
        is_attachment: Option<bool>,
        base64: Option<bool>,
    },

    /// Request to input from the user
    #[serde(rename = "request_input")]
    RequestInput {
        timestamp: u64,
        form_id: String,
        form_type: String,
        title: String,
        description: Option<String>,
        schema: Value,
        submit_button: Option<String>,
    },

    // ========================================================================
    // ACTION EVENTS
    // ========================================================================

    /// Respond action from the respond tool.
    /// Signals that a response should be sent to the originating hook.
    #[serde(rename = "action_respond")]
    ActionRespond {
        timestamp: u64,
        message: String,
        format: String,
        conversation_id: Option<String>,
        session_id: Option<String>,
    },

    /// Delegate action from the delegate tool.
    /// Signals that a task should be delegated to a subagent.
    #[serde(rename = "action_delegate")]
    ActionDelegate {
        timestamp: u64,
        agent_id: String,
        task: String,
        context: Option<Value>,
        wait_for_result: bool,
    },

    /// Plan update action from the update_plan tool.
    /// Signals that the agent's plan has been updated.
    #[serde(rename = "action_plan_update")]
    ActionPlanUpdate {
        timestamp: u64,
        plan: Value,
        explanation: Option<String>,
    },

    // ========================================================================
    // METRICS EVENTS
    // ========================================================================

    /// Token usage update after an LLM call.
    /// Cumulative counts of tokens consumed in the session.
    #[serde(rename = "token_update")]
    TokenUpdate {
        timestamp: u64,
        /// Cumulative input tokens (prompt tokens)
        tokens_in: u64,
        /// Cumulative output tokens (completion tokens)
        tokens_out: u64,
    },

    // ========================================================================
    // CHECKPOINT EVENTS
    // ========================================================================

    /// Execution heartbeat — emitted during silent phases (e.g., LLM reasoning)
    /// to signal the execution is still alive.
    #[serde(rename = "heartbeat")]
    Heartbeat {
        timestamp: u64,
    },

    /// Execution context state for checkpoint persistence.
    ///
    /// Emitted at the end of execution (after Done) to allow the gateway
    /// to persist the context state for session resumption. Contains skill
    /// tracking information and other tool context state.
    #[serde(rename = "context_state")]
    ContextState {
        /// Timestamp when state was captured
        timestamp: u64,
        /// Serialized tool context state (skill graph, loaded skills, etc.)
        state: Value,
    },

    // ========================================================================
    // WARD EVENTS
    // ========================================================================

    /// Agent switched to a different ward (project directory).
    #[serde(rename = "ward_changed")]
    WardChanged {
        timestamp: u64,
        /// The ward the agent switched to
        ward_id: String,
    },

    /// Executor auto-extended iterations because the agent is making progress.
    #[serde(rename = "iterations_extended")]
    IterationsExtended {
        timestamp: u64,
        /// Total iterations used so far
        iterations_used: u32,
        /// Additional iterations granted
        iterations_added: u32,
        /// Human-readable reason for extension
        reason: String,
    },
}

impl StreamEvent {
    /// Get the timestamp for this event
    #[must_use]
    pub const fn timestamp(&self) -> u64 {
        match self {
            Self::Metadata { timestamp, .. }
            | Self::Token { timestamp, .. }
            | Self::Reasoning { timestamp, .. }
            | Self::ToolCallStart { timestamp, .. }
            | Self::ToolCallEnd { timestamp, .. }
            | Self::ToolResult { timestamp, .. }
            | Self::Done { timestamp, .. }
            | Self::Error { timestamp, .. }
            | Self::ShowContent { timestamp, .. }
            | Self::RequestInput { timestamp, .. }
            | Self::ActionRespond { timestamp, .. }
            | Self::ActionDelegate { timestamp, .. }
            | Self::ActionPlanUpdate { timestamp, .. }
            | Self::TokenUpdate { timestamp, .. }
            | Self::Heartbeat { timestamp, .. }
            | Self::ContextState { timestamp, .. }
            | Self::WardChanged { timestamp, .. }
            | Self::IterationsExtended { timestamp, .. } => *timestamp,
        }
    }

    /// Check if this event is a terminal event (execution complete)
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::Error { .. })
    }
}

/// Helper function to get current timestamp in milliseconds
#[must_use]
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::Token {
            timestamp: 12345,
            content: "Hello".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"token\""));
    }

    #[test]
    fn test_terminal_event_detection() {
        assert!(StreamEvent::Done {
            timestamp: 0,
            final_message: String::new(),
            token_count: 0,
        }.is_terminal());

        assert!(StreamEvent::Error {
            timestamp: 0,
            error: String::new(),
            recoverable: false,
        }.is_terminal());

        assert!(!StreamEvent::Token {
            timestamp: 0,
            content: String::new(),
        }.is_terminal());
    }
}
