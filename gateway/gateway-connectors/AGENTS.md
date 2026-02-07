# gateway-connectors

External bidirectional connectors for dispatching agent responses at execution completion. Supports HTTP, gRPC, WebSocket, IPC, CLI transports.

## Build & Test

```bash
cargo test -p gateway-connectors    # 10 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `ConnectorRegistry` | In-memory cached registry with disk persistence |
| `ConnectorService` | CRUD operations for connector configs |
| `ConnectorConfig` | Connector definition (transport, capabilities, metadata) |
| `ConnectorTransport` | Http, Grpc, WebSocket, Ipc, Cli |
| `DispatchContext` | Context for dispatching a response |
| `DispatchResponse` / `DispatchError` | Dispatch result types |

## Public API (ConnectorRegistry)

| Method | Purpose |
|--------|---------|
| `init()` | Initialize registry from disk |
| `list()` / `get()` | Query connectors |
| `create()` / `update()` / `delete()` | CRUD |
| `get_enabled_outbound()` | Filter for outbound dispatch |
| `dispatch_to_many()` / `dispatch_to_one()` | Send responses |

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | ConnectorRegistry |
| `service.rs` | ConnectorService CRUD |
| `config.rs` | Config types |
| `dispatch.rs` | Dispatch logic |
