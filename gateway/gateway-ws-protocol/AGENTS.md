# gateway-ws-protocol

WebSocket message type definitions for client-server communication. Pure data types with no runtime dependencies.

## Build & Test

```bash
cargo test -p gateway-ws-protocol    # 7 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `ClientMessage` | Client → Server messages |
| `ServerMessage` | Server → Client messages |
| `SubscriptionScope` | Event filtering (All, Session, Execution) |

## ClientMessage Variants

```
Subscribe, Unsubscribe, Invoke, Stop, Continue,
Pause, Resume, Cancel, EndSession, Ping
```

## ServerMessage Variants

```
Event, Error, Connected, Disconnected
```

## SubscriptionScope

```rust
pub enum SubscriptionScope {
    All,                    // All events
    Session,                // Current session only
    Execution(String),      // Specific execution ID
}
```

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Public exports |
| `messages.rs` | All message types (7 tests) |
