# Connector Specification

## 1. Trigger an Agent

Send a POST request to start an agent execution:

```
POST http://localhost:18791/api/gateway/submit
Content-Type: application/json

{
  "agent_id": "root",
  "message": "Your prompt to the agent",
  "respond_to": ["your-connector-id"]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `agent_id` | Yes | Always use `"root"` |
| `message` | Yes | The prompt/instruction for the agent |
| `respond_to` | Yes | Array of connector IDs to receive the response |

---

## 2. Receive Agent Response

Your webhook receives a POST with this payload:

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

**Your endpoint must:**
- Handle `HEAD /webhook` → return `200 OK` (for connectivity test)
- Handle `POST /webhook` → return `{"success": true}` or `{"success": false, "error": "reason"}`

---

## 3. Metadata Reference

### Context Object

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | string | Groups related executions |
| `thread_id` | string \| null | For threaded conversations |
| `agent_id` | string | Which agent responded |
| `timestamp` | string | ISO 8601 dispatch time |

### Payload Object

| Field | Type | Description |
|-------|------|-------------|
| `message` | string | The agent's response |
| `execution_id` | string | Unique execution ID |
| `conversation_id` | string | Conversation ID |

---

## Register Your Connector

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "your-connector-id",
    "name": "Your Connector Name",
    "transport": {
      "type": "http",
      "callback_url": "http://your-server/webhook",
      "method": "POST",
      "headers": {}
    },
    "enabled": true
  }'
```
