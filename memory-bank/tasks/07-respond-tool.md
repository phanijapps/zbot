# Task 07: Respond Tool Integration

## Context

The `respond` tool is what agents use to send responses. It's the single point where all responses flow through, regardless of whether the request came from CLI, Web, Cron, or external hook.

### Key Principle
**Agents don't know HOW to respond, only WHAT to respond.** The respond tool uses the HookContext to automatically route to the correct destination.

---

## Specifications (BDD)

### Feature: Respond Tool

```gherkin
Feature: Respond Tool
  As an agent
  I want to use the respond tool to send responses
  So that I don't need to know the underlying channel

  Scenario: Basic response
    Given agent is invoked with a HookContext
    When agent calls respond tool with:
      | message | Hello, how can I help you? |
    Then the response is routed via HookRouter
    And respond tool returns:
      """
      {
        "status": "sent",
        "hook_type": "web"
      }
      """

  Scenario: Response with formatting
    Given agent is invoked via Web hook
    When agent calls respond tool with:
      | message | Here's the **result** |
      | format  | markdown              |
    Then response is sent with format metadata

  Scenario: Response fails if no HookContext
    Given agent is invoked WITHOUT HookContext
    When agent calls respond tool
    Then respond tool returns error:
      | error | No hook context available |

  Scenario: Agent can respond multiple times
    Given agent is processing a complex request
    When agent calls respond tool with "Processing step 1..."
    And agent calls respond tool with "Processing step 2..."
    And agent calls respond tool with "Done!"
    Then all three responses are delivered in order
```

### Feature: Respond Tool Schema

```gherkin
Feature: Respond Tool Schema
  As an LLM
  I need a clear tool schema
  So that I know how to call the respond tool

  Scenario: Tool schema for LLM
    Given I query the available tools
    Then respond tool has schema:
      """
      {
        "name": "respond",
        "description": "Send a response to the user. Use this to reply to the user's message.",
        "parameters": {
          "type": "object",
          "properties": {
            "message": {
              "type": "string",
              "description": "The response message to send to the user"
            },
            "format": {
              "type": "string",
              "enum": ["text", "markdown", "html"],
              "description": "Format of the message (default: text)"
            }
          },
          "required": ["message"]
        }
      }
      """
```

---

## Implementation

### File: `application/agent-runtime/src/tools/respond.rs`

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::tools::{Tool, ToolContext, ToolError, ToolResult};

/// Tool for sending responses back to the user.
///
/// This tool reads the HookContext from the execution context
/// and routes the response to the appropriate destination
/// (WebSocket, callback URL, stdout, etc.)
pub struct RespondTool {
    // HookRouter is accessed via context, not stored here
}

impl RespondTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for RespondTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RespondTool {
    fn name(&self) -> &str {
        "respond"
    }

    fn description(&self) -> &str {
        "Send a response to the user. Use this tool to reply to the user's message. \
         The response will be delivered through the same channel the user used to contact you \
         (web chat, WhatsApp, Telegram, etc.)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The response message to send to the user"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown", "html"],
                    "description": "Format of the message. Default is 'text'.",
                    "default": "text"
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, context: &ToolContext, args: Value) -> ToolResult {
        // Extract message from arguments
        let message = args.get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgument("message is required".into()))?;

        let format = args.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        // Get HookContext from execution context
        let hook_context = context.get_state::<crate::hooks::HookContext>("hook_context")
            .ok_or_else(|| ToolError::ExecutionError(
                "No hook context available. Cannot determine where to send response.".into()
            ))?;

        // Get HookRouter from execution context
        let hook_router = context.get_state::<Arc<crate::hooks::HookRouter>>("hook_router")
            .ok_or_else(|| ToolError::ExecutionError(
                "No hook router available.".into()
            ))?;

        // Route the response
        hook_router.respond(&hook_context, message).await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to send response: {}", e)))?;

        // Return success
        Ok(json!({
            "status": "sent",
            "hook_type": hook_context.hook_type.type_name(),
            "format": format
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_respond_tool_schema() {
        let tool = RespondTool::new();
        assert_eq!(tool.name(), "respond");

        let schema = tool.parameters_schema();
        assert!(schema["properties"]["message"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("message")));
    }
}
```

### File: `application/agent-runtime/src/tools/mod.rs` (update)

Add to tool registry:

```rust
mod respond;

pub use respond::RespondTool;

impl ToolRegistry {
    pub fn new_with_defaults() -> Self {
        let mut registry = Self::new();

        // Built-in tools
        registry.register(Arc::new(RespondTool::new()));
        // ... other tools ...

        registry
    }
}
```

### File: `application/gateway/src/execution/runner.rs` (update)

Inject HookRouter into execution context:

```rust
impl ExecutionRunner {
    pub async fn invoke_with_hook(
        &self,
        agent_id: &str,
        conversation_id: &str,
        message: &str,
        hook_context: HookContext,
    ) -> Result<(), String> {
        // ... existing setup ...

        // Create execution context with hook infrastructure
        let mut context = CallbackContext::new();
        context.set_state("hook_context", hook_context.clone());
        context.set_state("hook_router", self.hook_router.clone());
        context.set_state("agent_id", agent_id.to_string());
        context.set_state("conversation_id", conversation_id.to_string());

        // Execute agent with context
        executor.execute_stream_with_context(messages, callback, context).await
    }
}
```

---

## Verification

### Unit Tests

```rust
#[tokio::test]
async fn test_respond_tool_requires_message() {
    let tool = RespondTool::new();
    let context = create_test_context();

    let result = tool.execute(&context, json!({})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("message is required"));
}

#[tokio::test]
async fn test_respond_tool_requires_hook_context() {
    let tool = RespondTool::new();
    let context = ToolContext::new();  // No hook_context

    let result = tool.execute(&context, json!({"message": "hi"})).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No hook context"));
}

#[tokio::test]
async fn test_respond_tool_success() {
    let tool = RespondTool::new();

    let (event_bus, _) = create_test_event_bus();
    let hook_router = create_test_router(event_bus);

    let hook_context = HookContext::builtin(
        BuiltinHookType::Web { session_id: "sess".into() },
        "sess"
    ).with_conversation("conv");

    let mut context = ToolContext::new();
    context.set_state("hook_context", hook_context);
    context.set_state("hook_router", Arc::new(hook_router));

    let result = tool.execute(&context, json!({
        "message": "Hello user!"
    })).await;

    assert!(result.is_ok());
    let value = result.unwrap();
    assert_eq!(value["status"], "sent");
    assert_eq!(value["hook_type"], "web");
}
```

### Integration Test

```bash
# Send message via WebSocket
websocat ws://localhost:18790

# Send invoke:
{"type":"invoke","agent_id":"root","conversation_id":"test","message":"Hi, say hello back using the respond tool"}

# Agent should use respond tool, you should see response
```

---

## Dependencies

- Task 01-06 complete
- ToolRegistry from agent-runtime

## Outputs

- `application/agent-runtime/src/tools/respond.rs`
- Modified: `tools/mod.rs`, `execution/runner.rs`

## Next Task

Task 08: Agent Delegation Framework
