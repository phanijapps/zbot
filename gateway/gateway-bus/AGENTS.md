# gateway-bus

Unified intake interface trait for all session triggers (Web, CLI, Cron, API, plugins). Abstracts session creation and lifecycle management.

## Build & Test

```bash
cargo test -p gateway-bus    # 21 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `GatewayBus` trait | Unified intake interface |
| `SessionRequest` | Session submission request (agent, message, metadata) |
| `SessionHandle` | Returned session/execution IDs |
| `BusError` | Error type |

## GatewayBus Trait

```rust
#[async_trait]
pub trait GatewayBus: Send + Sync {
    async fn submit(&self, request: SessionRequest) -> Result<SessionHandle, BusError>;
    async fn status(&self, session_id: &str) -> Result<SessionStatus, BusError>;
    async fn cancel(&self, session_id: &str) -> Result<(), BusError>;
    async fn pause(&self, session_id: &str) -> Result<(), BusError>;
    async fn resume(&self, session_id: &str) -> Result<(), BusError>;
}
```

Gateway's `HttpGatewayBus` implements this trait, composing `ExecutionRunner` with session management.

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | GatewayBus trait |
| `types.rs` | SessionRequest, SessionHandle, BusError (21 tests) |
