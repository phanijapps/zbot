# AgentZero Bridge Worker Protocol Specification

**Version**: 1.0
**Transport**: WebSocket (RFC 6455)
**Encoding**: JSON (UTF-8)
**Endpoint**: `GET /bridge/ws`

---

## Overview

A **worker** is an external process that connects to the AgentZero gateway over WebSocket. Workers self-register by declaring their identity, capabilities, and resources in a handshake message. Once connected, AI agents can query the worker's resources and invoke its capabilities through a unified tool interface.

Workers are the integration layer between AgentZero and any external system — Slack, CRMs, databases, APIs, IoT devices, or custom services.

```
┌──────────────┐                           ┌──────────────────┐
│   Worker     │      WebSocket            │   AgentZero      │
│              │ ◄───────────────────────► │   Gateway        │
│ adapter_id:  │   GET /bridge/ws          │                  │
│ "my-worker"  │                           │                  │
│              │   1. Connect              │  BridgeRegistry  │
│ capabilities:│   2. Hello ──────────►    │  tracks worker   │
│ - send_msg   │   3. ◄────────── HelloAck │                  │
│ - create_tkt │   4. Ping/Pong heartbeat  │  Agents use      │
│              │   5. ◄── work (query/     │  query_resource  │
│ resources:   │          invoke/outbox)   │  tool to talk    │
│ - contacts   │   6. response ──────►     │  to workers      │
│ - channels   │                           │                  │
└──────────────┘                           └──────────────────┘
```

---

## 1. Connection Lifecycle

### 1.1 Connect

Open a WebSocket connection to the gateway:

```
ws://<host>:<port>/bridge/ws
```

Default port is `18791` (same as the HTTP API).

### 1.2 Handshake

Immediately after the WebSocket is open, the worker MUST send a **Hello** message. The server will respond with **HelloAck** on success or **Error** + disconnect on failure.

### 1.3 Heartbeat

The server sends **Ping** messages at regular intervals (specified in `HelloAck.heartbeat_seconds`). The worker MUST respond with **Pong**. Workers that miss heartbeats will be disconnected.

### 1.4 Disconnect

When the WebSocket closes, the worker is immediately unregistered from the server. The worker should reconnect with exponential backoff and re-send Hello.

### 1.5 Duplicate Rejection

Only one worker per `adapter_id` can be connected at a time. If a second worker connects with the same `adapter_id`, the server rejects it with an Error message.

---

## 2. Message Format

All messages are JSON objects with a required `type` field. Field names use `snake_case`.

```json
{
  "type": "<message_type>",
  ...fields
}
```

---

## 3. Worker → Server Messages

### 3.1 `hello`

Sent once, immediately after connection. Declares the worker's identity and what it offers.

```json
{
  "type": "hello",
  "adapter_id": "string (required, unique identifier)",
  "capabilities": [
    {
      "name": "string (required, e.g. 'send_message')",
      "description": "string (optional, human-readable)",
      "schema": { "JSON Schema object (optional, describes payload)" }
    }
  ],
  "resources": [
    {
      "name": "string (required, e.g. 'contacts')",
      "description": "string (optional, human-readable)"
    }
  ],
  "resume": {
    "last_acked_id": "string (optional, for outbox replay)"
  }
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `adapter_id` | Yes | Unique worker identifier. Used as the `connector_id` when agents interact with this worker. Must be stable across reconnects. |
| `capabilities` | No | Array of actions the worker can perform. Agents invoke these via the `invoke` action. Default: `[]` |
| `resources` | No | Array of data sources the worker exposes. Agents query these via the `query` action. Default: `[]` |
| `resume` | No | If reconnecting, set `last_acked_id` to the last outbox item the worker successfully processed. The server will replay any items after that ID. |

#### Capability Schema

The optional `schema` field on a capability uses [JSON Schema](https://json-schema.org/) to describe the expected payload structure. This is surfaced to agents so they know what parameters to pass.

```json
{
  "name": "send_message",
  "description": "Send a message to a channel",
  "schema": {
    "type": "object",
    "properties": {
      "channel": { "type": "string", "description": "Channel name or ID" },
      "text": { "type": "string", "description": "Message body" }
    },
    "required": ["channel", "text"]
  }
}
```

### 3.2 `pong`

Reply to a server Ping. No additional fields.

```json
{ "type": "pong" }
```

### 3.3 `ack`

Acknowledge successful processing of an outbox item.

```json
{
  "type": "ack",
  "outbox_id": "string (required, from the outbox_item)"
}
```

### 3.4 `fail`

Report failed processing of an outbox item. The server may retry later.

```json
{
  "type": "fail",
  "outbox_id": "string (required)",
  "error": "string (required, human-readable error description)",
  "retry_after_seconds": 30
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `outbox_id` | Yes | ID of the failed outbox item |
| `error` | Yes | Error description |
| `retry_after_seconds` | No | Suggested delay before retry |

### 3.5 `resource_response`

Reply to a ResourceQuery. Contains the queried data.

```json
{
  "type": "resource_response",
  "request_id": "string (required, echo from resource_query)",
  "data": { "any JSON value — the query result" }
}
```

The `request_id` MUST match the `request_id` from the originating `resource_query`. The server uses this to correlate the response with the waiting agent.

### 3.6 `capability_response`

Reply to a CapabilityInvoke. Contains the invocation result.

```json
{
  "type": "capability_response",
  "request_id": "string (required, echo from capability_invoke)",
  "result": { "any JSON value — the invocation result" }
}
```

### 3.7 `inbound`

Send a user message to trigger an agent execution. This is how external services (Slack, Discord, SMS, etc.) route user messages into AgentZero.

```json
{
  "type": "inbound",
  "text": "string (required, the user's message)",
  "thread_id": "string (optional, for conversation threading)",
  "sender": {
    "id": "string (required, external user ID)",
    "name": "string (optional, display name)"
  },
  "agent_id": "string (optional, target agent, defaults to 'root')",
  "metadata": { "arbitrary JSON (optional)" }
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `text` | Yes | The message to send to the agent |
| `thread_id` | No | External thread/conversation ID. Messages with the same `thread_id` share a session, enabling multi-turn conversations. |
| `sender` | No | Who sent the message. Stored in session metadata. |
| `agent_id` | No | Which agent should handle this. Defaults to `"root"`. |
| `metadata` | No | Arbitrary key-value data attached to the session. |

The agent's response will be delivered back to this worker via an `outbox_item` push (if the agent uses `respond` with this connector as a target).

---

## 4. Server → Worker Messages

### 4.1 `hello_ack`

Confirms successful registration.

```json
{
  "type": "hello_ack",
  "server_time": "2026-02-11T10:00:00Z",
  "heartbeat_seconds": 20
}
```

| Field | Description |
|-------|-------------|
| `server_time` | Server's current UTC timestamp (ISO 8601) |
| `heartbeat_seconds` | How often the server will send Ping. Worker should expect Pings at this interval. |

### 4.2 `ping`

Heartbeat check. Worker MUST reply with `pong`.

```json
{ "type": "ping" }
```

### 4.3 `outbox_item`

Pushes a message from an agent to the worker. This is how agent responses reach external systems.

```json
{
  "type": "outbox_item",
  "outbox_id": "string (unique, for ACK/FAIL tracking)",
  "capability": "string (which capability to use)",
  "payload": { "any JSON — the data to process" }
}
```

| Field | Description |
|-------|-------------|
| `outbox_id` | Unique identifier for this delivery. Worker MUST respond with `ack` or `fail` using this ID. |
| `capability` | Which of the worker's declared capabilities to invoke. |
| `payload` | The data to process. Structure matches the capability's schema. |

**Delivery contract**: The worker MUST respond with either `ack` or `fail` for every `outbox_item`. Unacknowledged items remain in the server's outbox and will be retried on reconnect or by the server's retry loop (30s interval).

### 4.4 `resource_query`

Requests data from one of the worker's declared resources.

```json
{
  "type": "resource_query",
  "request_id": "string (correlation ID)",
  "resource": "string (resource name from Hello)",
  "params": { "optional JSON — query parameters" }
}
```

| Field | Description |
|-------|-------------|
| `request_id` | Correlation ID. Worker MUST include this in the `resource_response`. |
| `resource` | Name of the resource to query (must match a resource declared in Hello). |
| `params` | Optional parameters for the query (e.g., filters, pagination). |

**Response contract**: Worker MUST reply with a `resource_response` containing the same `request_id`. The server has a **30-second timeout** — if no response arrives, the agent receives a timeout error.

### 4.5 `capability_invoke`

Invokes one of the worker's declared capabilities synchronously. Unlike `outbox_item` (fire-and-forget push), this expects a response that the agent is waiting for.

```json
{
  "type": "capability_invoke",
  "request_id": "string (correlation ID)",
  "capability": "string (capability name from Hello)",
  "payload": { "any JSON — invocation parameters" }
}
```

| Field | Description |
|-------|-------------|
| `request_id` | Correlation ID. Worker MUST include this in the `capability_response`. |
| `capability` | Name of the capability to invoke (must match a capability declared in Hello). |
| `payload` | Invocation parameters. Structure should match the capability's schema. |

**Response contract**: Worker MUST reply with a `capability_response` containing the same `request_id`. **30-second timeout** applies.

### 4.6 `error`

Server-side error (e.g., duplicate adapter_id, invalid Hello).

```json
{
  "type": "error",
  "message": "string (human-readable error description)"
}
```

After an Error during handshake, the server closes the connection.

---

## 5. Outbox Delivery Guarantees

The server persists outbox items in SQLite before pushing them to the worker. This provides **at-least-once delivery**:

1. Agent response is written to the outbox (persisted to disk)
2. If the worker is connected, the item is pushed immediately
3. If the worker is offline, the item waits in the outbox
4. On reconnect, the worker can set `resume.last_acked_id` to replay missed items
5. A background retry loop re-pushes unacknowledged items every 30 seconds

Workers should be **idempotent** — processing the same `outbox_id` twice should be safe.

### Outbox Item Lifecycle

```
pending ──► inflight ──► sent (on ack)
                    └──► failed (on fail, will retry)
```

---

## 6. How Agents Interact with Workers

Agents use the `query_resource` tool with three actions:

### 6.1 `list_resources`

Discovers all connected workers and their capabilities/resources.

```json
{ "action": "list_resources" }
```

Returns:
```json
{
  "connectors": [
    {
      "connector_id": "echo-worker",
      "name": "echo-worker",
      "resources": [
        { "name": "status", "type": "resource", "method": "GET", "description": "..." }
      ],
      "capabilities": [
        { "name": "echo", "type": "capability", "method": "POST", "description": "...", "schema": {...} }
      ]
    }
  ]
}
```

### 6.2 `query`

Fetches data from a worker's resource. This sends a `resource_query` to the worker and waits for the `resource_response`.

```json
{
  "action": "query",
  "connector_id": "echo-worker",
  "resource": "status",
  "params": {}
}
```

### 6.3 `invoke`

Invokes a worker's capability synchronously. This sends a `capability_invoke` to the worker and waits for the `capability_response`.

```json
{
  "action": "invoke",
  "connector_id": "echo-worker",
  "capability": "echo",
  "payload": { "text": "hello" }
}
```

The `connector_id` used by agents is the worker's `adapter_id` from the Hello message.

---

## 7. Reconnection

Workers should implement automatic reconnection with exponential backoff:

```
attempt 1: wait 3s
attempt 2: wait 6s
attempt 3: wait 12s
attempt 4: wait 24s
attempt 5+: wait 30s (capped)
```

On reconnect:
1. Open a new WebSocket
2. Send Hello again (re-registers the worker)
3. Optionally include `resume.last_acked_id` to replay missed outbox items
4. Reset backoff counter on successful HelloAck

---

## 8. Design Guidelines

### Naming

- `adapter_id`: Use lowercase with hyphens. Examples: `slack-worker`, `jira-bridge`, `crm-sync`
- Capability names: Use `snake_case` verbs. Examples: `send_message`, `create_ticket`, `update_contact`
- Resource names: Use `snake_case` nouns. Examples: `contacts`, `channels`, `recent_messages`

### Capabilities vs Resources

| | Resources | Capabilities |
|--|-----------|-------------|
| **Direction** | Read (GET) | Write (POST) |
| **Purpose** | Fetch data for the agent to reason about | Perform actions in the external system |
| **Examples** | `contacts`, `channels`, `order_status` | `send_message`, `create_ticket`, `update_record` |
| **Idempotent** | Always | Should be, but not required |

### Schema Best Practices

- Always provide `description` on capabilities and resources — agents use these to understand when to use them
- Provide `schema` on capabilities — agents use this to construct valid payloads
- Keep schemas simple — agents work best with flat objects and clear field descriptions

### Error Handling

- For `resource_response` and `capability_response`, return errors in the data/result field:
  ```json
  { "type": "resource_response", "request_id": "...", "data": { "error": "Not found" } }
  ```
- For `outbox_item`, use `fail` with a descriptive error:
  ```json
  { "type": "fail", "outbox_id": "...", "error": "Rate limited by Slack API" }
  ```

---

## 9. Complete Example: Minimal Worker

A minimal worker in pseudocode:

```
connect(ws://gateway:18791/bridge/ws)

on open:
  send({
    type: "hello",
    adapter_id: "my-worker",
    capabilities: [{ name: "greet", description: "Say hello" }],
    resources: [{ name: "health", description: "Health check" }]
  })

on message(msg):
  switch msg.type:
    case "hello_ack":
      log("registered")

    case "ping":
      send({ type: "pong" })

    case "outbox_item":
      process(msg.capability, msg.payload)
      send({ type: "ack", outbox_id: msg.outbox_id })

    case "resource_query":
      data = lookup(msg.resource, msg.params)
      send({ type: "resource_response", request_id: msg.request_id, data: data })

    case "capability_invoke":
      result = execute(msg.capability, msg.payload)
      send({ type: "capability_response", request_id: msg.request_id, result: result })

    case "error":
      log(msg.message)

on close:
  wait(backoff)
  reconnect()
```

---

## 10. Reference Implementations

| Language | Location | Description |
|----------|----------|-------------|
| Node.js | `workers/echo_worker/` | Echo worker with `echo`, `uppercase` capabilities and `status` resource |

---

## Appendix A: Message Type Summary

| Direction | Type | Response Required |
|-----------|------|-------------------|
| W→S | `hello` | Server sends `hello_ack` or `error` |
| W→S | `pong` | No |
| W→S | `ack` | No |
| W→S | `fail` | No (server may retry the item) |
| W→S | `resource_response` | No |
| W→S | `capability_response` | No |
| W→S | `inbound` | No (triggers async agent execution) |
| S→W | `hello_ack` | No |
| S→W | `ping` | Worker MUST send `pong` |
| S→W | `outbox_item` | Worker MUST send `ack` or `fail` |
| S→W | `resource_query` | Worker MUST send `resource_response` (30s timeout) |
| S→W | `capability_invoke` | Worker MUST send `capability_response` (30s timeout) |
| S→W | `error` | No |
