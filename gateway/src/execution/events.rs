//! # Execution Events
//!
//! Event conversion and emission helpers for agent execution.

use crate::events::GatewayEvent;
use agent_runtime::StreamEvent;

/// Convert an agent runtime stream event to a gateway event.
///
/// This function maps the internal executor events to gateway-level events
/// that can be broadcast to connected clients.
///
/// Note: This function takes session_id as input, but execution_id is not available
/// at this level. Callers should update the execution_id if needed.
pub fn convert_stream_event(
    event: StreamEvent,
    agent_id: &str,
    conversation_id: &str,
    session_id: &str,
) -> GatewayEvent {
    // For backward compatibility, use session_id as execution_id when not available
    let execution_id = session_id;

    match event {
        StreamEvent::Metadata { .. } => GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::Token { content, .. } => GatewayEvent::Token {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            delta: content,
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::Reasoning { content, .. } => GatewayEvent::Thinking {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            content,
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::ToolCallStart {
            tool_id, tool_name, args, ..
        } => GatewayEvent::ToolCall {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            tool_id,
            tool_name,
            args,
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::ToolResult {
            tool_id, result, error, ..
        } => GatewayEvent::ToolResult {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            tool_id,
            result,
            error,
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::Done { final_message, .. } => GatewayEvent::TurnComplete {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            message: final_message,
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::Error { error, .. } => GatewayEvent::Error {
            agent_id: Some(agent_id.to_string()),
            session_id: Some(session_id.to_string()),
            execution_id: Some(execution_id.to_string()),
            message: error,
            conversation_id: Some(conversation_id.to_string()),
        },
        // Action events from tools
        StreamEvent::ActionRespond {
            message,
            session_id: respond_session_id,
            ..
        } => GatewayEvent::Respond {
            session_id: respond_session_id.unwrap_or_else(|| session_id.to_string()),
            execution_id: execution_id.to_string(),
            message,
            conversation_id: Some(conversation_id.to_string()),
        },
        StreamEvent::ActionDelegate {
            agent_id: child_agent_id,
            task,
            ..
        } => GatewayEvent::DelegationStarted {
            session_id: session_id.to_string(),
            parent_execution_id: execution_id.to_string(),
            child_execution_id: format!("{}-sub", execution_id),
            parent_agent_id: agent_id.to_string(),
            child_agent_id,
            task,
            parent_conversation_id: Some(conversation_id.to_string()),
            child_conversation_id: Some(format!("{}-sub", conversation_id)),
        },
        // Handle other event types (ToolCallEnd, ShowContent, RequestInput)
        // These don't have direct gateway equivalents, so emit a no-op event
        _ => GatewayEvent::AgentStarted {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            conversation_id: Some(conversation_id.to_string()),
        },
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

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1");

        match gateway_event {
            GatewayEvent::Token { agent_id, session_id, delta, conversation_id, .. } => {
                assert_eq!(agent_id, "agent-1");
                assert_eq!(session_id, "session-1");
                assert_eq!(delta, "Hello");
                assert_eq!(conversation_id, Some("conv-1".to_string()));
            }
            _ => panic!("Expected Token event"),
        }
    }

    #[test]
    fn test_convert_error_event() {
        let event = StreamEvent::Error {
            timestamp: 0,
            error: "Something went wrong".to_string(),
            recoverable: false,
        };

        let gateway_event = convert_stream_event(event, "agent-1", "conv-1", "session-1");

        match gateway_event {
            GatewayEvent::Error { agent_id, session_id, message, conversation_id, .. } => {
                assert_eq!(agent_id, Some("agent-1".to_string()));
                assert_eq!(session_id, Some("session-1".to_string()));
                assert_eq!(message, "Something went wrong");
                assert_eq!(conversation_id, Some("conv-1".to_string()));
            }
            _ => panic!("Expected Error event"),
        }
    }
}
