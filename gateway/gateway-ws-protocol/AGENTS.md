# gateway-ws-protocol

WebSocket message type definitions for client-server communication. Pure data types with no runtime dependencies — usable by both server and client code.

## Build & Test

```bash
cargo test -p gateway-ws-protocol    # 7 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `ClientMessage` | Client → Server messages |
| `ServerMessage` | Server → Client messages (carries event payloads inline) |
| `SubscriptionScope` | Event filtering scope |

## ClientMessage Variants

```
Subscribe { conversation_id, scope }
Unsubscribe { conversation_id }
Invoke { agent_id, conversation_id, message, session_id?, mode? }
Stop { conversation_id }
Continue { conversation_id, additional_iterations? }
Pause / Resume / Cancel { session_id }
EndSession { session_id }
Ping
```

## ServerMessage Variants (partial)

```
Subscribed / Unsubscribed / SubscriptionError
AgentStarted / AgentCompleted / AgentStopped
Token / Thinking / ToolCall / ToolResult / TurnComplete
Error / Iteration / ContinuationPrompt
MessageAdded / Heartbeat / TokenUsage
DelegationStarted / DelegationCompleted
Pong / Connected
SessionPaused / SessionResumed / SessionCancelled / SessionEnded
InvokeAccepted / WardChanged / ...
```

`ServerMessage` carries event data inline (not a wrapper around `GatewayEvent`).

## SubscriptionScope

```rust
pub enum SubscriptionScope {
    All,                    // All events (default, backward compatible)
    Session,                // Root events + delegation lifecycle only
    Execution(String),      // All events for a specific execution ID
}
```

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Public exports |
| `messages.rs` | All message types + tests |
