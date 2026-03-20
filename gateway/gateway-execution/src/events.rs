//! # Execution Events
//!
//! Event conversion and emission helpers for agent execution.

use gateway_events::GatewayEvent;
use agent_runtime::StreamEvent;

/// Convert an agent runtime stream event to a gateway event.
///
/// This function maps the internal executor events to gateway-level events
/// that can be broadcast to connected clients.
///
/// Returns `None` for internal events that should not be broadcast to the UI
/// (e.g., `ContextState` for checkpoint persistence).
pub fn convert_stream_event(
    event: StreamEvent,
    agent_id: &str,
    conversation_id: &str,
    session_id: &str,
    execution_id: &str,
) -> Option<GatewayEvent> {

    match event {
        StreamEvent::Metadata { .. } => Some(GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::Token { content, .. } => Some(GatewayEvent::Token {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            delta: content,
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::Reasoning { content, .. } => Some(GatewayEvent::Thinking {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            content,
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::ToolCallStart {
            tool_id, tool_name, args, ..
        } => Some(GatewayEvent::ToolCall {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            tool_id,
            tool_name,
            args,
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::ToolResult {
            tool_id, result, error, ..
        } => Some(GatewayEvent::ToolResult {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            tool_id,
            result,
            error,
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::Done { final_message, .. } => Some(GatewayEvent::TurnComplete {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            message: final_message,
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::Error { error, .. } => Some(GatewayEvent::Error {
            agent_id: Some(agent_id.to_string()),
            session_id: Some(session_id.to_string()),
            execution_id: Some(execution_id.to_string()),
            message: error,
            conversation_id: Some(conversation_id.to_string()),
        }),
        // Action events from tools
        StreamEvent::ActionRespond {
            message,
            session_id: respond_session_id,
            ..
        } => Some(GatewayEvent::Respond {
            session_id: respond_session_id.unwrap_or_else(|| session_id.to_string()),
            execution_id: execution_id.to_string(),
            message,
            conversation_id: Some(conversation_id.to_string()),
        }),
        // ActionDelegate is handled by the runner/delegation system directly,
        // which emits DelegationStarted with proper IDs. Don't emit here to avoid
        // duplicate events or confusing the UI.
        StreamEvent::ActionDelegate { .. } => None,
        // Heartbeat signals execution is alive during silent phases (e.g., LLM reasoning).
        StreamEvent::Heartbeat { .. } => Some(GatewayEvent::Heartbeat {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            conversation_id: Some(conversation_id.to_string()),
        }),
        // ContextState is internal state for checkpoint persistence - don't emit to UI.
        StreamEvent::ContextState { .. } => None,
        // Ward changed - agent switched to a different project directory.
        StreamEvent::WardChanged { ward_id, .. } => Some(GatewayEvent::WardChanged {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            ward_id,
        }),
        // IterationsExtended — auto-extension event from executor
        StreamEvent::IterationsExtended {
            iterations_used, iterations_added, reason, ..
        } => Some(GatewayEvent::IterationsExtended {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            iterations_used,
            iterations_added,
            reason,
            conversation_id: Some(conversation_id.to_string()),
        }),
        StreamEvent::ActionPlanUpdate { plan, explanation, .. } => Some(GatewayEvent::PlanUpdate {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            plan,
            explanation,
            conversation_id: Some(conversation_id.to_string()),
        }),
        // Handle other event types (ToolCallEnd, ShowContent, RequestInput, TokenUpdate)
        // These don't have direct gateway equivalents or are handled separately.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_token_event() {
        let event = StreamEvent::Token {
            timestamp: 0,
            content: "Hello".to_string(),
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1", "exec-1");

        match gateway_event {
            Some(GatewayEvent::Token { agent_id, session_id, execution_id, delta, conversation_id, .. }) => {
                assert_eq!(agent_id, "agent-1");
                assert_eq!(session_id, "session-1");
                assert_eq!(execution_id, "exec-1");
                assert_eq!(delta, "Hello");
                assert_eq!(conversation_id, Some("conv-1".to_string()));
            }
            _ => panic!("Expected Some(Token) event"),
        }
    }

    #[test]
    fn test_convert_error_event() {
        let event = StreamEvent::Error {
            timestamp: 0,
            error: "Something went wrong".to_string(),
            recoverable: false,
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1", "exec-1");

        match gateway_event {
            Some(GatewayEvent::Error { agent_id, session_id, execution_id, message, conversation_id, .. }) => {
                assert_eq!(agent_id, Some("agent-1".to_string()));
                assert_eq!(session_id, Some("session-1".to_string()));
                assert_eq!(execution_id, Some("exec-1".to_string()));
                assert_eq!(message, "Something went wrong");
                assert_eq!(conversation_id, Some("conv-1".to_string()));
            }
            _ => panic!("Expected Some(Error) event"),
        }
    }

    #[test]
    fn test_convert_context_state_returns_none() {
        let event = StreamEvent::ContextState {
            timestamp: 0,
            state: serde_json::json!({"skill:graph": {}}),
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1", "exec-1");
        assert!(gateway_event.is_none(), "ContextState should return None");
    }

    #[test]
    fn test_convert_heartbeat_event() {
        let event = StreamEvent::Heartbeat {
            timestamp: 12345,
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1", "exec-1");

        match gateway_event {
            Some(GatewayEvent::Heartbeat { session_id, execution_id, conversation_id }) => {
                assert_eq!(session_id, "session-1");
                assert_eq!(execution_id, "exec-1");
                assert_eq!(conversation_id, Some("conv-1".to_string()));
            }
            _ => panic!("Expected Some(Heartbeat) event"),
        }
    }

    #[test]
    fn test_convert_action_delegate_returns_none() {
        let event = StreamEvent::ActionDelegate {
            timestamp: 0,
            agent_id: "child-agent".to_string(),
            task: "do something".to_string(),
            context: None,
            wait_for_result: false,
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1", "exec-1");
        assert!(gateway_event.is_none(), "ActionDelegate should return None");
    }
}
