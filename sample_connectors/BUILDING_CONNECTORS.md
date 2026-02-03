# Building AgentZero Connectors

A complete guide for developers building connectors that integrate with AgentZero.

## What is a Connector?

A connector is an HTTP service that receives agent responses from AgentZero. When an agent completes execution, AgentZero dispatches the response to your connector's webhook endpoint.

```
Agent executes → AgentZero Gateway → POST to your connector → Your processing
```

## Quick Start

### 1. Create Your Webhook Server

Your server needs two endpoints:

```javascript
// Minimal Node.js example
const express = require('express');
const app = express();
app.use(express.json());

// HEAD - Required for connectivity testing
app.head('/webhook', (req, res) => res.status(200).end());

// POST - Receives agent responses
app.post('/webhook', (req, res) => {
  const { context, capability, payload } = req.body;
  console.log('Received:', payload.message);
  res.json({ success: true });
});

app.listen(8080);
```

### 2. Register Your Connector

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "my-connector",
    "name": "My Connector",
    "transport": {
      "type": "http",
      "callback_url": "http://localhost:8080/webhook",
      "method": "POST",
      "headers": {}
    },
    "enabled": true
  }'
```

### 3. Test Connectivity

```bash
curl -X POST http://localhost:18791/api/connectors/my-connector/test
```

### 4. Trigger an Agent with Your Connector

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "root",
    "message": "Hello, what can you help me with?",
    "respond_to": ["my-connector"]
  }'
```

---

## Webhook Payload Format

When AgentZero dispatches to your connector, you receive this JSON payload:

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
    "message": "The agent's response text goes here",
    "execution_id": "exec-xyz789",
    "conversation_id": "conv-abc123"
  }
}
```

### Field Reference

| Field | Type | Description |
|-------|------|-------------|
| `context.session_id` | string | Groups related executions together |
| `context.thread_id` | string \| null | For threaded conversations (optional) |
| `context.agent_id` | string | Which agent produced this response |
| `context.timestamp` | string | ISO 8601 timestamp of dispatch |
| `capability` | string | Always `"respond"` for now |
| `payload.message` | string | **The agent's response text** |
| `payload.execution_id` | string | Unique identifier for this execution |
| `payload.conversation_id` | string | Conversation context identifier |

---

## Required Endpoints

### HEAD /webhook

AgentZero sends a HEAD request to verify your connector is reachable before dispatching.

```javascript
app.head('/webhook', (req, res) => {
  res.status(200).end();
});
```

### POST /webhook

Receives the actual agent response.

**Expected Response (Success):**
```json
{
  "success": true,
  "message": "Processed successfully"
}
```

**Expected Response (Error):**
```json
{
  "success": false,
  "error": "Description of what went wrong"
}
```

---

## Connector Registration

### Full Registration Schema

```json
{
  "id": "unique-connector-id",
  "name": "Human Readable Name",
  "transport": {
    "type": "http",
    "callback_url": "https://your-domain.com/webhook",
    "method": "POST",
    "headers": {
      "Authorization": "Bearer your-secret-token",
      "X-Custom-Header": "value"
    }
  },
  "enabled": true
}
```

### Transport Options

**HTTP Transport (Recommended):**
```json
{
  "type": "http",
  "callback_url": "https://your-endpoint.com/webhook",
  "method": "POST",
  "headers": {
    "Authorization": "Bearer token"
  }
}
```

**CLI Transport (Local Scripts):**
```json
{
  "type": "cli",
  "command": "/path/to/script.sh",
  "args": ["--format", "json"]
}
```

---

## Management API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/connectors` | Register new connector |
| `GET` | `/api/connectors` | List all connectors |
| `GET` | `/api/connectors/:id` | Get connector details |
| `PUT` | `/api/connectors/:id` | Update connector |
| `DELETE` | `/api/connectors/:id` | Remove connector |
| `POST` | `/api/connectors/:id/test` | Test connectivity |
| `POST` | `/api/connectors/:id/enable` | Enable connector |
| `POST` | `/api/connectors/:id/disable` | Disable connector |

---

## Complete Examples

### Node.js (Express)

```javascript
const express = require('express');
const app = express();

app.use(express.json());

app.head('/webhook', (req, res) => {
  res.status(200).end();
});

app.post('/webhook', (req, res) => {
  const { context, capability, payload } = req.body;

  console.log('='.repeat(50));
  console.log('Session:', context.session_id);
  console.log('Agent:', context.agent_id);
  console.log('Message:', payload.message);
  console.log('='.repeat(50));

  // Your processing logic here
  // - Forward to Slack/Discord/Teams
  // - Store in database
  // - Trigger workflows
  // - Send emails

  res.json({
    success: true,
    message: 'Processed',
    received_at: new Date().toISOString()
  });
});

app.listen(8080, () => {
  console.log('Connector running on http://localhost:8080');
});
```

### Python (Flask)

```python
from flask import Flask, request, jsonify
from datetime import datetime

app = Flask(__name__)

@app.route('/webhook', methods=['HEAD'])
def webhook_head():
    return '', 200

@app.route('/webhook', methods=['POST'])
def webhook():
    data = request.get_json() or {}

    context = data.get('context', {})
    payload = data.get('payload', {})

    print('=' * 50)
    print(f"Session: {context.get('session_id')}")
    print(f"Agent: {context.get('agent_id')}")
    print(f"Message: {payload.get('message')}")
    print('=' * 50)

    # Your processing logic here

    return jsonify({
        'success': True,
        'message': 'Processed',
        'received_at': datetime.utcnow().isoformat() + 'Z'
    })

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8080)
```

### Bash (CLI Connector)

For CLI transport, create a script that reads JSON from stdin:

```bash
#!/bin/bash
# connector.sh

# Read JSON payload from stdin
PAYLOAD=$(cat)

# Parse with jq
MESSAGE=$(echo "$PAYLOAD" | jq -r '.payload.message')
SESSION=$(echo "$PAYLOAD" | jq -r '.context.session_id')

echo "Received from session $SESSION: $MESSAGE"

# Your processing here
# Example: append to log file
echo "[$(date)] $MESSAGE" >> /var/log/agent-responses.log

# Output success response
echo '{"success": true}'
```

Register as CLI connector:
```bash
curl -X POST http://localhost:18791/api/connectors \
  -d '{
    "id": "log-connector",
    "name": "Log File Connector",
    "transport": {
      "type": "cli",
      "command": "/path/to/connector.sh"
    },
    "enabled": true
  }'
```

---

## Using Connectors

### With API Requests

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "root",
    "message": "Generate a summary report",
    "respond_to": ["my-connector", "slack-notifier"]
  }'
```

### With Scheduled Jobs (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-report",
    "name": "Daily Report",
    "schedule": "0 0 9 * * *",
    "message": "Generate the daily report",
    "respond_to": ["email-connector"],
    "enabled": true
  }'
```

Note: Cron schedule uses 6-field format: `sec min hour day month weekday`

---

## Security Best Practices

1. **Use HTTPS** in production environments
2. **Authenticate requests** with headers (API keys, Bearer tokens)
3. **Validate payload structure** before processing
4. **Handle timeouts** - AgentZero has a 30-second default timeout
5. **Design for idempotency** - you may receive duplicate deliveries
6. **Log requests** for debugging and audit trails

### Example: Validating Requests

```javascript
app.post('/webhook', (req, res) => {
  // Verify auth header
  const apiKey = req.headers['x-api-key'];
  if (apiKey !== process.env.EXPECTED_API_KEY) {
    return res.status(401).json({ success: false, error: 'Unauthorized' });
  }

  // Validate payload structure
  const { context, payload } = req.body;
  if (!payload?.message) {
    return res.status(400).json({ success: false, error: 'Missing message' });
  }

  // Process...
  res.json({ success: true });
});
```

---

## Troubleshooting

### Connector Test Fails

1. Ensure your server is running and accessible
2. Check the HEAD endpoint returns 200
3. Verify the callback_url is correct
4. Check firewall/network settings

### Not Receiving Webhooks

1. Verify connector is enabled: `GET /api/connectors/:id`
2. Check `respond_to` includes your connector ID
3. Look at AgentZero logs for dispatch errors
4. Test with a simple echo server first

### Payload is Empty/Malformed

1. Ensure `Content-Type: application/json` middleware is active
2. Parse request body before accessing fields
3. Log raw request body for debugging

---

## Reference: Sample Connectors

See the `sample_connectors/` directory for working examples:

- `webhook_receiver/` - Basic webhook server (Node.js & Python)
- `slack_notifier/` - Forwards responses to Slack channels
