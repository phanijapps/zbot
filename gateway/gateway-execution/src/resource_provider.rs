//! # Gateway Resource Provider
//!
//! Implementation of `ConnectorResourceProvider` backed by the gateway's
//! `ConnectorRegistry`.

use async_trait::async_trait;
use gateway_connectors::ConnectorRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use zero_core::connectors::{
    CapabilityInfo, ConnectorInfo, ConnectorResourceProvider, ResourceInfo,
};

/// Gateway implementation of `ConnectorResourceProvider`.
///
/// Wraps the `ConnectorRegistry` to provide resource querying capabilities
/// to agent tools.
pub struct GatewayResourceProvider {
    registry: Arc<ConnectorRegistry>,
}

impl GatewayResourceProvider {
    /// Create a new gateway resource provider.
    pub fn new(registry: Arc<ConnectorRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ConnectorResourceProvider for GatewayResourceProvider {
    async fn list_connectors(&self) -> Result<Vec<ConnectorInfo>, String> {
        let connectors = self
            .registry
            .list()
            .await
            .map_err(|e| format!("Failed to list connectors: {}", e))?;

        Ok(connectors
            .into_iter()
            .filter(|c| c.enabled)
            .map(|c| ConnectorInfo {
                id: c.id,
                name: c.name,
                resources: c
                    .metadata
                    .resources
                    .into_iter()
                    .map(|r| ResourceInfo {
                        name: r.name,
                        uri: r.uri,
                        method: r.method,
                        description: r.description,
                    })
                    .collect(),
                capabilities: c
                    .metadata
                    .capabilities
                    .into_iter()
                    .map(|cap| CapabilityInfo {
                        name: cap.name,
                        schema: cap.schema,
                        description: cap.description,
                    })
                    .collect(),
            })
            .collect())
    }

    async fn query_resource(
        &self,
        connector_id: &str,
        resource_name: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<serde_json::Value, String> {
        let connector = self
            .registry
            .get(connector_id)
            .await
            .map_err(|e| format!("Connector '{}' not found: {}", connector_id, e))?;

        // Find the resource by name
        let resource = connector
            .metadata
            .resources
            .iter()
            .find(|r| r.name == resource_name)
            .ok_or_else(|| {
                let available: Vec<&str> = connector
                    .metadata
                    .resources
                    .iter()
                    .map(|r| r.name.as_str())
                    .collect();
                format!(
                    "Resource '{}' not found on connector '{}'. Available: {:?}",
                    resource_name, connector_id, available
                )
            })?;

        // Expand URI template with params
        let mut uri = resource.uri.clone();
        if let Some(params) = &params {
            for (key, value) in params {
                uri = uri.replace(&format!("{{{}}}", key), value);
            }
        }

        // Build HTTP request with resource headers
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let mut request = match resource.method.to_uppercase().as_str() {
            "POST" => client.post(&uri),
            "PUT" => client.put(&uri),
            "DELETE" => client.delete(&uri),
            _ => client.get(&uri),
        };

        // Apply resource-specific headers
        for (key, value) in &resource.headers {
            request = request.header(key, value);
        }

        // Also apply connector transport headers (for auth etc.)
        if let gateway_connectors::ConnectorTransport::Http { headers, .. } = &connector.transport {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Resource request failed: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if !status.is_success() {
            return Err(format!(
                "Resource returned HTTP {}: {}",
                status.as_u16(),
                body.chars().take(500).collect::<String>()
            ));
        }

        // Try to parse as JSON, fall back to string value
        match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(json) => Ok(json),
            Err(_) => Ok(serde_json::Value::String(body)),
        }
    }

    async fn invoke_capability(
        &self,
        connector_id: &str,
        capability: &str,
        payload: serde_json::Value,
        session_id: &str,
        agent_id: &str,
    ) -> Result<serde_json::Value, String> {
        // Validate the capability exists on this connector
        let connector = self
            .registry
            .get(connector_id)
            .await
            .map_err(|e| format!("Connector '{}' not found: {}", connector_id, e))?;

        let cap_exists = connector
            .metadata
            .capabilities
            .iter()
            .any(|c| c.name == capability);

        if !cap_exists {
            let available: Vec<&str> = connector
                .metadata
                .capabilities
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            return Err(format!(
                "Capability '{}' not found on connector '{}'. Available: {:?}",
                capability, connector_id, available
            ));
        }

        let context = gateway_connectors::DispatchContext {
            session_id: session_id.to_string(),
            thread_id: None,
            agent_id: agent_id.to_string(),
            timestamp: chrono::Utc::now(),
        };

        let result = self
            .registry
            .dispatch_to_one(connector_id, capability, payload, &context)
            .await
            .map_err(|e| format!("Dispatch failed: {}", e))?;

        Ok(serde_json::json!({
            "success": result.success,
            "status": result.status,
            "body": result.body,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_template_expansion() {
        let mut uri = "https://api.example.com/contacts/{id}".to_string();
        let params = HashMap::from([("id".to_string(), "user-123".to_string())]);

        for (key, value) in &params {
            uri = uri.replace(&format!("{{{}}}", key), value);
        }

        assert_eq!(uri, "https://api.example.com/contacts/user-123");
    }

    #[test]
    fn test_uri_template_multiple_params() {
        let mut uri = "https://api.example.com/{org}/repos/{repo}".to_string();
        let params = HashMap::from([
            ("org".to_string(), "acme".to_string()),
            ("repo".to_string(), "project".to_string()),
        ]);

        for (key, value) in &params {
            uri = uri.replace(&format!("{{{}}}", key), value);
        }

        assert_eq!(uri, "https://api.example.com/acme/repos/project");
    }

    #[test]
    fn test_uri_template_no_params() {
        let uri = "https://api.example.com/contacts".to_string();
        // No expansion needed
        assert!(!uri.contains('{'));
    }
}
