# Task 03: Web Hook Implementation

## Objective
Implement the `WebHook` that routes responses back through WebSocket connections, refactoring the existing WebSocket response mechanism.

## Background
The current system streams responses via WebSocket events. This task wraps that behavior in a `Hook` implementation so it works with the unified hook system.

## Current State
- `HookRegistry` and `Hook` trait exist from Task 02
- WebSocket responses handled in `application/gateway/src/websocket/`
- Events published via `EventBus` in `application/gateway/src/events/`

## Key Files to Understand
- `application/gateway/src/websocket/session.rs` - WebSocket session handling
- `application/gateway/src/events/bus.rs` - Event broadcasting
- `application/gateway/src/events/types.rs` - GatewayEvent enum

## Deliverables

### 1. Create `application/gateway/src/hooks/web.rs`
```rust
use crate::events::{EventBus, GatewayEvent};
use crate::hooks::{Attachment, Hook, HookContext, HookType};
use async_trait::async_trait;
use std::sync::Arc;

/// Hook that routes responses through WebSocket connections
pub struct WebHook {
    event_bus: Arc<EventBus>,
}

impl WebHook {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl Hook for WebHook {
    fn hook_type_name(&self) -> &'static str {
        "web"
    }

    fn can_handle(&self, ctx: &HookContext) -> bool {
        matches!(ctx.hook_type, HookType::Web { .. })
    }

    async fn respond(
        &self,
        ctx: &HookContext,
        message: &str,
        _attachments: Option<Vec<Attachment>>,
    ) -> Result<(), String> {
        // Extract conversation_id from metadata or source_id
        let conversation_id = ctx
            .metadata
            .get("conversation_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.source_id);

        // Extract agent_id from metadata
        let agent_id = ctx
            .metadata
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("root");

        // Publish a respond event that WebSocket will forward
        let event = GatewayEvent::Respond {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            message: message.to_string(),
        };

        self.event_bus.publish(event).await;
        Ok(())
    }
}
```

### 2. Add Respond event to GatewayEvent
**File**: `application/gateway/src/events/types.rs`

Add to the `GatewayEvent` enum:
```rust
/// Response to be sent via hook
Respond {
    agent_id: String,
    conversation_id: String,
    message: String,
},
```

### 3. Handle Respond in WebSocket session
**File**: `application/gateway/src/websocket/session.rs`

In the event handler loop, add case for `Respond`:
```rust
GatewayEvent::Respond { conversation_id, message, .. } => {
    // Send as a special message type to the client
    let msg = ServerMessage::Response {
        conversation_id,
        message,
    };
    // Send to WebSocket...
}
```

### 4. Add ServerMessage::Response variant
**File**: `application/gateway/src/websocket/messages.rs`

```rust
/// Response routed through hook system
Response {
    conversation_id: String,
    message: String,
},
```

### 5. Update `application/gateway/src/hooks/mod.rs`
```rust
mod context;
mod registry;
mod web;

pub use context::{HookContext, HookType};
pub use registry::{Attachment, Hook, HookRegistry};
pub use web::WebHook;
```

## Verification
1. Build: `cargo build -p agentzero-gateway`
2. Integration test:
   - Start gateway
   - Connect WebSocket
   - Manually publish a `GatewayEvent::Respond`
   - Verify WebSocket receives the message

## Dependencies
- Task 01, 02 complete
- Access to `EventBus`

## Next Task
Task 04: Hook Context Injection in Execution
