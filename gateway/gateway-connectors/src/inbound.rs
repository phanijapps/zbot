//! # Inbound Types
//!
//! Types for messages received from external connectors.

use serde::{Deserialize, Serialize};

/// Payload received from a connector on the inbound path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundPayload {
    /// The message text from the external service.
    pub message: String,
    /// External thread ID for conversation threading.
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Who sent the message.
    #[serde(default)]
    pub sender: Option<InboundSender>,
    /// Route to a specific agent (defaults to "root").
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Override response routing (defaults to [connector_id]).
    #[serde(default)]
    pub respond_to: Option<Vec<String>>,
    /// Arbitrary metadata passed through to the session.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Sender information from the external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundSender {
    /// External user/sender ID.
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: Option<String>,
}

/// Result returned after accepting an inbound message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundResult {
    /// Session ID created for this inbound message.
    pub session_id: String,
    /// Whether the message was accepted.
    pub accepted: bool,
}

/// Log entry for an inbound message (stored in-memory ring buffer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundLogEntry {
    /// Connector that received the message.
    pub connector_id: String,
    /// The message text.
    pub message: String,
    /// Who sent the message.
    #[serde(default)]
    pub sender: Option<InboundSender>,
    /// External thread ID.
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Session ID created for this message.
    pub session_id: String,
    /// When the message was received.
    pub received_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_payload_minimal() {
        let json = r#"{"message": "Hello from Slack"}"#;
        let payload: InboundPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.message, "Hello from Slack");
        assert!(payload.thread_id.is_none());
        assert!(payload.sender.is_none());
        assert!(payload.agent_id.is_none());
        assert!(payload.respond_to.is_none());
    }

    #[test]
    fn test_inbound_payload_full() {
        let json = r#"{
            "message": "Hello",
            "thread_id": "thread-123",
            "sender": { "id": "U123", "name": "Alice" },
            "agent_id": "researcher",
            "respond_to": ["slack", "email"],
            "metadata": { "channel": "general" }
        }"#;
        let payload: InboundPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.message, "Hello");
        assert_eq!(payload.thread_id.unwrap(), "thread-123");
        assert_eq!(payload.sender.as_ref().unwrap().id, "U123");
        assert_eq!(
            payload.sender.as_ref().unwrap().name.as_deref(),
            Some("Alice")
        );
        assert_eq!(payload.agent_id.unwrap(), "researcher");
        assert_eq!(payload.respond_to.unwrap().len(), 2);
    }

    #[test]
    fn test_inbound_result_serialization() {
        let result = InboundResult {
            session_id: "sess-abc123".to_string(),
            accepted: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: InboundResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, "sess-abc123");
        assert!(parsed.accepted);
    }

    #[test]
    fn test_inbound_log_entry_serialization() {
        let entry = InboundLogEntry {
            connector_id: "slack".to_string(),
            message: "Hello from Slack".to_string(),
            sender: Some(InboundSender {
                id: "U123".to_string(),
                name: Some("Alice".to_string()),
            }),
            thread_id: Some("thread-456".to_string()),
            session_id: "sess-abc".to_string(),
            received_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: InboundLogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.connector_id, "slack");
        assert_eq!(parsed.message, "Hello from Slack");
        assert_eq!(parsed.session_id, "sess-abc");
        assert!(parsed.sender.is_some());
    }
}
