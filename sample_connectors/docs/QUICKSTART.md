# Quick Start: Building Your First Connector

This guide walks you through creating and testing a connector in 5 minutes.

## Prerequisites

- AgentZero daemon running (`zerod`)
- Node.js, Python, or any HTTP server

## Step 1: Create a Simple Webhook Server

Choose your preferred language:

### Node.js (Express)

```bash
cd sample_connectors/webhook_receiver
npm install
npm start
```

### Python (Flask)

```bash
cd sample_connectors/webhook_receiver
pip install -r requirements.txt
python app.py
```

Your webhook server is now running at `http://localhost:8080`.

## Step 2: Register the Connector

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

## Step 3: Test the Connector

```bash
curl -X POST http://localhost:18791/api/connectors/local-webhook/test
```

You should see output in your webhook server's console.

## Step 4: Create a Scheduled Task

Using the UI at `http://localhost:5173/hooks`, create a new scheduled task:

1. Name: "Test Schedule"
2. Schedule: Every minute (`* * * * *`)
3. Message: "Hello from scheduled task!"

Or via API:

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-schedule",
    "name": "Test Schedule",
    "schedule": "* * * * *",
    "agent_id": "root",
    "message": "Hello from scheduled task!",
    "respond_to": ["local-webhook"],
    "enabled": true
  }'
```

## Step 5: Watch It Work

Your webhook server will receive the agent's response every minute!

## Next Steps

- Check out the [Slack Notifier](../slack_notifier/) for a real-world example
- Read the [full specification](./CONNECTOR_SPEC.md)
- Explore connector management in the UI at `/connectors`

## Troubleshooting

### Connector not receiving requests?

1. Check connector is enabled: `GET /api/connectors/local-webhook`
2. Verify your server is running: `curl http://localhost:8080/health`
3. Check gateway logs for dispatch errors

### Request timing out?

Increase the timeout in connector config:
```json
{
  "transport": {
    "type": "http",
    "timeout_ms": 60000
  }
}
```
