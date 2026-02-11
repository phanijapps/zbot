//! # Connector Configuration
//!
//! Types for connector configuration and transport definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transport type for connector communication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConnectorTransport {
    /// HTTP/HTTPS callback.
    Http {
        callback_url: String,
        #[serde(default = "default_http_method")]
        method: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    /// gRPC endpoint.
    Grpc {
        endpoint: String,
        service: String,
        method: String,
    },
    /// WebSocket connection.
    WebSocket { url: String },
    /// Unix/Windows IPC socket.
    Ipc { socket_path: String },
    /// CLI command execution.
    Cli {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
}

fn default_http_method() -> String {
    "POST".to_string()
}

/// Capability schema for a connector action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorCapability {
    /// Capability name (e.g., "send_email", "send_message").
    pub name: String,
    /// JSON schema for the capability payload.
    #[serde(default)]
    pub schema: serde_json::Value,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

/// MCP-like queryable resource definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorResource {
    /// Resource name (e.g., "contacts", "tickets", "users").
    pub name: String,
    /// URI or URI template (e.g., `https://api.example.com/contacts/{id}`).
    pub uri: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// HTTP method (default: GET).
    #[serde(default = "default_get_method")]
    pub method: String,
    /// Custom headers for this resource.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// JSON Schema for the response.
    #[serde(default)]
    pub response_schema: Option<serde_json::Value>,
}

fn default_get_method() -> String {
    "GET".to_string()
}

/// Named outbound payload schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSchema {
    /// Schema name (e.g., "send_message", "create_ticket").
    pub name: String,
    /// JSON Schema object.
    pub schema: serde_json::Value,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

/// Connector metadata containing capabilities and additional info.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectorMetadata {
    /// Available capabilities/actions.
    #[serde(default)]
    pub capabilities: Vec<ConnectorCapability>,
    /// Queryable resources (MCP-like).
    #[serde(default)]
    pub resources: Vec<ConnectorResource>,
    /// Named outbound payload schemas.
    #[serde(default)]
    pub response_schemas: Vec<ResponseSchema>,
    /// Free-form context text for agent prompts.
    #[serde(default)]
    pub context: Option<String>,
    /// Additional connector-specific data (contacts, settings, etc.).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Configuration for an external connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    /// Unique identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Transport configuration.
    pub transport: ConnectorTransport,
    /// Connector metadata (capabilities, contacts, etc.).
    #[serde(default)]
    pub metadata: ConnectorMetadata,
    /// Whether the connector is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether outbound dispatch is enabled for this connector.
    #[serde(default = "default_enabled")]
    pub outbound_enabled: bool,
    /// Whether inbound messages are accepted from this connector.
    #[serde(default = "default_enabled")]
    pub inbound_enabled: bool,
    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_enabled() -> bool {
    true
}

/// Request to create a new connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConnectorRequest {
    pub id: String,
    pub name: String,
    pub transport: ConnectorTransport,
    #[serde(default)]
    pub metadata: ConnectorMetadata,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_enabled")]
    pub outbound_enabled: bool,
    #[serde(default = "default_enabled")]
    pub inbound_enabled: bool,
}

/// Request to update a connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConnectorRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub transport: Option<ConnectorTransport>,
    #[serde(default)]
    pub metadata: Option<ConnectorMetadata>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub outbound_enabled: Option<bool>,
    #[serde(default)]
    pub inbound_enabled: Option<bool>,
}

/// Dispatch context for sending responses to connectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchContext {
    /// Session ID.
    pub session_id: String,
    /// Optional thread ID for conversation threading.
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Agent ID that generated the response.
    pub agent_id: String,
    /// Timestamp of the response.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Payload sent to connectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorPayload {
    /// Dispatch context.
    pub context: DispatchContext,
    /// Capability being invoked (e.g., "send_message").
    pub capability: String,
    /// Payload data.
    pub payload: serde_json::Value,
}

/// Stored connector data for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectorsStore {
    pub connectors: Vec<ConnectorConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_transport_serialization() {
        let transport = ConnectorTransport::Http {
            callback_url: "http://localhost:9001/callback".to_string(),
            method: "POST".to_string(),
            headers: HashMap::from([("Authorization".to_string(), "Bearer xxx".to_string())]),
            timeout_ms: Some(5000),
        };

        let json = serde_json::to_string(&transport).unwrap();
        let parsed: ConnectorTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(transport, parsed);
    }

    #[test]
    fn test_cli_transport_serialization() {
        let transport = ConnectorTransport::Cli {
            command: "/usr/bin/notify-send".to_string(),
            args: vec!["--urgency=normal".to_string()],
            env: HashMap::new(),
        };

        let json = serde_json::to_string(&transport).unwrap();
        let parsed: ConnectorTransport = serde_json::from_str(&json).unwrap();
        assert_eq!(transport, parsed);
    }

    #[test]
    fn test_connector_config_defaults() {
        let json = r#"{
            "id": "test",
            "name": "Test Connector",
            "transport": {
                "type": "http",
                "callback_url": "http://localhost:9001"
            }
        }"#;

        let config: ConnectorConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert!(config.outbound_enabled);
        assert!(config.inbound_enabled);
    }

    #[test]
    fn test_connector_resource_serialization() {
        let resource = ConnectorResource {
            name: "contacts".to_string(),
            uri: "https://api.example.com/contacts/{id}".to_string(),
            description: Some("Contact records".to_string()),
            method: "GET".to_string(),
            headers: HashMap::from([("Authorization".to_string(), "Bearer xxx".to_string())]),
            response_schema: Some(serde_json::json!({"type": "object"})),
        };

        let json = serde_json::to_string(&resource).unwrap();
        let parsed: ConnectorResource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "contacts");
        assert_eq!(parsed.uri, "https://api.example.com/contacts/{id}");
        assert_eq!(parsed.method, "GET");
        assert!(parsed.description.is_some());
        assert!(parsed.response_schema.is_some());
    }

    #[test]
    fn test_connector_resource_defaults() {
        let json = r#"{"name": "users", "uri": "https://api.example.com/users"}"#;
        let resource: ConnectorResource = serde_json::from_str(json).unwrap();
        assert_eq!(resource.method, "GET");
        assert!(resource.headers.is_empty());
        assert!(resource.description.is_none());
        assert!(resource.response_schema.is_none());
    }

    #[test]
    fn test_response_schema_serialization() {
        let schema = ResponseSchema {
            name: "send_message".to_string(),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"}
                }
            }),
            description: Some("Send a message".to_string()),
        };

        let json = serde_json::to_string(&schema).unwrap();
        let parsed: ResponseSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "send_message");
        assert!(parsed.description.is_some());
    }

    #[test]
    fn test_metadata_backward_compat() {
        // Minimal JSON with no new fields — should deserialize with defaults
        let json = r#"{"capabilities": [{"name": "send_email"}]}"#;
        let metadata: ConnectorMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.capabilities.len(), 1);
        assert!(metadata.resources.is_empty());
        assert!(metadata.response_schemas.is_empty());
        assert!(metadata.context.is_none());
    }

    #[test]
    fn test_metadata_full_roundtrip() {
        let metadata = ConnectorMetadata {
            capabilities: vec![ConnectorCapability {
                name: "send_email".to_string(),
                schema: serde_json::json!({}),
                description: None,
            }],
            resources: vec![ConnectorResource {
                name: "contacts".to_string(),
                uri: "https://api.example.com/contacts".to_string(),
                description: None,
                method: "GET".to_string(),
                headers: HashMap::new(),
                response_schema: None,
            }],
            response_schemas: vec![ResponseSchema {
                name: "send_message".to_string(),
                schema: serde_json::json!({"type": "object"}),
                description: Some("Send a message".to_string()),
            }],
            context: Some("This connector bridges Gmail.".to_string()),
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let parsed: ConnectorMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.capabilities.len(), 1);
        assert_eq!(parsed.resources.len(), 1);
        assert_eq!(parsed.response_schemas.len(), 1);
        assert_eq!(parsed.context.as_deref(), Some("This connector bridges Gmail."));
    }
}
