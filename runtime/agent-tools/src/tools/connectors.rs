// ============================================================================
// QUERY RESOURCE TOOL
// Discover and query data from external connector resources
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::connectors::ConnectorResourceProvider;
use zero_core::{Result, Tool, ToolContext, ToolPermissions, ZeroError};

/// Tool for querying resources and invoking capabilities on external connectors.
///
/// Provides three actions:
/// - `list_resources`: Discover available connectors, resources (GET), and capabilities (POST)
/// - `query`: Fetch data from a connector resource URI
/// - `invoke`: Invoke a capability on a connector (e.g., send_message)
pub struct QueryResourceTool {
    provider: Arc<dyn ConnectorResourceProvider>,
}

impl QueryResourceTool {
    /// Create a new QueryResourceTool with the given provider.
    pub fn new(provider: Arc<dyn ConnectorResourceProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl Tool for QueryResourceTool {
    fn name(&self) -> &str {
        "query_resource"
    }

    fn description(&self) -> &str {
        "Discover resources and capabilities on external connectors. \
        Actions: 'list_resources' (discover connectors, resources, and capabilities), \
        'query' (fetch data from a resource via GET), \
        'invoke' (call a capability like send_message via POST). \
        Use list_resources first to discover what's available."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list_resources", "query", "invoke"],
                    "description": "The operation to perform"
                },
                "connector_id": {
                    "type": "string",
                    "description": "Connector ID (required for 'query' and 'invoke')"
                },
                "resource": {
                    "type": "string",
                    "description": "Resource name to query (required for 'query')"
                },
                "capability": {
                    "type": "string",
                    "description": "Capability name to invoke, e.g. 'send_message' (required for 'invoke')"
                },
                "params": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Parameters for URI template expansion (for 'query')"
                },
                "payload": {
                    "type": "object",
                    "description": "Payload to send when invoking a capability (for 'invoke')"
                }
            },
            "required": ["action"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::moderate(vec!["network:http".into()])
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        match action {
            "list_resources" => {
                let connectors = self
                    .provider
                    .list_connectors()
                    .await
                    .map_err(|e| ZeroError::Tool(e))?;

                if connectors.is_empty() {
                    return Ok(json!({
                        "message": "No connectors configured. Add connectors via the web UI or API.",
                        "connectors": []
                    }));
                }

                // Format for agent consumption
                let summary: Vec<Value> = connectors
                    .iter()
                    .map(|c| {
                        let resources: Vec<Value> = c
                            .resources
                            .iter()
                            .map(|r| {
                                json!({
                                    "name": r.name,
                                    "type": "resource",
                                    "method": r.method,
                                    "description": r.description,
                                })
                            })
                            .collect();

                        let capabilities: Vec<Value> = c
                            .capabilities
                            .iter()
                            .map(|cap| {
                                json!({
                                    "name": cap.name,
                                    "type": "capability",
                                    "method": "POST",
                                    "description": cap.description,
                                    "schema": cap.schema,
                                })
                            })
                            .collect();

                        json!({
                            "connector_id": c.id,
                            "name": c.name,
                            "resources": resources,
                            "capabilities": capabilities,
                        })
                    })
                    .collect();

                Ok(json!({
                    "connectors": summary,
                    "usage": "query_resource(action='query', ...) to fetch data from resources. query_resource(action='invoke', connector_id='...', capability='...', payload={...}) to call a capability."
                }))
            }

            "query" => {
                let connector_id = args
                    .get("connector_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ZeroError::Tool(
                            "Missing 'connector_id' parameter for query action".to_string(),
                        )
                    })?;

                let resource = args
                    .get("resource")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ZeroError::Tool(
                            "Missing 'resource' parameter for query action".to_string(),
                        )
                    })?;

                // Parse params from JSON object to HashMap<String, String>
                let params: Option<HashMap<String, String>> =
                    args.get("params").and_then(|v| {
                        v.as_object().map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                .collect()
                        })
                    });

                let result = self
                    .provider
                    .query_resource(connector_id, resource, params)
                    .await
                    .map_err(|e| ZeroError::Tool(e))?;

                Ok(result)
            }

            "invoke" => {
                let connector_id = args
                    .get("connector_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ZeroError::Tool(
                            "Missing 'connector_id' parameter for invoke action".to_string(),
                        )
                    })?;

                let capability = args
                    .get("capability")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ZeroError::Tool(
                            "Missing 'capability' parameter for invoke action".to_string(),
                        )
                    })?;

                let payload = args
                    .get("payload")
                    .cloned()
                    .unwrap_or(json!({}));

                let session_id = ctx.session_id().to_string();
                let agent_id = ctx.agent_name().to_string();

                let result = self
                    .provider
                    .invoke_capability(
                        connector_id,
                        capability,
                        payload,
                        &session_id,
                        &agent_id,
                    )
                    .await
                    .map_err(|e| ZeroError::Tool(e))?;

                Ok(result)
            }

            _ => Err(ZeroError::Tool(format!(
                "Unknown action '{}'. Valid: list_resources, query, invoke",
                action
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::connectors::{CapabilityInfo, ConnectorInfo, ResourceInfo};
    use zero_core::event::EventActions;
    use zero_core::types::Content;
    use zero_core::context::{ReadonlyContext, CallbackContext};

    /// Mock provider for testing.
    struct MockProvider {
        connectors: Vec<ConnectorInfo>,
    }

    #[async_trait]
    impl ConnectorResourceProvider for MockProvider {
        async fn list_connectors(&self) -> std::result::Result<Vec<ConnectorInfo>, String> {
            Ok(self.connectors.clone())
        }

        async fn query_resource(
            &self,
            connector_id: &str,
            resource_name: &str,
            _params: Option<HashMap<String, String>>,
        ) -> std::result::Result<serde_json::Value, String> {
            if connector_id == "signal" && resource_name == "aliases" {
                Ok(json!([
                    {"alias": "dev-team", "number": "+1234567890"},
                    {"alias": "ops", "number": "+0987654321"}
                ]))
            } else {
                Err(format!(
                    "Resource '{}' not found on '{}'",
                    resource_name, connector_id
                ))
            }
        }

        async fn invoke_capability(
            &self,
            connector_id: &str,
            capability: &str,
            _payload: serde_json::Value,
            _session_id: &str,
            _agent_id: &str,
        ) -> std::result::Result<serde_json::Value, String> {
            if connector_id == "signal" && capability == "send_message" {
                Ok(json!({"success": true, "status": 200, "body": "sent"}))
            } else {
                Err(format!(
                    "Capability '{}' not found on '{}'",
                    capability, connector_id
                ))
            }
        }
    }

    fn mock_provider() -> Arc<dyn ConnectorResourceProvider> {
        Arc::new(MockProvider {
            connectors: vec![ConnectorInfo {
                id: "signal".to_string(),
                name: "Signal Bridge".to_string(),
                resources: vec![
                    ResourceInfo {
                        name: "aliases".to_string(),
                        uri: "http://localhost:9001/aliases".to_string(),
                        method: "GET".to_string(),
                        description: Some("List signal aliases".to_string()),
                    },
                    ResourceInfo {
                        name: "messages".to_string(),
                        uri: "http://localhost:9001/messages/{thread_id}".to_string(),
                        method: "GET".to_string(),
                        description: Some("Get messages for a thread".to_string()),
                    },
                ],
                capabilities: vec![
                    CapabilityInfo {
                        name: "send_message".to_string(),
                        schema: json!({"type": "object", "properties": {"text": {"type": "string"}, "recipient": {"type": "string"}}}),
                        description: Some("Send a message via Signal".to_string()),
                    },
                ],
            }],
        })
    }

    struct MockToolContext;

    impl ReadonlyContext for MockToolContext {
        fn invocation_id(&self) -> &str { "test" }
        fn agent_name(&self) -> &str { "test-agent" }
        fn user_id(&self) -> &str { "test" }
        fn app_name(&self) -> &str { "test" }
        fn session_id(&self) -> &str { "test" }
        fn branch(&self) -> &str { "test" }
        fn user_content(&self) -> &Content {
            use std::sync::LazyLock;
            static CONTENT: LazyLock<Content> = LazyLock::new(|| Content {
                role: "user".to_string(),
                parts: vec![],
            });
            &CONTENT
        }
    }

    impl CallbackContext for MockToolContext {
        fn get_state(&self, _key: &str) -> Option<Value> { None }
        fn set_state(&self, _key: String, _value: Value) {}
    }

    impl ToolContext for MockToolContext {
        fn function_call_id(&self) -> String { "test-call".to_string() }
        fn actions(&self) -> EventActions { EventActions::default() }
        fn set_actions(&self, _actions: EventActions) {}
    }

    fn mock_context() -> Arc<dyn ToolContext> {
        Arc::new(MockToolContext)
    }

    #[tokio::test]
    async fn test_list_resources() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(ctx, json!({"action": "list_resources"}))
            .await
            .unwrap();

        let connectors = result["connectors"].as_array().unwrap();
        assert_eq!(connectors.len(), 1);
        assert_eq!(connectors[0]["connector_id"], "signal");

        let resources = connectors[0]["resources"].as_array().unwrap();
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0]["name"], "aliases");
    }

    #[tokio::test]
    async fn test_query_resource() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(
                ctx,
                json!({
                    "action": "query",
                    "connector_id": "signal",
                    "resource": "aliases"
                }),
            )
            .await
            .unwrap();

        let aliases = result.as_array().unwrap();
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0]["alias"], "dev-team");
    }

    #[tokio::test]
    async fn test_query_unknown_resource() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(
                ctx,
                json!({
                    "action": "query",
                    "connector_id": "signal",
                    "resource": "nonexistent"
                }),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_action() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool.execute(ctx, json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_query_missing_connector_id() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(
                ctx,
                json!({
                    "action": "query",
                    "resource": "aliases"
                }),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_empty_connectors() {
        let provider: Arc<dyn ConnectorResourceProvider> = Arc::new(MockProvider {
            connectors: vec![],
        });
        let tool = QueryResourceTool::new(provider);
        let ctx = mock_context();

        let result = tool
            .execute(ctx, json!({"action": "list_resources"}))
            .await
            .unwrap();

        let connectors = result["connectors"].as_array().unwrap();
        assert!(connectors.is_empty());
        assert!(result["message"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_list_resources_includes_capabilities() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(ctx, json!({"action": "list_resources"}))
            .await
            .unwrap();

        let connectors = result["connectors"].as_array().unwrap();
        assert_eq!(connectors.len(), 1);

        let capabilities = connectors[0]["capabilities"].as_array().unwrap();
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0]["name"], "send_message");
        assert_eq!(capabilities[0]["type"], "capability");
        assert_eq!(capabilities[0]["method"], "POST");
    }

    #[tokio::test]
    async fn test_invoke_capability() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(
                ctx,
                json!({
                    "action": "invoke",
                    "connector_id": "signal",
                    "capability": "send_message",
                    "payload": {"text": "hello", "recipient": "+1234567890"}
                }),
            )
            .await
            .unwrap();

        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn test_invoke_unknown_capability() {
        let tool = QueryResourceTool::new(mock_provider());
        let ctx = mock_context();

        let result = tool
            .execute(
                ctx,
                json!({
                    "action": "invoke",
                    "connector_id": "signal",
                    "capability": "nonexistent"
                }),
            )
            .await;

        assert!(result.is_err());
    }
}
