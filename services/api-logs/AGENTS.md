# api-logs

Execution logging and tracing. Captures agent traces, tool calls, and session lifecycle events with filtering and cleanup.

## Build & Test

```bash
cargo test -p api-logs
```

## Key Types

| Type | Purpose |
|------|---------|
| `ExecutionLog` | Single log entry with level, category, timestamp, metadata |
| `LogLevel` | Debug, Info, Warn, Error |
| `LogCategory` | Session, Token, ToolCall, ToolResult, Thinking, Delegation, System, Error |
| `LogSession` | Summary of an execution session |
| `SessionDetail` | Session with all logs |
| `LogFilter` | Query criteria (agent_id, level, time range) |

## Public API (LogService)

| Method | Purpose |
|--------|---------|
| `log()` | Emit single log entry |
| `log_batch()` | Emit multiple entries |
| `log_session_start()` | Session started event |
| `log_session_end()` | Session completed/errored event |

## HTTP Routes

```
GET    /sessions      — List log sessions
GET    /sessions/:id  — Session with logs
DELETE /sessions/:id  — Delete session logs
DELETE /cleanup       — Clean old logs
```

## Trait

```rust
pub trait DbProvider {
    fn get_connection(&self) -> &Connection;
}
```

## File Structure

| File | Purpose |
|------|---------|
| `types.rs` | Data types |
| `service.rs` | LogService implementation |
| `repository.rs` | Database operations |
| `handlers.rs` | HTTP route handlers |
| `lib.rs` | Routes + schema constant |
