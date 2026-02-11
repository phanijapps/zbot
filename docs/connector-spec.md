# AgentZero Connector Specification

> **Version**: 1.0
> **Base URL**: `http://127.0.0.1:18791`
> **Content-Type**: All requests and responses use `application/json`

---

## 1. Architecture Overview

A **connector** is a bidirectional bridge between an external system (Signal, Slack, Discord, email, SMS, custom app) and the AgentZero agent runtime.

```
                         AgentZero Gateway (:18791)
                        +--------------------------+
                        |                          |
  YOUR SERVICE          |   Connector Registry     |         AGENT RUNTIME
  (Signal bot,   ------>|                          |-------> Agent executes
   webhook, etc)  POST  |  POST /connectors/:id/   |         with your message
                  inbound|       inbound            |
                        |                          |
                        |   When agent finishes:   |
  YOUR WEBHOOK   <------|   POST to callback_url   |<------- Agent produces
  (receives the         |   with ConnectorPayload  |         response via
   agent response)      |                          |         respond tool
                        +--------------------------+
```

### Two directions

| Direction | What happens | Your role |
|-----------|-------------|-----------|
| **Inbound** | Your service sends a message TO AgentZero | You POST to `/api/connectors/{id}/inbound` |
| **Outbound** | AgentZero sends agent responses TO your service | You expose an HTTP endpoint that receives `ConnectorPayload` |

### Lifecycle

1. **Register** your connector (once) via `POST /api/connectors`
2. **Receive inbound** — your service POSTs messages to AgentZero when events occur (e.g., Signal message received)
3. **Handle outbound** — your service exposes a webhook URL that AgentZero POSTs to when the agent responds
4. **Optionally declare metadata** — resources, capabilities, schemas, context text

---

## 2. Quick Start (3 commands)

### Step 1: Register a connector

```bash
curl -X POST http://127.0.0.1:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "signal-bridge",
    "name": "Signal Bridge",
    "transport": {
      "type": "http",
      "callback_url": "http://localhost:8080/webhook/agentzero",
      "method": "POST",
      "headers": {
        "X-Secret": "my-shared-secret"
      }
    }
  }'
```

**Response** (`201 Created`):
```json
{
  "id": "signal-bridge",
  "name": "Signal Bridge",
  "transport": {
    "type": "http",
    "callback_url": "http://localhost:8080/webhook/agentzero",
    "method": "POST",
    "headers": { "X-Secret": "my-shared-secret" }
  },
  "metadata": { "capabilities": [], "resources": [], "response_schemas": [] },
  "enabled": true,
  "outbound_enabled": true,
  "inbound_enabled": true,
  "created_at": "2026-02-10T12:00:00Z",
  "updated_at": "2026-02-10T12:00:00Z"
}
```

### Step 2: Send an inbound message

```bash
curl -X POST http://127.0.0.1:18791/api/connectors/signal-bridge/inbound \
  -H "Content-Type: application/json" \
  -d '{
    "message": "What is the weather in NYC?",
    "sender": { "id": "+1234567890", "name": "Alice" },
    "thread_id": "signal-group-abc"
  }'
```

**Response** (`202 Accepted`):
```json
{
  "session_id": "sess-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "accepted": true
}
```

The agent now processes the message asynchronously.

### Step 3: Receive the outbound response

When the agent finishes, AgentZero POSTs to your `callback_url`:

```
POST http://localhost:8080/webhook/agentzero
Content-Type: application/json
X-Secret: my-shared-secret

{
  "context": {
    "session_id": "sess-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "thread_id": "signal-group-abc",
    "agent_id": "root",
    "timestamp": "2026-02-10T12:00:05Z"
  },
  "capability": "respond",
  "payload": {
    "message": "The current weather in NYC is 42°F with partly cloudy skies."
  }
}
```

Your service reads `payload.message` and sends it back to Signal.

---

## 3. Connector Registration API

### 3.1 Create Connector

```
POST /api/connectors
```

**Request body — `CreateConnectorRequest`:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | `string` | **yes** | — | Unique identifier. Alphanumeric, `-`, `_` only. |
| `name` | `string` | **yes** | — | Human-readable display name. |
| `transport` | `ConnectorTransport` | **yes** | — | How AgentZero delivers outbound messages. See [Section 5](#5-transport-configuration). |
| `metadata` | `ConnectorMetadata` | no | `{}` | Capabilities, resources, schemas, context. See [Section 8](#8-metadata--agent-introspection). |
| `enabled` | `bool` | no | `true` | Master switch — disables both inbound and outbound. |
| `outbound_enabled` | `bool` | no | `true` | Whether agent responses are dispatched to this connector. |
| `inbound_enabled` | `bool` | no | `true` | Whether this connector can POST inbound messages. |

**ID validation rules:**
- Cannot be empty
- Characters allowed: `a-z`, `A-Z`, `0-9`, `-`, `_`
- Examples: `signal-bridge`, `slack_bot`, `my-app-v2`
- Invalid: `my connector` (space), `app@v1` (@), `` (empty)

**Responses:**

| Status | Body | Condition |
|--------|------|-----------|
| `201 Created` | `ConnectorConfig` | Success |
| `400 Bad Request` | `{"error": "...", "code": "INVALID_ID"}` | Invalid ID characters or empty |
| `409 Conflict` | `{"error": "...", "code": "CONNECTOR_EXISTS"}` | ID already taken |
| `500 Internal Server Error` | `{"error": "...", "code": "INTERNAL_ERROR"}` | Server error |

### 3.2 Update Connector

```
PUT /api/connectors/{id}
```

**Request body — `UpdateConnectorRequest`:**

All fields are optional. Only provided fields are updated. The rest remain unchanged.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string?` | New display name |
| `transport` | `ConnectorTransport?` | Replace transport config entirely |
| `metadata` | `ConnectorMetadata?` | Replace metadata entirely (not merged — full replacement) |
| `enabled` | `bool?` | Master enable/disable |
| `outbound_enabled` | `bool?` | Toggle outbound dispatch |
| `inbound_enabled` | `bool?` | Toggle inbound acceptance |

**Important**: `metadata` is a **full replacement**, not a merge. If you update metadata, send the complete metadata object including fields you want to keep.

**Responses:**

| Status | Body | Condition |
|--------|------|-----------|
| `200 OK` | `ConnectorConfig` | Success |
| `404 Not Found` | `{"error": "...", "code": "CONNECTOR_NOT_FOUND"}` | ID not found |
| `500 Internal Server Error` | `{"error": "...", "code": "INTERNAL_ERROR"}` | Server error |

### 3.3 Other CRUD Endpoints

| Method | Path | Response | Notes |
|--------|------|----------|-------|
| `GET` | `/api/connectors` | `200` — `ConnectorConfig[]` | List all connectors |
| `GET` | `/api/connectors/{id}` | `200` — `ConnectorConfig` | Get single connector |
| `DELETE` | `/api/connectors/{id}` | `204 No Content` | Delete connector |
| `GET` | `/api/connectors/{id}/metadata` | `200` — `ConnectorMetadata` | Get metadata only |
| `POST` | `/api/connectors/{id}/test` | `200`/`503` — `TestResult` | Test connectivity |
| `POST` | `/api/connectors/{id}/enable` | `200` — `ConnectorConfig` | Set `enabled: true` |
| `POST` | `/api/connectors/{id}/disable` | `200` — `ConnectorConfig` | Set `enabled: false` |

### 3.4 ConnectorConfig (full response shape)

Every read/write endpoint returns this shape:

```json
{
  "id": "signal-bridge",
  "name": "Signal Bridge",
  "transport": { ... },
  "metadata": {
    "capabilities": [],
    "resources": [],
    "response_schemas": [],
    "context": null
  },
  "enabled": true,
  "outbound_enabled": true,
  "inbound_enabled": true,
  "created_at": "2026-02-10T12:00:00Z",
  "updated_at": "2026-02-10T12:00:00Z"
}
```

### 3.5 Test Connectivity

```
POST /api/connectors/{id}/test
```

**Response — `TestResult`:**

```json
{
  "success": true,
  "message": "HTTP endpoint reachable (200 OK)",
  "latency_ms": 42
}
```

- **HTTP transport**: sends a `HEAD` request to `callback_url`, measures latency
- **CLI transport**: checks if the `command` binary exists on disk (`which`/`where`)
- Returns `503 Service Unavailable` with `success: false` if the test fails

---

## 4. Inbound Messages (External -> AgentZero)

This is how your service sends messages to AgentZero to trigger an agent session.

### 4.1 Send Inbound Message

```
POST /api/connectors/{id}/inbound
```

**Request body — `InboundPayload`:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `message` | `string` | **yes** | — | The message text from the external user. |
| `sender` | `InboundSender?` | no | `null` | Who sent this message. Passed to agent as metadata. |
| `thread_id` | `string?` | no | `null` | Conversation thread ID for threading. Carried through to outbound `context.thread_id`. |
| `agent_id` | `string?` | no | `"root"` | Which agent should handle this message. |
| `respond_to` | `string[]?` | no | `["{connector_id}"]` | Connector IDs to receive the agent's response. |
| `metadata` | `object?` | no | `null` | Arbitrary JSON passed through to the agent session. |

**`InboundSender`:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | `string` | **yes** | External user ID (phone number, Slack user ID, email, etc.) |
| `name` | `string?` | no | Display name |

### 4.2 Pre-flight Checks

Before accepting the message, AgentZero validates:

| Check | Error | Status |
|-------|-------|--------|
| Connector exists | `CONNECTOR_NOT_FOUND` | `404` |
| `connector.enabled` is `true` | `CONNECTOR_DISABLED` | `403` |
| `connector.inbound_enabled` is `true` | `INBOUND_DISABLED` | `403` |
| Execution runner is initialized | `INTERNAL_ERROR` | `503` |

### 4.3 Response

**Success** (`202 Accepted`):
```json
{
  "session_id": "sess-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "accepted": true
}
```

The `session_id` is a correlation ID. The same `session_id` will appear in the outbound `ConnectorPayload.context.session_id` when the agent responds.

**Error responses:**

| Status | Code | When |
|--------|------|------|
| `404` | `CONNECTOR_NOT_FOUND` | Connector ID doesn't exist |
| `403` | `CONNECTOR_DISABLED` | Connector's `enabled` is `false` |
| `403` | `INBOUND_DISABLED` | Connector's `inbound_enabled` is `false` |
| `503` | `INTERNAL_ERROR` | Agent runtime not ready |
| `500` | `INTERNAL_ERROR` | Failed to create session |

### 4.4 What Happens After Acceptance

1. AgentZero creates a **session** with `source: "connector"`
2. The agent specified by `agent_id` (default: `"root"`) receives the message
3. The agent processes the message (may use tools, call sub-agents, etc.)
4. When the agent calls `respond`, the response is dispatched to all connectors listed in `respond_to`
5. The inbound message is logged to the in-memory audit ring buffer

### 4.5 Metadata Handling

The `sender` field is automatically merged into session metadata:

```
If metadata provided:
  metadata.sender = { id: sender.id, name: sender.name }

If no metadata but sender provided:
  metadata = { sender: { id: sender.id, name: sender.name } }
```

The agent can access this metadata to know who it's talking to.

### 4.6 Minimal vs Full Inbound Examples

**Minimal** — just a message:
```json
{ "message": "Hello" }
```

**With sender** — agent knows who's talking:
```json
{
  "message": "Look up order #12345",
  "sender": { "id": "+1234567890", "name": "Alice" }
}
```

**With threading** — maintains conversation context:
```json
{
  "message": "What about the second item?",
  "sender": { "id": "+1234567890", "name": "Alice" },
  "thread_id": "signal-group-abc-123"
}
```

**Full** — all fields:
```json
{
  "message": "Research this topic and email the results",
  "sender": { "id": "+1234567890", "name": "Alice" },
  "thread_id": "signal-group-abc-123",
  "agent_id": "researcher",
  "respond_to": ["signal-bridge", "email-connector"],
  "metadata": {
    "channel": "research-team",
    "priority": "high",
    "attachments": []
  }
}
```

### 4.7 Inbound Audit Log

Recent inbound messages are stored in a ring buffer (max 500 entries, in-memory, not persisted across restarts).

```
GET /api/connectors/{id}/inbound-log?limit=50
```

**Query parameters:**

| Param | Type | Default | Max | Description |
|-------|------|---------|-----|-------------|
| `limit` | `integer` | `50` | `500` | Number of entries to return |

**Response** (`200 OK`):
```json
[
  {
    "connector_id": "signal-bridge",
    "message": "Hello from Signal",
    "sender": { "id": "+1234567890", "name": "Alice" },
    "thread_id": "signal-group-abc",
    "session_id": "sess-a1b2c3d4",
    "received_at": "2026-02-10T12:00:00Z"
  }
]
```

Entries are returned **newest first**.

---

## 5. Transport Configuration

Transport defines how AgentZero delivers outbound messages to your service.

### 5.1 HTTP Transport (recommended)

```json
{
  "type": "http",
  "callback_url": "http://localhost:8080/webhook/agentzero",
  "method": "POST",
  "headers": {
    "Authorization": "Bearer my-secret-token",
    "X-Custom-Header": "value"
  },
  "timeout_ms": 30000
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | `"http"` | **yes** | — | Transport discriminator |
| `callback_url` | `string` | **yes** | — | URL AgentZero will POST/PUT to |
| `method` | `string` | no | `"POST"` | HTTP method (`POST` or `PUT` only) |
| `headers` | `object` | no | `{}` | Custom headers added to every outbound request |
| `timeout_ms` | `integer?` | no | `30000` | Request timeout in milliseconds |

**Behavior:**
- AgentZero adds `Content-Type: application/json` automatically
- Your custom headers are added alongside (can include auth tokens, API keys, etc.)
- Timeout default is 30 seconds
- Non-2xx responses are logged as warnings but do NOT cause retries

### 5.2 CLI Transport

```json
{
  "type": "cli",
  "command": "/usr/local/bin/signal-cli",
  "args": ["--config", "/etc/signal", "send"],
  "env": {
    "SIGNAL_PHONE": "+1987654321"
  }
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | `"cli"` | **yes** | — | Transport discriminator |
| `command` | `string` | **yes** | — | Absolute path to executable |
| `args` | `string[]` | no | `[]` | Command-line arguments |
| `env` | `object` | no | `{}` | Extra environment variables (merged with system env) |

**Behavior:**
- The `ConnectorPayload` JSON is written to **stdin**
- **stdout** is captured as the response body
- **stderr** is captured as fallback if stdout is empty
- Exit code 0 = success, non-zero = failure

### 5.3 Future Transports (typed but not yet implemented)

These are defined in the type system but will return `UnsupportedTransport` error:

```json
// gRPC
{ "type": "grpc", "endpoint": "localhost:50051", "service": "MyService", "method": "Send" }

// WebSocket
{ "type": "web_socket", "url": "ws://localhost:9000/ws" }

// IPC (Unix socket)
{ "type": "ipc", "socket_path": "/tmp/my-connector.sock" }
```

---

## 6. Outbound Dispatch (AgentZero -> Your Service)

When the agent finishes processing and calls `respond`, AgentZero dispatches the response to your connector.

### 6.1 Outbound Payload Format — `ConnectorPayload`

This is the **exact JSON** your webhook endpoint will receive:

```json
{
  "context": {
    "session_id": "sess-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "thread_id": "signal-group-abc",
    "agent_id": "root",
    "timestamp": "2026-02-10T12:00:05Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Here is the agent's response text."
  }
}
```

### 6.2 Field Reference

**Top level:**

| Field | Type | Description |
|-------|------|-------------|
| `context` | `DispatchContext` | Metadata about the session that generated this response |
| `capability` | `string` | Which action is being performed (usually `"respond"`) |
| `payload` | `object` | The actual response data |

**`context` (DispatchContext):**

| Field | Type | Always present | Description |
|-------|------|----------------|-------------|
| `session_id` | `string` | **yes** | Session ID — matches the `session_id` from the inbound `202` response |
| `thread_id` | `string?` | if sent on inbound | The `thread_id` you provided on the inbound message, echoed back for threading |
| `agent_id` | `string` | **yes** | Which agent generated this response (e.g., `"root"`, `"researcher"`) |
| `timestamp` | `string` (ISO 8601) | **yes** | When the response was generated |

### 6.3 What Your Endpoint Should Do

1. **Parse** the JSON body as `ConnectorPayload`
2. **Extract** `payload.message` — this is the agent's text response
3. **Use** `context.thread_id` to route the reply to the correct conversation/thread
4. **Use** `context.session_id` if you need to correlate with the original inbound request
5. **Return** any 2xx status to acknowledge receipt

**Your endpoint MUST:**
- Accept `POST` requests with `Content-Type: application/json`
- Return a response within the timeout period (default 30 seconds)
- Return a 2xx status code on success

**Your endpoint response body** is captured by AgentZero for logging but is NOT used for any processing. You can return empty body, `{"ok": true}`, or anything.

### 6.4 Outbound Pre-flight Checks

Before dispatching, AgentZero checks:

| Check | Error |
|-------|-------|
| `connector.enabled` is `true` | `DispatchError::Disabled` |
| `connector.outbound_enabled` is `true` | `DispatchError::OutboundDisabled` |

If either check fails, the dispatch is silently skipped for that connector.

### 6.5 Error Handling

| Scenario | AgentZero behavior |
|----------|-------------------|
| Your endpoint returns 2xx | Logged as success |
| Your endpoint returns non-2xx | Logged as warning, no retry |
| Connection refused | Logged as error (`DispatchError::Connection`) |
| Timeout exceeded | Logged as error (`DispatchError::Timeout`) |
| DNS resolution failure | Logged as error (`DispatchError::Http`) |

**There are no automatic retries.** If reliability is critical, your webhook should be highly available, or you should implement your own retry logic by polling the session.

### 6.6 HTTP Outbound Request Details

```
POST {callback_url}
Content-Type: application/json
{your custom headers from transport.headers}

{ConnectorPayload JSON}
```

### 6.7 CLI Outbound Execution Details

```
{command} {args...}
  stdin:  {ConnectorPayload JSON}
  stdout: (captured as response body)
  stderr: (captured as fallback if stdout empty)
  env:    {system env} + {transport.env}

  exit 0  = success
  exit !0 = failure (logged as warning)
```

---

## 7. Response Routing (`respond_to`)

### 7.1 Default Routing

If you don't specify `respond_to` on the inbound message, the response goes back to the **same connector** that sent the inbound message:

```json
// Inbound from "signal-bridge" with no respond_to:
{ "message": "Hello" }

// Internally becomes: respond_to = ["signal-bridge"]
// Agent response is dispatched to signal-bridge's callback_url
```

### 7.2 Explicit Routing

Override `respond_to` to control where responses go:

```json
{
  "message": "Research this and email the results",
  "respond_to": ["email-connector"]
}
```

The response goes **only** to `email-connector`. It does **NOT** go back to the inbound source.

### 7.3 Multi-Connector Fan-Out

Send responses to multiple connectors simultaneously:

```json
{
  "message": "Important update",
  "respond_to": ["signal-bridge", "slack-connector", "email-connector"]
}
```

All three connectors receive the same `ConnectorPayload`.

### 7.4 Include Self + Others

If you want the response to go back to the source AND also to other connectors, you must include yourself explicitly:

```json
{
  "message": "Handle this and also notify the team",
  "respond_to": ["signal-bridge", "slack-connector"]
}
```

### 7.5 Routing Rules Summary

| `respond_to` value | Where response goes |
|--------------------|-------------------|
| Not provided / `null` | Back to the inbound connector only |
| `["signal-bridge"]` | Only to signal-bridge |
| `["slack", "email"]` | To slack AND email (NOT back to source) |
| `["signal-bridge", "slack"]` | Back to source AND to slack |
| `[]` (empty array) | Nowhere (response is lost) |

---

## 8. Metadata & Agent Introspection

Metadata tells agents what your connector can do. The agent sees this information in its system prompt and can use it to make decisions.

### 8.1 Capabilities

Declare what actions your connector can perform:

```json
{
  "metadata": {
    "capabilities": [
      {
        "name": "send_message",
        "description": "Send a message to a Signal user or group",
        "schema": {
          "type": "object",
          "required": ["recipient", "text"],
          "properties": {
            "recipient": {
              "type": "string",
              "description": "Phone number or group ID"
            },
            "text": {
              "type": "string",
              "description": "Message text to send"
            }
          }
        }
      },
      {
        "name": "send_reaction",
        "description": "React to a message with an emoji",
        "schema": {
          "type": "object",
          "required": ["message_id", "emoji"],
          "properties": {
            "message_id": { "type": "string" },
            "emoji": { "type": "string" }
          }
        }
      }
    ]
  }
}
```

**`ConnectorCapability`:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | **yes** | Action identifier (e.g., `"send_message"`) |
| `description` | `string?` | no | Human-readable description for the agent |
| `schema` | `object` | no | JSON Schema describing the expected payload |

The `capability` field in the outbound `ConnectorPayload` tells your service which capability the agent is invoking.

### 8.2 Resources

Declare queryable data endpoints that agents can read from:

```json
{
  "metadata": {
    "resources": [
      {
        "name": "contacts",
        "uri": "https://api.signal.example.com/v1/contacts",
        "method": "GET",
        "description": "List all Signal contacts",
        "headers": {
          "Authorization": "Bearer signal-api-token"
        },
        "response_schema": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "phone": { "type": "string" },
              "name": { "type": "string" },
              "verified": { "type": "boolean" }
            }
          }
        }
      },
      {
        "name": "group-members",
        "uri": "https://api.signal.example.com/v1/groups/{group_id}/members",
        "method": "GET",
        "description": "List members of a Signal group",
        "headers": {
          "Authorization": "Bearer signal-api-token"
        }
      }
    ]
  }
}
```

**`ConnectorResource`:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | `string` | **yes** | — | Resource identifier (e.g., `"contacts"`) |
| `uri` | `string` | **yes** | — | URL or URI template. Use `{param}` for path parameters. |
| `method` | `string` | no | `"GET"` | HTTP method (`GET` or `POST`) |
| `description` | `string?` | no | — | What this resource provides |
| `headers` | `object` | no | `{}` | Headers to include when querying this resource |
| `response_schema` | `object?` | no | — | JSON Schema describing the response format |

### 8.3 Response Schemas

Document the payload formats your connector expects on outbound:

```json
{
  "metadata": {
    "response_schemas": [
      {
        "name": "send_message",
        "description": "Schema for sending a message via Signal",
        "schema": {
          "type": "object",
          "required": ["text"],
          "properties": {
            "text": {
              "type": "string",
              "description": "Message text"
            },
            "recipient": {
              "type": "string",
              "description": "Phone number or group ID (defaults to thread sender)"
            },
            "quote_message_id": {
              "type": "string",
              "description": "Message ID to quote/reply to"
            }
          }
        }
      }
    ]
  }
}
```

**`ResponseSchema`:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | **yes** | Schema identifier |
| `description` | `string?` | no | What this schema defines |
| `schema` | `object` | **yes** | JSON Schema object |

### 8.4 Context

Free-form text injected into agent system prompts. Use this to give the agent background knowledge about your connector:

```json
{
  "metadata": {
    "context": "This connector bridges Signal messenger. Users are identified by phone number in E.164 format (+1234567890). Group IDs are base64 strings. The bot can send text messages, reactions, and read receipts. Rate limit: 60 messages per minute per recipient."
  }
}
```

### 8.5 Extra Fields

The metadata object supports arbitrary extra fields via `serde(flatten)`:

```json
{
  "metadata": {
    "capabilities": [],
    "resources": [],
    "response_schemas": [],
    "context": null,
    "custom_field": "any value",
    "settings": { "max_retries": 3, "batch_size": 10 }
  }
}
```

### 8.6 Updating Metadata

Metadata is replaced entirely on update — it is **NOT** merged:

```bash
# First, GET current metadata
CURRENT=$(curl -s http://127.0.0.1:18791/api/connectors/signal-bridge/metadata)

# Then PUT with modifications (include ALL fields you want to keep)
curl -X PUT http://127.0.0.1:18791/api/connectors/signal-bridge \
  -H "Content-Type: application/json" \
  -d '{
    "metadata": {
      "capabilities": [...],
      "resources": [...],
      "response_schemas": [...],
      "context": "Updated context text"
    }
  }'
```

---

## 9. Conversation Threading

Threading lets the agent maintain conversation context across multiple messages.

### 9.1 How It Works

```
  Signal Group Chat            AgentZero                  Your Webhook
  ─────────────────           ──────────                  ────────────

  Alice: "Hi"  ───────────►  POST /inbound
                              thread_id: "grp-abc"
                              ◄─ sess-111, accepted

                              Agent processes...

                              POST callback_url  ────────► Receive payload:
                              context.thread_id: "grp-abc"  context.thread_id = "grp-abc"
                              payload.message: "Hello!"     → send to group grp-abc

  Alice: "Thanks" ─────────► POST /inbound
                              thread_id: "grp-abc"
                              ◄─ sess-222, accepted

                              Agent processes...

                              POST callback_url  ────────► context.thread_id = "grp-abc"
                              payload.message: "Welcome!"   → send to group grp-abc
```

### 9.2 Rules

- `thread_id` is an **opaque string** — AgentZero doesn't interpret it
- You define the format (Signal group ID, Slack channel+ts, email thread ID, etc.)
- The same `thread_id` is echoed back in the outbound `context.thread_id`
- Your service uses it to route the reply to the correct conversation

---

## 10. Complete Connector Wrapper Example (Signal)

Here's the full architecture for a Signal connector wrapper:

### 10.1 Registration (on startup)

```python
import requests

AGENTZERO_URL = "http://127.0.0.1:18791"
CONNECTOR_ID = "signal-bridge"
MY_WEBHOOK_URL = "http://localhost:8080/webhook/agentzero"

def register_connector():
    resp = requests.post(f"{AGENTZERO_URL}/api/connectors", json={
        "id": CONNECTOR_ID,
        "name": "Signal Bridge",
        "transport": {
            "type": "http",
            "callback_url": MY_WEBHOOK_URL,
            "method": "POST",
            "headers": {
                "X-Connector-Secret": "shared-secret-here"
            },
            "timeout_ms": 15000
        },
        "metadata": {
            "capabilities": [
                {
                    "name": "send_message",
                    "description": "Send a text message to a Signal user or group",
                    "schema": {
                        "type": "object",
                        "required": ["text"],
                        "properties": {
                            "text": {"type": "string"},
                            "recipient": {"type": "string", "description": "Phone or group ID"}
                        }
                    }
                },
                {
                    "name": "send_reaction",
                    "description": "React to a message with an emoji",
                    "schema": {
                        "type": "object",
                        "required": ["emoji"],
                        "properties": {
                            "emoji": {"type": "string"},
                            "target_message": {"type": "string"}
                        }
                    }
                }
            ],
            "resources": [
                {
                    "name": "contacts",
                    "uri": "https://signal-api.local/v1/contacts",
                    "method": "GET",
                    "description": "List known Signal contacts"
                }
            ],
            "response_schemas": [
                {
                    "name": "send_message",
                    "description": "Expected payload format for sending messages",
                    "schema": {
                        "type": "object",
                        "required": ["text"],
                        "properties": {
                            "text": {"type": "string"},
                            "recipient": {"type": "string"}
                        }
                    }
                }
            ],
            "context": "Signal messenger bridge. Users identified by E.164 phone numbers (+1234567890). Groups identified by base64 group IDs. Supports text messages, reactions, and typing indicators."
        },
        "enabled": True,
        "outbound_enabled": True,
        "inbound_enabled": True
    })

    if resp.status_code == 201:
        print(f"Connector registered: {resp.json()['id']}")
    elif resp.status_code == 409:
        print("Connector already exists, updating...")
        requests.put(f"{AGENTZERO_URL}/api/connectors/{CONNECTOR_ID}", json={
            "transport": {
                "type": "http",
                "callback_url": MY_WEBHOOK_URL,
                "method": "POST",
                "headers": {"X-Connector-Secret": "shared-secret-here"},
                "timeout_ms": 15000
            }
        })
    else:
        raise Exception(f"Registration failed: {resp.status_code} {resp.text}")
```

### 10.2 Inbound Handler (Signal -> AgentZero)

This runs inside your Signal bot when a message arrives:

```python
def on_signal_message(envelope):
    """Called when a Signal message is received."""

    source_number = envelope["source"]       # e.g., "+1234567890"
    source_name = envelope.get("sourceName") # e.g., "Alice"
    message_text = envelope["dataMessage"]["message"]
    group_id = envelope["dataMessage"].get("groupInfo", {}).get("groupId")
    timestamp = envelope["dataMessage"]["timestamp"]

    # Determine thread_id
    thread_id = group_id if group_id else f"dm-{source_number}"

    # Forward to AgentZero
    resp = requests.post(
        f"{AGENTZERO_URL}/api/connectors/{CONNECTOR_ID}/inbound",
        json={
            "message": message_text,
            "sender": {
                "id": source_number,
                "name": source_name
            },
            "thread_id": thread_id,
            "metadata": {
                "signal_timestamp": timestamp,
                "is_group": group_id is not None,
                "group_id": group_id
            }
        }
    )

    if resp.status_code == 202:
        result = resp.json()
        print(f"Message accepted, session: {result['session_id']}")
    elif resp.status_code == 403:
        print(f"Connector disabled: {resp.json()['code']}")
    elif resp.status_code == 404:
        print("Connector not found — need to re-register")
    else:
        print(f"Inbound failed: {resp.status_code} {resp.text}")
```

### 10.3 Outbound Handler (AgentZero -> Signal)

Your webhook endpoint that receives agent responses:

```python
from flask import Flask, request, jsonify

app = Flask(__name__)

@app.route("/webhook/agentzero", methods=["POST"])
def handle_outbound():
    # Verify shared secret
    secret = request.headers.get("X-Connector-Secret")
    if secret != "shared-secret-here":
        return jsonify({"error": "unauthorized"}), 401

    # Parse ConnectorPayload
    payload = request.json
    context = payload["context"]
    capability = payload["capability"]
    data = payload["payload"]

    session_id = context["session_id"]
    thread_id = context.get("thread_id")   # Your thread_id echoed back
    agent_id = context["agent_id"]
    timestamp = context["timestamp"]

    print(f"Outbound from agent '{agent_id}', session {session_id}")
    print(f"  capability: {capability}")
    print(f"  thread_id:  {thread_id}")

    if capability == "respond":
        # Standard text response
        message_text = data.get("message", "")
        send_signal_message(thread_id, message_text)

    elif capability == "send_reaction":
        # Agent wants to react to a message
        emoji = data.get("emoji", "")
        target = data.get("target_message", "")
        send_signal_reaction(thread_id, target, emoji)

    else:
        print(f"Unknown capability: {capability}")

    return jsonify({"ok": True}), 200


def send_signal_message(thread_id, text):
    """Send a message back via Signal CLI or API."""
    if thread_id.startswith("dm-"):
        # Direct message
        recipient = thread_id.replace("dm-", "")
        # signal-cli send -m "{text}" {recipient}
    else:
        # Group message
        group_id = thread_id
        # signal-cli send -m "{text}" -g {group_id}
    print(f"Sent to {thread_id}: {text}")


def send_signal_reaction(thread_id, target_message, emoji):
    """Send a reaction via Signal."""
    print(f"Reacted {emoji} to {target_message} in {thread_id}")
```

### 10.4 Polling Inbound Log (optional, for debugging)

```python
def check_inbound_log():
    """View recent inbound messages for debugging."""
    resp = requests.get(
        f"{AGENTZERO_URL}/api/connectors/{CONNECTOR_ID}/inbound-log",
        params={"limit": 20}
    )

    for entry in resp.json():
        sender = entry.get("sender", {})
        print(f"[{entry['received_at']}] "
              f"{sender.get('name', 'unknown')} ({sender.get('id', '?')}): "
              f"{entry['message'][:80]}... "
              f"→ session {entry['session_id']}")
```

---

## 11. Error Reference

### 11.1 Error Response Format

All errors follow this shape:

```json
{
  "error": "Human-readable error message",
  "code": "MACHINE_READABLE_CODE"
}
```

### 11.2 Error Code Table

| Code | HTTP Status | When | Action |
|------|-------------|------|--------|
| `CONNECTOR_NOT_FOUND` | `404` | Connector ID doesn't exist | Register the connector first |
| `CONNECTOR_EXISTS` | `409` | ID already taken on create | Use a different ID or update existing |
| `INVALID_ID` | `400` | ID contains invalid characters | Use only `a-z`, `A-Z`, `0-9`, `-`, `_` |
| `CONNECTOR_DISABLED` | `403` | `enabled` is `false` | Enable the connector via PUT or `/enable` |
| `INBOUND_DISABLED` | `403` | `inbound_enabled` is `false` | Enable inbound via PUT |
| `INTERNAL_ERROR` | `500`/`503` | Server error or runtime not ready | Check AgentZero logs |

### 11.3 Outbound Dispatch Errors (not returned to you — logged server-side)

| Error | Cause | Effect |
|-------|-------|--------|
| `Disabled` | Connector `enabled` is `false` | Dispatch skipped |
| `OutboundDisabled` | `outbound_enabled` is `false` | Dispatch skipped |
| `Timeout` | Your webhook didn't respond in time | Logged, no retry |
| `Connection` | Your webhook URL is unreachable | Logged, no retry |
| `Http` | Network error or invalid method | Logged, no retry |
| `UnsupportedTransport` | Using gRPC/WebSocket/IPC (not yet implemented) | Dispatch fails |

---

## 12. API Reference (Compact)

| Method | Path | Request Body | Success | Description |
|--------|------|-------------|---------|-------------|
| `POST` | `/api/connectors` | `CreateConnectorRequest` | `201` + `ConnectorConfig` | Register connector |
| `GET` | `/api/connectors` | — | `200` + `ConnectorConfig[]` | List all |
| `GET` | `/api/connectors/{id}` | — | `200` + `ConnectorConfig` | Get one |
| `PUT` | `/api/connectors/{id}` | `UpdateConnectorRequest` | `200` + `ConnectorConfig` | Update (partial) |
| `DELETE` | `/api/connectors/{id}` | — | `204` | Delete |
| `GET` | `/api/connectors/{id}/metadata` | — | `200` + `ConnectorMetadata` | Get metadata |
| `POST` | `/api/connectors/{id}/test` | — | `200` + `TestResult` | Test connectivity |
| `POST` | `/api/connectors/{id}/enable` | — | `200` + `ConnectorConfig` | Enable |
| `POST` | `/api/connectors/{id}/disable` | — | `200` + `ConnectorConfig` | Disable |
| `POST` | `/api/connectors/{id}/inbound` | `InboundPayload` | `202` + `InboundResult` | Send inbound message |
| `GET` | `/api/connectors/{id}/inbound-log` | `?limit=50` | `200` + `InboundLogEntry[]` | Audit log |

---

## 13. Type Reference (JSON Shapes)

### ConnectorConfig
```json
{
  "id": "string",
  "name": "string",
  "transport": "ConnectorTransport",
  "metadata": "ConnectorMetadata",
  "enabled": "bool (default: true)",
  "outbound_enabled": "bool (default: true)",
  "inbound_enabled": "bool (default: true)",
  "created_at": "ISO 8601 datetime | null",
  "updated_at": "ISO 8601 datetime | null"
}
```

### ConnectorTransport (HTTP)
```json
{
  "type": "http",
  "callback_url": "string (required)",
  "method": "string (default: POST, allowed: POST|PUT)",
  "headers": "Record<string, string> (default: {})",
  "timeout_ms": "integer | null (default: 30000)"
}
```

### ConnectorTransport (CLI)
```json
{
  "type": "cli",
  "command": "string (required)",
  "args": "string[] (default: [])",
  "env": "Record<string, string> (default: {})"
}
```

### ConnectorMetadata
```json
{
  "capabilities": "ConnectorCapability[] (default: [])",
  "resources": "ConnectorResource[] (default: [])",
  "response_schemas": "ResponseSchema[] (default: [])",
  "context": "string | null (default: null)",
  "...extra": "any additional key-value pairs"
}
```

### ConnectorCapability
```json
{
  "name": "string (required)",
  "description": "string | null",
  "schema": "JSON Schema object (default: {})"
}
```

### ConnectorResource
```json
{
  "name": "string (required)",
  "uri": "string (required)",
  "method": "string (default: GET, allowed: GET|POST)",
  "description": "string | null",
  "headers": "Record<string, string> (default: {})",
  "response_schema": "JSON Schema object | null"
}
```

### ResponseSchema
```json
{
  "name": "string (required)",
  "schema": "JSON Schema object (required)",
  "description": "string | null"
}
```

### InboundPayload
```json
{
  "message": "string (required)",
  "sender": "InboundSender | null",
  "thread_id": "string | null",
  "agent_id": "string | null (default: root)",
  "respond_to": "string[] | null (default: [connector_id])",
  "metadata": "object | null"
}
```

### InboundSender
```json
{
  "id": "string (required)",
  "name": "string | null"
}
```

### InboundResult
```json
{
  "session_id": "string",
  "accepted": "bool"
}
```

### ConnectorPayload (outbound — what your webhook receives)
```json
{
  "context": {
    "session_id": "string",
    "thread_id": "string | null",
    "agent_id": "string",
    "timestamp": "ISO 8601 datetime"
  },
  "capability": "string",
  "payload": "object"
}
```

### TestResult
```json
{
  "success": "bool",
  "message": "string",
  "latency_ms": "integer | null"
}
```

### InboundLogEntry
```json
{
  "connector_id": "string",
  "message": "string",
  "sender": "InboundSender | null",
  "thread_id": "string | null",
  "session_id": "string",
  "received_at": "ISO 8601 datetime"
}
```

### ErrorResponse
```json
{
  "error": "string",
  "code": "string"
}
```

---

## 14. Checklist for Building a Connector Wrapper

Use this checklist when implementing your connector:

### Registration
- [ ] Choose a unique connector ID (alphanumeric, `-`, `_`)
- [ ] Set `callback_url` to your publicly reachable webhook endpoint
- [ ] Add auth headers if your webhook validates them
- [ ] Set `timeout_ms` appropriate for your response time
- [ ] Declare capabilities the agent should know about
- [ ] Declare resources if the agent should be able to query data
- [ ] Add context text describing your connector's behavior and limits
- [ ] Handle `409 Conflict` on registration (connector already exists — update instead)

### Inbound Path (your service -> AgentZero)
- [ ] POST to `/api/connectors/{id}/inbound` when external events occur
- [ ] Always include `message` (required)
- [ ] Include `sender.id` so the agent knows who's talking
- [ ] Include `thread_id` to maintain conversation context
- [ ] Handle `202 Accepted` — save `session_id` for correlation
- [ ] Handle `403` — check if connector is disabled
- [ ] Handle `404` — re-register if needed

### Outbound Path (AgentZero -> your service)
- [ ] Expose an HTTP endpoint matching your `callback_url`
- [ ] Accept `POST` with `Content-Type: application/json`
- [ ] Parse `ConnectorPayload` from request body
- [ ] Extract `payload.message` for the agent's text response
- [ ] Use `context.thread_id` to route reply to correct conversation
- [ ] Use `context.session_id` for correlation with inbound requests
- [ ] Validate auth headers (from `transport.headers`)
- [ ] Return `200 OK` on success
- [ ] Respond within `timeout_ms` (default 30s)

### Robustness
- [ ] Handle webhook being called multiple times for the same session (idempotency)
- [ ] Log `session_id` for debugging
- [ ] Monitor inbound-log endpoint for debugging message flow
- [ ] Test connectivity with `POST /api/connectors/{id}/test`
