//! # Bridge Protocol
//!
//! All message types for the Worker <-> AgentZero WebSocket protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// WORKER → AGENTZERO
// ============================================================================

/// Messages sent from a worker to the AgentZero server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerMessage {
    /// Initial handshake: worker declares its identity and capabilities.
    Hello {
        /// Unique worker/adapter identifier (matches connector config).
        adapter_id: String,
        /// Capabilities this worker can execute (outbound actions).
        #[serde(default)]
        capabilities: Vec<WorkerCapability>,
        /// Resources this worker exposes for querying.
        #[serde(default)]
        resources: Vec<WorkerResource>,
        /// Resume state for replay (last acknowledged outbox ID).
        #[serde(default)]
        resume: Option<ResumeState>,
    },

    /// Inbound message from the external service (user → agent).
    Inbound {
        /// Message text.
        text: String,
        /// External thread ID for conversation threading.
        #[serde(default)]
        thread_id: Option<String>,
        /// Who sent the message.
        #[serde(default)]
        sender: Option<InboundSender>,
        /// Route to a specific agent (defaults to "root").
        #[serde(default)]
        agent_id: Option<String>,
        /// Arbitrary metadata.
        #[serde(default)]
        metadata: Option<Value>,
    },

    /// Acknowledge successful delivery of an outbox item.
    Ack {
        /// The outbox item ID that was delivered.
        outbox_id: String,
    },

    /// Report failed delivery of an outbox item.
    Fail {
        /// The outbox item ID that failed.
        outbox_id: String,
        /// Error description.
        error: String,
        /// Optional retry delay in seconds.
        #[serde(default)]
        retry_after_seconds: Option<u64>,
    },

    /// Response to a ResourceQuery.
    ResourceResponse {
        /// Correlation ID matching the original query.
        request_id: String,
        /// Query result data.
        data: Value,
    },

    /// Response to a CapabilityInvoke.
    CapabilityResponse {
        /// Correlation ID matching the original invocation.
        request_id: String,
        /// Invocation result.
        result: Value,
    },

    /// Heartbeat response.
    Pong,
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

/// Resume state sent during Hello for outbox replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeState {
    /// Last outbox ID the worker successfully processed.
    pub last_acked_id: String,
}

// ============================================================================
// AGENTZERO → WORKER
// ============================================================================

/// Messages sent from the AgentZero server to a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeServerMessage {
    /// Acknowledge a successful Hello handshake.
    HelloAck {
        /// Server timestamp.
        server_time: String,
        /// Heartbeat interval in seconds (worker should respond to Pings).
        heartbeat_seconds: u64,
    },

    /// Push an outbox item to the worker for delivery.
    OutboxItem {
        /// Outbox item ID (worker must ACK or FAIL this).
        outbox_id: String,
        /// Capability name to invoke on the worker side.
        capability: String,
        /// Payload data.
        payload: Value,
    },

    /// Query a resource from the worker.
    ResourceQuery {
        /// Correlation ID (worker must include in ResourceResponse).
        request_id: String,
        /// Resource name to query.
        resource: String,
        /// Optional query parameters.
        #[serde(default)]
        params: Option<Value>,
    },

    /// Invoke a capability on the worker.
    CapabilityInvoke {
        /// Correlation ID (worker must include in CapabilityResponse).
        request_id: String,
        /// Capability name.
        capability: String,
        /// Payload data.
        payload: Value,
    },

    /// Heartbeat ping.
    Ping,

    /// Error message.
    Error {
        /// Error description.
        message: String,
    },
}

// ============================================================================
// SHARED TYPES
// ============================================================================

/// A capability that a worker can execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerCapability {
    /// Capability name (e.g., "send_message", "create_ticket").
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON schema for the capability payload.
    #[serde(default)]
    pub schema: Option<Value>,
}

/// A resource that a worker exposes for querying.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResource {
    /// Resource name (e.g., "contacts", "channels").
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_hello_roundtrip() {
        let msg = WorkerMessage::Hello {
            adapter_id: "slack-1".to_string(),
            capabilities: vec![WorkerCapability {
                name: "send_message".to_string(),
                description: Some("Send a message".to_string()),
                schema: None,
            }],
            resources: vec![WorkerResource {
                name: "channels".to_string(),
                description: Some("List channels".to_string()),
            }],
            resume: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WorkerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            WorkerMessage::Hello {
                adapter_id,
                capabilities,
                resources,
                ..
            } => {
                assert_eq!(adapter_id, "slack-1");
                assert_eq!(capabilities.len(), 1);
                assert_eq!(resources.len(), 1);
            }
            _ => panic!("Expected Hello"),
        }
    }

    #[test]
    fn test_worker_hello_minimal() {
        let json = r#"{"type":"hello","adapter_id":"test-1"}"#;
        let msg: WorkerMessage = serde_json::from_str(json).unwrap();
        match msg {
            WorkerMessage::Hello {
                adapter_id,
                capabilities,
                resources,
                resume,
            } => {
                assert_eq!(adapter_id, "test-1");
                assert!(capabilities.is_empty());
                assert!(resources.is_empty());
                assert!(resume.is_none());
            }
            _ => panic!("Expected Hello"),
        }
    }

    #[test]
    fn test_worker_hello_with_resume() {
        let json = r#"{"type":"hello","adapter_id":"test-1","resume":{"last_acked_id":"obx-123"}}"#;
        let msg: WorkerMessage = serde_json::from_str(json).unwrap();
        match msg {
            WorkerMessage::Hello { resume, .. } => {
                assert_eq!(resume.unwrap().last_acked_id, "obx-123");
            }
            _ => panic!("Expected Hello"),
        }
    }

    #[test]
    fn test_worker_inbound_roundtrip() {
        let msg = WorkerMessage::Inbound {
            text: "Hello from Slack".to_string(),
            thread_id: Some("thread-456".to_string()),
            sender: Some(InboundSender {
                id: "U123".to_string(),
                name: Some("Alice".to_string()),
            }),
            agent_id: None,
            metadata: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WorkerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            WorkerMessage::Inbound {
                text,
                thread_id,
                sender,
                ..
            } => {
                assert_eq!(text, "Hello from Slack");
                assert_eq!(thread_id.unwrap(), "thread-456");
                assert_eq!(sender.unwrap().id, "U123");
            }
            _ => panic!("Expected Inbound"),
        }
    }

    #[test]
    fn test_worker_ack_roundtrip() {
        let json = r#"{"type":"ack","outbox_id":"obx-abc"}"#;
        let msg: WorkerMessage = serde_json::from_str(json).unwrap();
        match msg {
            WorkerMessage::Ack { outbox_id } => assert_eq!(outbox_id, "obx-abc"),
            _ => panic!("Expected Ack"),
        }
    }

    #[test]
    fn test_worker_fail_roundtrip() {
        let msg = WorkerMessage::Fail {
            outbox_id: "obx-abc".to_string(),
            error: "Timeout".to_string(),
            retry_after_seconds: Some(30),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WorkerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            WorkerMessage::Fail {
                outbox_id,
                error,
                retry_after_seconds,
            } => {
                assert_eq!(outbox_id, "obx-abc");
                assert_eq!(error, "Timeout");
                assert_eq!(retry_after_seconds, Some(30));
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_worker_resource_response_roundtrip() {
        let json = r#"{"type":"resource_response","request_id":"req-1","data":{"contacts":[]}}"#;
        let msg: WorkerMessage = serde_json::from_str(json).unwrap();
        match msg {
            WorkerMessage::ResourceResponse { request_id, data } => {
                assert_eq!(request_id, "req-1");
                assert!(data.is_object());
            }
            _ => panic!("Expected ResourceResponse"),
        }
    }

    #[test]
    fn test_worker_capability_response_roundtrip() {
        let json = r#"{"type":"capability_response","request_id":"req-2","result":{"ok":true}}"#;
        let msg: WorkerMessage = serde_json::from_str(json).unwrap();
        match msg {
            WorkerMessage::CapabilityResponse { request_id, result } => {
                assert_eq!(request_id, "req-2");
                assert_eq!(result["ok"], true);
            }
            _ => panic!("Expected CapabilityResponse"),
        }
    }

    #[test]
    fn test_worker_pong_roundtrip() {
        let json = r#"{"type":"pong"}"#;
        let msg: WorkerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, WorkerMessage::Pong));
    }

    #[test]
    fn test_server_hello_ack_roundtrip() {
        let msg = BridgeServerMessage::HelloAck {
            server_time: "2024-01-01T00:00:00Z".to_string(),
            heartbeat_seconds: 20,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: BridgeServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            BridgeServerMessage::HelloAck {
                heartbeat_seconds, ..
            } => {
                assert_eq!(heartbeat_seconds, 20);
            }
            _ => panic!("Expected HelloAck"),
        }
    }

    #[test]
    fn test_server_outbox_item_roundtrip() {
        let msg = BridgeServerMessage::OutboxItem {
            outbox_id: "obx-123".to_string(),
            capability: "send_message".to_string(),
            payload: serde_json::json!({"text": "Hello"}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: BridgeServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            BridgeServerMessage::OutboxItem {
                outbox_id,
                capability,
                payload,
            } => {
                assert_eq!(outbox_id, "obx-123");
                assert_eq!(capability, "send_message");
                assert_eq!(payload["text"], "Hello");
            }
            _ => panic!("Expected OutboxItem"),
        }
    }

    #[test]
    fn test_server_resource_query_roundtrip() {
        let msg = BridgeServerMessage::ResourceQuery {
            request_id: "req-1".to_string(),
            resource: "contacts".to_string(),
            params: Some(serde_json::json!({"limit": 10})),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: BridgeServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            BridgeServerMessage::ResourceQuery {
                request_id,
                resource,
                params,
            } => {
                assert_eq!(request_id, "req-1");
                assert_eq!(resource, "contacts");
                assert_eq!(params.unwrap()["limit"], 10);
            }
            _ => panic!("Expected ResourceQuery"),
        }
    }

    #[test]
    fn test_server_capability_invoke_roundtrip() {
        let msg = BridgeServerMessage::CapabilityInvoke {
            request_id: "req-2".to_string(),
            capability: "send_message".to_string(),
            payload: serde_json::json!({"channel": "#general", "text": "Hi"}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: BridgeServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            BridgeServerMessage::CapabilityInvoke {
                request_id,
                capability,
                ..
            } => {
                assert_eq!(request_id, "req-2");
                assert_eq!(capability, "send_message");
            }
            _ => panic!("Expected CapabilityInvoke"),
        }
    }

    #[test]
    fn test_server_ping_roundtrip() {
        let json = r#"{"type":"ping"}"#;
        let msg: BridgeServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, BridgeServerMessage::Ping));
    }

    #[test]
    fn test_server_error_roundtrip() {
        let msg = BridgeServerMessage::Error {
            message: "Unknown adapter".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: BridgeServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            BridgeServerMessage::Error { message } => assert_eq!(message, "Unknown adapter"),
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_worker_capability_schema() {
        let cap = WorkerCapability {
            name: "send_message".to_string(),
            description: Some("Send a message to a channel".to_string()),
            schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": { "type": "string" },
                    "text": { "type": "string" }
                }
            })),
        };
        let json = serde_json::to_string(&cap).unwrap();
        let parsed: WorkerCapability = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "send_message");
        assert!(parsed.schema.is_some());
    }
}
