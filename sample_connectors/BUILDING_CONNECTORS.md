# Connector Specification

## 1. Trigger an Agent

```
POST http://localhost:18791/api/gateway/submit
Content-Type: application/json
```

```json
{
  "agent_id": "root",
  "message": "Your prompt to the agent",
  "respond_to": ["your-connector-id"],
  "metadata": {
    "user_id": "u-123",
    "channel": "slack-general",
    "custom_field": "any value"
  },
  "thread_id": "thread-456",
  "external_ref": "slack-msg-789"
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `agent_id` | Yes | Always use `"root"` |
| `message` | Yes | The prompt for the agent |
| `respond_to` | Yes | Connector IDs to receive the response |
| `metadata` | No | Arbitrary JSON passed to agent context |
| `thread_id` | No | For threaded conversations |
| `external_ref` | No | Your reference ID for correlation |

---

## 2. Receive Agent Response

Your webhook receives:

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": "thread-456",
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

**Your endpoint must:**
- `HEAD /webhook` → return `200 OK`
- `POST /webhook` → return `{"success": true}`

---

## 3. Metadata Reference

### Trigger Fields (Inbound)

| Field | Type | Description |
|-------|------|-------------|
| `agent_id` | string | Target agent (use `"root"`) |
| `message` | string | Prompt text |
| `respond_to` | string[] | Connector IDs for response |
| `metadata` | object | Custom data for agent context |
| `thread_id` | string | Thread identifier |
| `external_ref` | string | Your correlation ID |
| `session_id` | string | Continue existing session |
| `connector_id` | string | Your connector ID |

### Response Fields (Outbound)

| Field | Type | Description |
|-------|------|-------------|
| `context.session_id` | string | Session identifier |
| `context.thread_id` | string \| null | Thread ID (echoed back) |
| `context.agent_id` | string | Responding agent |
| `context.timestamp` | string | ISO 8601 timestamp |
| `payload.message` | string | Agent's response |
| `payload.execution_id` | string | Execution ID |
| `payload.conversation_id` | string | Conversation ID |

---

## Register Connector

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "your-connector-id",
    "name": "Your Connector",
    "transport": {
      "type": "http",
      "callback_url": "http://your-server/webhook",
      "method": "POST",
      "headers": {}
    },
    "enabled": true
  }'
```
