# Task 02: Built-in Web Hook

## Context

The Web dashboard uses WebSocket for real-time communication. Currently, responses are streamed directly through the WebSocket. This task refactors that to use the hook system, making it consistent with other hooks.

### Current Behavior
1. User sends message via WebSocket
2. Gateway executes agent
3. Gateway publishes events to EventBus
4. WebSocket session forwards events to client

### Target Behavior
1. User sends message via WebSocket
2. Gateway creates `HookContext` with `BuiltinHookType::Web`
3. Gateway executes agent with HookContext
4. When agent uses `respond()` tool, it routes through hook system
5. Web hook sends response via WebSocket

### Why This Matters
- Unifies response handling across all hooks
- Agent doesn't need to know HOW to respond, just WHAT to respond
- `respond()` tool works the same for Web, CLI, and external hooks

---

## Specifications (BDD)

### Feature: Web Hook Response Routing

```gherkin
Feature: Web Hook Response Routing
  As a user on the web dashboard
  I want to receive agent responses via WebSocket
  So that I can have real-time conversations

  Background:
    Given I am connected to the gateway via WebSocket
    And my session_id is "sess-abc123"

  Scenario: Agent responds via hook system
    Given I send message "Hello agent"
    And the gateway creates a HookContext with:
      | field      | value                              |
      | hook_type  | Builtin(Web { session_id: "sess-abc123" }) |
      | source_id  | "sess-abc123"                      |
    When the agent uses the respond tool with message "Hello human!"
    Then the Web hook is invoked
    And I receive a WebSocket message:
      """
      {
        "type": "respond",
        "conversation_id": "conv-xyz",
        "message": "Hello human!"
      }
      """

  Scenario: Streaming tokens still work
    Given I send message "Write a poem"
    When the agent streams tokens
    Then I receive token events via WebSocket:
      | type  | delta    |
      | token | "Roses " |
      | token | "are "   |
      | token | "red"    |
    And when agent uses respond tool with final message
    Then I receive a respond event

  Scenario: Multiple clients same conversation
    Given client A is connected with session "sess-A"
    And client B is connected with session "sess-B"
    And both are viewing conversation "conv-shared"
    When agent responds in conversation "conv-shared"
    Then both clients receive the respond event
```

### Feature: HookContext Creation from WebSocket

```gherkin
Feature: HookContext Creation from WebSocket
  As the gateway
  I need to create HookContext when receiving WebSocket messages
  So that responses can be routed back

  Scenario: Invoke message creates HookContext
    Given WebSocket session "sess-123" is connected
    When I receive message:
      """
      {
        "type": "invoke",
        "agent_id": "root",
        "conversation_id": "conv-456",
        "message": "Hello"
      }
      """
    Then I create HookContext:
      | field           | value         |
      | hook_type       | Builtin(Web)  |
      | source_id       | "sess-123"    |
      | conversation_id | "conv-456"    |
    And I pass HookContext to ExecutionRunner.invoke_with_hook()
```

---

## Implementation

### File: `application/gateway/src/hooks/builtin/mod.rs`

```rust
mod web;

pub use web::WebHook;
```

### File: `application/gateway/src/hooks/builtin/web.rs`

```rust
use crate::events::{EventBus, GatewayEvent};
use crate::hooks::{HookContext, HookType, BuiltinHookType};
use std::sync::Arc;

/// Built-in hook for Web dashboard (WebSocket responses)
pub struct WebHook {
    event_bus: Arc<EventBus>,
}

impl WebHook {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }

    /// Route a response through the Web hook
    ///
    /// Publishes a Respond event to the EventBus, which WebSocket
    /// sessions pick up and forward to connected clients.
    pub async fn respond(&self, ctx: &HookContext, message: &str) -> Result<(), String> {
        // Validate this is a Web hook context
        if !matches!(ctx.hook_type, HookType::Builtin(BuiltinHookType::Web { .. })) {
            return Err("WebHook cannot handle non-Web context".into());
        }

        let conversation_id = ctx.conversation_id.as_ref()
            .ok_or("No conversation_id in HookContext")?;

        let event = GatewayEvent::Respond {
            conversation_id: conversation_id.clone(),
            message: message.to_string(),
        };

        self.event_bus.publish(event).await;
        Ok(())
    }

    /// Check if this hook can handle the given context
    pub fn can_handle(&self, ctx: &HookContext) -> bool {
        matches!(ctx.hook_type, HookType::Builtin(BuiltinHookType::Web { .. }))
    }
}
```

### File: `application/gateway/src/events/types.rs` (modification)

Add to `GatewayEvent` enum:

```rust
/// Response from agent via hook system
Respond {
    conversation_id: String,
    message: String,
},
```

### File: `application/gateway/src/websocket/messages.rs` (modification)

Add to `ServerMessage` enum:

```rust
/// Response routed through hook system
#[serde(rename = "respond")]
Respond {
    conversation_id: String,
    message: String,
},
```

### File: `application/gateway/src/websocket/session.rs` (modification)

In the event handling loop, add case for `Respond`:

```rust
GatewayEvent::Respond { conversation_id, message } => {
    let msg = ServerMessage::Respond {
        conversation_id: conversation_id.clone(),
        message: message.clone(),
    };
    if let Err(e) = tx.send(Message::Text(serde_json::to_string(&msg).unwrap())).await {
        tracing::error!("Failed to send respond message: {}", e);
    }
}
```

### File: `application/gateway/src/websocket/handler.rs` (modification)

When handling `ClientMessage::Invoke`, create HookContext:

```rust
ClientMessage::Invoke { agent_id, conversation_id, message, .. } => {
    // Create hook context for this WebSocket session
    let hook_context = HookContext::builtin(
        BuiltinHookType::Web { session_id: session_id.clone() },
        session_id.clone(),
    ).with_conversation(&conversation_id);

    // Invoke with hook context
    runtime.invoke_with_hook(
        &agent_id,
        &conversation_id,
        &message,
        hook_context,
    ).await?;
}
```

### File: `application/gateway/src/hooks/mod.rs` (modification)

```rust
mod context;
mod types;
pub mod builtin;

pub use context::HookContext;
pub use types::{BuiltinHookType, ExternalHookConfig, HookType};
pub use builtin::WebHook;
```

---

## Verification

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_web_hook_respond() {
        let (tx, mut rx) = broadcast::channel(16);
        let event_bus = Arc::new(EventBus::new_with_sender(tx));
        let web_hook = WebHook::new(event_bus);

        let ctx = HookContext::builtin(
            BuiltinHookType::Web { session_id: "sess-1".into() },
            "sess-1"
        ).with_conversation("conv-1");

        web_hook.respond(&ctx, "Hello!").await.unwrap();

        let event = rx.recv().await.unwrap();
        assert!(matches!(
            event,
            GatewayEvent::Respond { message, .. } if message == "Hello!"
        ));
    }

    #[tokio::test]
    async fn test_web_hook_rejects_non_web_context() {
        let event_bus = Arc::new(EventBus::new());
        let web_hook = WebHook::new(event_bus);

        let ctx = HookContext::builtin(
            BuiltinHookType::Cli,
            "cli"
        );

        let result = web_hook.respond(&ctx, "Hello!").await;
        assert!(result.is_err());
    }
}
```

### Integration Test

```bash
# Terminal 1: Start gateway
cargo run -p agentzero-gateway

# Terminal 2: Connect WebSocket and send invoke
websocat ws://localhost:18790

# Send:
{"type":"invoke","agent_id":"root","conversation_id":"test-123","message":"Hello"}

# Expect to see respond event when agent completes
```

---

## Dependencies

- Task 01 complete (HookContext, HookType exist)
- Access to EventBus

## Outputs

- `application/gateway/src/hooks/builtin/mod.rs`
- `application/gateway/src/hooks/builtin/web.rs`
- Modified: `events/types.rs`, `websocket/messages.rs`, `websocket/session.rs`, `websocket/handler.rs`

## Next Task

Task 03: Built-in Cron Hook - Execute scheduled jobs with hook context
