# Echo Worker

A reference bridge worker that demonstrates the full AgentZero WebSocket worker protocol. Use this as a starting point for building your own workers.

## Quick Start

```bash
cd workers/echo_worker
npm install
npm start
```

The worker connects to `ws://localhost:18791/bridge/ws`, sends a Hello handshake, and appears in the Workers panel in the UI within 5 seconds.

To connect to a different gateway:

```bash
GATEWAY_URL=ws://localhost:18791 npm start
```

## What It Does

| Type | Name | Behavior |
|------|------|----------|
| **Capability** | `echo` | Returns whatever payload it receives |
| **Capability** | `uppercase` | Returns the payload text in UPPERCASE |
| **Resource** | `status` | Returns uptime and message counts |

## How Workers Work

Workers are external processes that connect to AgentZero via WebSocket. Unlike the old HTTP connectors (which required CRUD registration), workers **self-register** — they connect, send a Hello message declaring their capabilities and resources, and are immediately available to agents.

```
┌──────────────┐         WebSocket          ┌──────────────────┐
│  Echo Worker │ ◄─────────────────────────► │  AgentZero       │
│              │   /bridge/ws               │  Gateway         │
│  capabilities│                            │                  │
│  - echo      │   1. Worker sends Hello    │  BridgeRegistry  │
│  - uppercase │   2. Server sends HelloAck │  ┌────────────┐  │
│              │   3. Ping/Pong heartbeat   │  │ echo-worker│  │
│  resources   │   4. Server pushes work    │  └────────────┘  │
│  - status    │   5. Worker responds       │                  │
└──────────────┘                            └──────────────────┘
```

### Connection Lifecycle

1. **Connect** — Worker opens a WebSocket to `GET /bridge/ws`
2. **Hello** — Worker sends a `hello` message with `adapter_id`, `capabilities[]`, and `resources[]`
3. **HelloAck** — Server confirms registration and provides heartbeat interval
4. **Heartbeat** — Server sends `ping`, worker replies `pong` (keeps connection alive)
5. **Work** — Server sends `outbox_item`, `resource_query`, or `capability_invoke`
6. **Disconnect** — Worker is automatically unregistered; reconnects with backoff

### Reconnection

The worker automatically reconnects with exponential backoff (3s, 6s, 12s, ... capped at 30s). On reconnect it re-sends Hello to re-register.

## Protocol Reference

All messages are JSON objects with a `type` field. The protocol uses `snake_case` for all field names.

### Worker → Server

| Type | Fields | When |
|------|--------|------|
| `hello` | `adapter_id`, `capabilities[]`, `resources[]`, `resume?` | On connect |
| `pong` | _(none)_ | Reply to `ping` |
| `ack` | `outbox_id` | Outbox item delivered successfully |
| `fail` | `outbox_id`, `error`, `retry_after_seconds?` | Outbox item delivery failed |
| `resource_response` | `request_id`, `data` | Reply to `resource_query` |
| `capability_response` | `request_id`, `result` | Reply to `capability_invoke` |
| `inbound` | `text`, `thread_id?`, `sender?`, `agent_id?`, `metadata?` | User message → agent |

### Server → Worker

| Type | Fields | When |
|------|--------|------|
| `hello_ack` | `server_time`, `heartbeat_seconds` | After valid Hello |
| `ping` | _(none)_ | Heartbeat check |
| `outbox_item` | `outbox_id`, `capability`, `payload` | Agent response routed to worker |
| `resource_query` | `request_id`, `resource`, `params?` | Agent queries a resource |
| `capability_invoke` | `request_id`, `capability`, `payload` | Agent invokes a capability |
| `error` | `message` | Protocol error |

### Message Examples

**Hello (worker → server):**
```json
{
  "type": "hello",
  "adapter_id": "echo-worker",
  "capabilities": [
    {
      "name": "echo",
      "description": "Echoes back the payload",
      "schema": {
        "type": "object",
        "properties": {
          "text": { "type": "string" }
        }
      }
    }
  ],
  "resources": [
    {
      "name": "status",
      "description": "Worker uptime and message counts"
    }
  ]
}
```

**HelloAck (server → worker):**
```json
{
  "type": "hello_ack",
  "server_time": "2026-02-11T10:00:00Z",
  "heartbeat_seconds": 20
}
```

**OutboxItem (server → worker):**
```json
{
  "type": "outbox_item",
  "outbox_id": "obx-abc123",
  "capability": "echo",
  "payload": { "text": "Hello from an agent!" }
}
```

**Ack (worker → server):**
```json
{
  "type": "ack",
  "outbox_id": "obx-abc123"
}
```

**ResourceQuery (server → worker):**
```json
{
  "type": "resource_query",
  "request_id": "req-xyz",
  "resource": "status",
  "params": null
}
```

**ResourceResponse (worker → server):**
```json
{
  "type": "resource_response",
  "request_id": "req-xyz",
  "data": {
    "adapter_id": "echo-worker",
    "uptime_seconds": 3600,
    "messages_received": 42
  }
}
```

**CapabilityInvoke (server → worker):**
```json
{
  "type": "capability_invoke",
  "request_id": "req-abc",
  "capability": "uppercase",
  "payload": { "text": "hello world" }
}
```

**CapabilityResponse (worker → server):**
```json
{
  "type": "capability_response",
  "request_id": "req-abc",
  "result": { "text": "HELLO WORLD" }
}
```

**Inbound (worker → server) — triggers an agent:**
```json
{
  "type": "inbound",
  "text": "What's the weather like?",
  "thread_id": "slack-thread-123",
  "sender": { "id": "U123", "name": "Alice" },
  "agent_id": "root"
}
```

## Testing with wscat

You can test the protocol manually without running the worker:

```bash
# Install wscat globally
npm i -g wscat

# Connect to the bridge
wscat -c ws://localhost:18791/bridge/ws

# Send Hello
{"type":"hello","adapter_id":"test-manual","capabilities":[{"name":"echo","description":"Echoes payload"}],"resources":[{"name":"status","description":"Worker status"}]}

# Respond to pings
{"type":"pong"}
```

## Building Your Own Worker

1. Copy this directory as a starting point
2. Change `ADAPTER_ID` to your worker's name
3. Update `capabilities` and `resources` in `buildHello()`
4. Implement your logic in `handleOutboxItem`, `handleResourceQuery`, and `handleCapabilityInvoke`
5. To send user messages to agents, send `inbound` messages with the text and optional thread_id

### Key Rules

- **Always ACK or FAIL outbox items** — the server tracks delivery state and will retry unacknowledged items
- **Always include `request_id`** in resource_response and capability_response — the server uses it to correlate requests with waiting agents
- **Respond to pings** — the server will disconnect workers that miss heartbeats
- **Use `adapter_id`** consistently — it identifies your worker across reconnects

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GATEWAY_URL` | `ws://localhost:18791` | Gateway WebSocket base URL |
| `ADAPTER_ID` | `echo-worker` | Unique worker identifier |
