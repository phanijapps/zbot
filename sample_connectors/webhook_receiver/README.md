# Webhook Receiver

A simple webhook server that receives callbacks from AgentZero connectors.

## Quick Start

### Node.js

```bash
npm install
npm start
```

### Python

```bash
pip install -r requirements.txt
python app.py
```

Server runs at `http://localhost:8080`.

## Register as Connector

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "local-webhook",
    "name": "Local Webhook Receiver",
    "transport": {
      "type": "http",
      "callback_url": "http://localhost:8080/webhook",
      "method": "POST",
      "headers": {}
    },
    "enabled": true
  }'
```

## Test

```bash
curl -X POST http://localhost:18791/api/connectors/local-webhook/test
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/webhook` | Main webhook endpoint |

## Payload Format

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

## Customization

Edit `server.js` (Node) or `app.py` (Python) to add your custom processing logic in the webhook handler.
