# gateway-events

Foundation crate. Event bus for broadcasting gateway events to connected clients via tokio broadcast channels. No gateway dependencies.

## Build & Test

```bash
cargo test -p gateway-events    # 8 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `EventBus` | Pub/sub event distribution with agent/session channels |
| `GatewayEvent` | 20+ event variants (AgentStarted, Token, ToolCall, WardChanged, etc.) |
| `HookContext` | Context passed to hooks (agent_id, session_id, message, source) |
| `HookType` | Hook origin type (Cli, Web, Cron, Webhook, etc.) |

## Public API (EventBus)

| Method | Purpose |
|--------|---------|
| `new()` / `with_capacity()` | Create event bus |
| `publish()` | Async publish event |
| `publish_sync()` | Sync publish (preserves token ordering) |
| `subscribe_all()` | Subscribe to all events |
| `subscribe_agent()` | Subscribe to specific agent's events |
| `subscribe_session()` | Subscribe to specific session's events |

## GatewayEvent Accessors

Every event variant exposes: `agent_id()`, `session_id()`, `execution_id()`, `conversation_id()`

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | GatewayEvent enum, public API |
| `broadcast.rs` | EventBus implementation (5 tests) |
| `context.rs` | HookContext, HookType (3 tests) |
