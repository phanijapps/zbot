# AgentZero Connector Specification

## Overview

Connectors are external services that receive agent responses. When an agent execution completes, AgentZero can dispatch the response to one or more configured connectors.

## How Connectors Work

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Trigger       │────▶│   AgentZero     │────▶│   Connector     │
│ (Cron/API/Web)  │     │   Gateway       │     │   (Your Service)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              │ respond_to: ["my-connector"]
                              ▼
                        ┌─────────────────┐
                        │  HTTP POST to   │
                        │  your endpoint  │
                        └─────────────────┘
```

## Transport Types

### 1. HTTP Connector (Recommended)

The most common connector type. AgentZero sends an HTTP POST to your endpoint.

```json
{
  "type": "http",
  "callback_url": "https://your-service.com/webhook",
  "method": "POST",
  "headers": {
    "Authorization": "Bearer your-token",
    "Content-Type": "application/json"
  },
  "timeout_ms": 30000
}
```

### 2. CLI Connector

Executes a local command with the response as input.

```json
{
  "type": "cli",
  "command": "/path/to/your/script.sh",
  "args": ["--format", "json"],
  "env": {
    "API_KEY": "your-key"
  }
}
```

### 3. Future Transport Types

- `grpc` - gRPC service calls
- `websocket` - WebSocket connections
- `ipc` - Inter-process communication

## Request Payload

When AgentZero dispatches to your connector, it sends this payload:

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": null,
    "agent_id": "root",
    "timestamp": "2024-01-15T09:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "The agent's response text",
    "execution_id": "exec-xyz789",
    "conversation_id": "conv-abc123"
  }
}
```

### Payload Fields

| Field | Type | Description |
|-------|------|-------------|
| `context.session_id` | string | Unique session identifier |
| `context.thread_id` | string? | Thread ID (if applicable) |
| `context.agent_id` | string | The agent that produced the response |
| `context.timestamp` | string | ISO 8601 timestamp |
| `capability` | string | The action type (e.g., "respond") |
| `payload.message` | string | The agent's response text |
| `payload.execution_id` | string | Unique execution identifier |
| `payload.conversation_id` | string | Conversation identifier |

## Response Requirements

Your connector should respond with:

### Success (2xx)
```json
{
  "success": true,
  "message": "Processed successfully"
}
```

### Error (4xx/5xx)
```json
{
  "success": false,
  "error": "Description of what went wrong"
}
```

**Note**: AgentZero logs the result but does not retry failed dispatches automatically.

## Creating a Connector

### Step 1: Register via API

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "my-webhook",
    "name": "My Webhook Connector",
    "transport": {
      "type": "http",
      "callback_url": "http://localhost:8080/webhook",
      "method": "POST",
      "headers": {
        "X-Api-Key": "secret123"
      }
    },
    "enabled": true
  }'
```

### Step 2: Test the Connector

```bash
curl -X POST http://localhost:18791/api/connectors/my-webhook/test
```

### Step 3: Use in Requests

Include connector IDs in `respond_to` when triggering agents:

```bash
# Via API
curl -X POST http://localhost:18791/api/sessions \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "root",
    "message": "Generate a daily report",
    "respond_to": ["my-webhook"]
  }'
```

Or configure in cron jobs (connectors receive the response automatically).

## Connector Management API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/connectors` | List all connectors |
| GET | `/api/connectors/:id` | Get connector by ID |
| POST | `/api/connectors` | Create connector |
| PUT | `/api/connectors/:id` | Update connector |
| DELETE | `/api/connectors/:id` | Delete connector |
| POST | `/api/connectors/:id/test` | Test connector |
| POST | `/api/connectors/:id/enable` | Enable connector |
| POST | `/api/connectors/:id/disable` | Disable connector |

## Security Considerations

1. **Authentication**: Always use authentication headers
2. **HTTPS**: Use HTTPS in production
3. **Validation**: Validate the payload structure
4. **Timeouts**: Handle timeouts gracefully (default: 30s)
5. **Idempotency**: Design for potential duplicate deliveries

## Example Use Cases

1. **Slack Notifications**: Post agent responses to Slack channels
2. **Email Delivery**: Send responses via email services
3. **Database Logging**: Store responses in external databases
4. **Webhook Pipelines**: Trigger downstream workflows
5. **Monitoring**: Send metrics to observability platforms
