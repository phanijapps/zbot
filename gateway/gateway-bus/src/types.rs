//! Types for the Gateway Bus.

use execution_state::TriggerSource;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A request to submit a session to the gateway.
///
/// This struct contains all the information needed to start a new session
/// or continue an existing one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRequest {
    /// Existing session ID to continue, or None to create a new session.
    pub session_id: Option<String>,

    /// The trigger source for this session (web, cli, cron, api, plugin).
    #[serde(default)]
    pub source: TriggerSource,

    /// The agent ID to execute.
    pub agent_id: String,

    /// The message to send to the agent.
    pub message: String,

    /// Optional priority for queue ordering (lower = higher priority).
    /// Only used when session queuing is enabled.
    pub priority: Option<u32>,

    /// Optional external reference ID (e.g., email message ID, webhook event ID).
    /// Useful for correlating sessions with external systems.
    pub external_ref: Option<String>,

    /// Optional metadata for the session.
    pub metadata: Option<serde_json::Value>,

    /// Optional conversation ID for legacy message persistence.
    /// If not provided, will be auto-generated.
    pub conversation_id: Option<String>,

    /// Connector IDs to send the response to at end of execution.
    /// If empty/None, response goes to web UI only (default behavior).
    /// Original trigger source is NOT automatically included (explicit routing).
    #[serde(default)]
    pub respond_to: Option<Vec<String>>,

    /// Connector ID that triggered this session (for correlation).
    #[serde(default)]
    pub connector_id: Option<String>,

    /// Thread ID for conversation threading with external connectors.
    #[serde(default)]
    pub thread_id: Option<String>,
}

impl SessionRequest {
    /// Create a new session request with the minimum required fields.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - The agent to execute
    /// * `message` - The message to send to the agent
    pub fn new(agent_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            session_id: None,
            source: TriggerSource::default(),
            agent_id: agent_id.into(),
            message: message.into(),
            priority: None,
            external_ref: None,
            metadata: None,
            conversation_id: None,
            respond_to: None,
            connector_id: None,
            thread_id: None,
        }
    }

    /// Set the session ID to continue an existing session.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the trigger source.
    pub fn with_source(mut self, source: TriggerSource) -> Self {
        self.source = source;
        self
    }

    /// Set the priority for queue ordering.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set the external reference ID.
    pub fn with_external_ref(mut self, external_ref: impl Into<String>) -> Self {
        self.external_ref = Some(external_ref.into());
        self
    }

    /// Set the metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the conversation ID for legacy message persistence.
    pub fn with_conversation_id(mut self, conversation_id: impl Into<String>) -> Self {
        self.conversation_id = Some(conversation_id.into());
        self
    }

    /// Set the connector IDs to send the response to at end of execution.
    pub fn with_respond_to(mut self, connector_ids: Vec<String>) -> Self {
        self.respond_to = Some(connector_ids);
        self
    }

    /// Set the connector ID that triggered this session.
    pub fn with_connector_id(mut self, connector_id: impl Into<String>) -> Self {
        self.connector_id = Some(connector_id.into());
        self
    }

    /// Set the thread ID for conversation threading.
    pub fn with_thread_id(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }
}

/// Handle returned when a session is submitted.
///
/// Contains the IDs needed to track and control the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHandle {
    /// The session ID (sess-{uuid}).
    pub session_id: String,

    /// The execution ID for this specific agent invocation (exec-{uuid}).
    pub execution_id: String,

    /// The conversation ID used for message persistence.
    pub conversation_id: String,
}

impl SessionHandle {
    /// Create a new session handle.
    pub fn new(
        session_id: impl Into<String>,
        execution_id: impl Into<String>,
        conversation_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            execution_id: execution_id.into(),
            conversation_id: conversation_id.into(),
        }
    }
}

/// Errors that can occur when using the Gateway Bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusError {
    /// Session not found.
    SessionNotFound(String),

    /// Execution not found.
    ExecutionNotFound(String),

    /// Agent not found or failed to load.
    AgentError(String),

    /// Provider not found or failed to load.
    ProviderError(String),

    /// Session is in an invalid state for the requested operation.
    InvalidState {
        session_id: String,
        current_state: String,
        expected_states: Vec<String>,
    },

    /// Internal error.
    Internal(String),
}

impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusError::SessionNotFound(id) => write!(f, "Session not found: {}", id),
            BusError::ExecutionNotFound(id) => write!(f, "Execution not found: {}", id),
            BusError::AgentError(msg) => write!(f, "Agent error: {}", msg),
            BusError::ProviderError(msg) => write!(f, "Provider error: {}", msg),
            BusError::InvalidState {
                session_id,
                current_state,
                expected_states,
            } => {
                write!(
                    f,
                    "Session {} is in state '{}', expected one of: {:?}",
                    session_id, current_state, expected_states
                )
            }
            BusError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for BusError {}

impl From<String> for BusError {
    fn from(s: String) -> Self {
        BusError::Internal(s)
    }
}

impl From<&str> for BusError {
    fn from(s: &str) -> Self {
        BusError::Internal(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== SessionRequest Tests ====================

    #[test]
    fn session_request_new() {
        let req = SessionRequest::new("root", "Hello!");

        assert_eq!(req.agent_id, "root");
        assert_eq!(req.message, "Hello!");
        assert_eq!(req.source, TriggerSource::Web); // default
        assert!(req.session_id.is_none());
        assert!(req.priority.is_none());
        assert!(req.external_ref.is_none());
        assert!(req.metadata.is_none());
        assert!(req.conversation_id.is_none());
    }

    #[test]
    fn session_request_builder_all_fields() {
        let metadata = serde_json::json!({"key": "value", "count": 42});
        let request = SessionRequest::new("agent", "msg")
            .with_source(TriggerSource::Plugin)
            .with_session_id("sess-123")
            .with_priority(10)
            .with_external_ref("ext-ref")
            .with_metadata(metadata.clone())
            .with_conversation_id("conv-456");

        assert_eq!(request.agent_id, "agent");
        assert_eq!(request.message, "msg");
        assert_eq!(request.source, TriggerSource::Plugin);
        assert_eq!(request.session_id, Some("sess-123".to_string()));
        assert_eq!(request.priority, Some(10));
        assert_eq!(request.external_ref, Some("ext-ref".to_string()));
        assert_eq!(request.metadata, Some(metadata));
        assert_eq!(request.conversation_id, Some("conv-456".to_string()));
    }

    #[test]
    fn session_request_continue_session() {
        let request = SessionRequest::new("root", "Follow up message")
            .with_session_id("sess-existing");

        assert_eq!(request.session_id, Some("sess-existing".to_string()));
        assert_eq!(request.message, "Follow up message");
        assert_eq!(request.agent_id, "root");
    }

    #[test]
    fn session_request_with_each_source() {
        let sources = [
            TriggerSource::Web,
            TriggerSource::Cli,
            TriggerSource::Cron,
            TriggerSource::Api,
            TriggerSource::Plugin,
        ];

        for source in sources {
            let req = SessionRequest::new("root", "test").with_source(source.clone());
            assert_eq!(req.source, source);
        }
    }

    #[test]
    fn session_request_json_deserialization_full() {
        let json = r#"{
            "agent_id": "root",
            "message": "Hello!",
            "source": "plugin",
            "priority": 5,
            "external_ref": "test-ref",
            "session_id": "sess-existing",
            "conversation_id": "conv-123",
            "metadata": {"custom": "data"}
        }"#;

        let req: SessionRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.agent_id, "root");
        assert_eq!(req.message, "Hello!");
        assert_eq!(req.source, TriggerSource::Plugin);
        assert_eq!(req.priority, Some(5));
        assert_eq!(req.external_ref, Some("test-ref".to_string()));
        assert_eq!(req.session_id, Some("sess-existing".to_string()));
        assert_eq!(req.conversation_id, Some("conv-123".to_string()));
        assert!(req.metadata.is_some());
    }

    #[test]
    fn session_request_json_minimal() {
        let json = r#"{
            "agent_id": "root",
            "message": "Hi"
        }"#;

        let req: SessionRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.agent_id, "root");
        assert_eq!(req.message, "Hi");
        assert_eq!(req.source, TriggerSource::Web); // default
        assert!(req.session_id.is_none());
        assert!(req.priority.is_none());
        assert!(req.external_ref.is_none());
        assert!(req.metadata.is_none());
        assert!(req.conversation_id.is_none());
    }

    #[test]
    fn session_request_serialization_roundtrip() {
        let original = SessionRequest::new("test-agent", "test message")
            .with_source(TriggerSource::Cron)
            .with_priority(100)
            .with_external_ref("cron-job-1");

        let json = serde_json::to_string(&original).unwrap();
        let parsed: SessionRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.agent_id, original.agent_id);
        assert_eq!(parsed.message, original.message);
        assert_eq!(parsed.source, original.source);
        assert_eq!(parsed.priority, original.priority);
        assert_eq!(parsed.external_ref, original.external_ref);
    }

    // ==================== SessionHandle Tests ====================

    #[test]
    fn session_handle_new() {
        let handle = SessionHandle::new("sess-1", "exec-1", "conv-1");

        assert_eq!(handle.session_id, "sess-1");
        assert_eq!(handle.execution_id, "exec-1");
        assert_eq!(handle.conversation_id, "conv-1");
    }

    #[test]
    fn session_handle_serialization() {
        let handle = SessionHandle::new("sess-abc", "exec-def", "conv-ghi");

        let json = serde_json::to_string(&handle).unwrap();

        assert!(json.contains("sess-abc"));
        assert!(json.contains("exec-def"));
        assert!(json.contains("conv-ghi"));
        assert!(json.contains("session_id"));
        assert!(json.contains("execution_id"));
        assert!(json.contains("conversation_id"));
    }

    #[test]
    fn session_handle_deserialization() {
        let json = r#"{
            "session_id": "sess-123",
            "execution_id": "exec-456",
            "conversation_id": "conv-789"
        }"#;

        let handle: SessionHandle = serde_json::from_str(json).unwrap();

        assert_eq!(handle.session_id, "sess-123");
        assert_eq!(handle.execution_id, "exec-456");
        assert_eq!(handle.conversation_id, "conv-789");
    }

    #[test]
    fn session_handle_roundtrip() {
        let original = SessionHandle::new("sess-test", "exec-test", "conv-test");

        let json = serde_json::to_string(&original).unwrap();
        let parsed: SessionHandle = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.session_id, original.session_id);
        assert_eq!(parsed.execution_id, original.execution_id);
        assert_eq!(parsed.conversation_id, original.conversation_id);
    }

    // ==================== BusError Tests ====================

    #[test]
    fn bus_error_session_not_found_display() {
        let err = BusError::SessionNotFound("sess-123".to_string());
        let msg = err.to_string();

        assert!(msg.contains("sess-123"));
        assert!(msg.to_lowercase().contains("session"));
        assert!(msg.to_lowercase().contains("not found"));
    }

    #[test]
    fn bus_error_execution_not_found_display() {
        let err = BusError::ExecutionNotFound("exec-456".to_string());
        let msg = err.to_string();

        assert!(msg.contains("exec-456"));
        assert!(msg.to_lowercase().contains("execution"));
        assert!(msg.to_lowercase().contains("not found"));
    }

    #[test]
    fn bus_error_agent_error_display() {
        let err = BusError::AgentError("Agent 'researcher' not found".to_string());
        let msg = err.to_string();

        assert!(msg.contains("researcher"));
        assert!(msg.to_lowercase().contains("agent"));
    }

    #[test]
    fn bus_error_provider_error_display() {
        let err = BusError::ProviderError("OpenAI API key not set".to_string());
        let msg = err.to_string();

        assert!(msg.contains("OpenAI"));
        assert!(msg.to_lowercase().contains("provider"));
    }

    #[test]
    fn bus_error_invalid_state_display() {
        let err = BusError::InvalidState {
            session_id: "sess-123".to_string(),
            current_state: "completed".to_string(),
            expected_states: vec!["running".to_string(), "paused".to_string()],
        };
        let msg = err.to_string();

        assert!(msg.contains("sess-123"));
        assert!(msg.contains("completed"));
        assert!(msg.contains("running") || msg.contains("paused"));
    }

    #[test]
    fn bus_error_internal_display() {
        let err = BusError::Internal("Database connection failed".to_string());
        let msg = err.to_string();

        assert!(msg.contains("Database connection failed"));
        assert!(msg.to_lowercase().contains("internal"));
    }

    #[test]
    fn bus_error_from_string() {
        let err: BusError = "Something went wrong".to_string().into();

        match err {
            BusError::Internal(msg) => assert_eq!(msg, "Something went wrong"),
            _ => panic!("Expected BusError::Internal"),
        }
    }

    #[test]
    fn bus_error_from_str() {
        let err: BusError = "Error message".into();

        match err {
            BusError::Internal(msg) => assert_eq!(msg, "Error message"),
            _ => panic!("Expected BusError::Internal"),
        }
    }

    #[test]
    fn bus_error_serialization() {
        let errors = vec![
            BusError::SessionNotFound("sess-1".to_string()),
            BusError::ExecutionNotFound("exec-1".to_string()),
            BusError::AgentError("test error".to_string()),
            BusError::ProviderError("provider error".to_string()),
            BusError::InvalidState {
                session_id: "sess-1".to_string(),
                current_state: "completed".to_string(),
                expected_states: vec!["running".to_string()],
            },
            BusError::Internal("internal error".to_string()),
        ];

        for err in errors {
            let json = serde_json::to_string(&err).unwrap();
            let parsed: BusError = serde_json::from_str(&json).unwrap();
            assert_eq!(err.to_string(), parsed.to_string());
        }
    }

    #[test]
    fn bus_error_is_std_error() {
        let err = BusError::SessionNotFound("test".to_string());
        let _: &dyn std::error::Error = &err;
    }
}
