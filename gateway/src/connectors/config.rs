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

/// Connector metadata containing capabilities and additional info.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectorMetadata {
    /// Available capabilities/actions.
    #[serde(default)]
    pub capabilities: Vec<ConnectorCapability>,
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
    }
}
