//! Test utilities for gateway tests.
//!
//! Provides helper functions for creating test fixtures and mock data.

use crate::bus::{SessionHandle, SessionRequest};
use execution_state::TriggerSource;

/// Create a test session request.
pub fn mock_session_request(agent_id: &str, message: &str) -> SessionRequest {
    SessionRequest::new(agent_id, message)
}

/// Create a test session request with a specific source.
pub fn mock_session_request_with_source(
    agent_id: &str,
    message: &str,
    source: TriggerSource,
) -> SessionRequest {
    SessionRequest::new(agent_id, message).with_source(source)
}

/// Create a test session handle.
pub fn mock_session_handle(
    session_id: &str,
    execution_id: &str,
    conversation_id: &str,
) -> SessionHandle {
    SessionHandle {
        session_id: session_id.to_string(),
        execution_id: execution_id.to_string(),
        conversation_id: conversation_id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_session_request() {
        let req = mock_session_request("root", "Hello!");

        assert_eq!(req.agent_id, "root");
        assert_eq!(req.message, "Hello!");
        assert_eq!(req.source, TriggerSource::Web); // default
    }

    #[test]
    fn test_mock_session_request_with_source() {
        let req = mock_session_request_with_source("agent", "Test", TriggerSource::Connector);

        assert_eq!(req.source, TriggerSource::Connector);
    }

    #[test]
    fn test_mock_session_handle() {
        let handle = mock_session_handle("sess-1", "exec-1", "conv-1");

        assert_eq!(handle.session_id, "sess-1");
        assert_eq!(handle.execution_id, "exec-1");
        assert_eq!(handle.conversation_id, "conv-1");
    }
}
