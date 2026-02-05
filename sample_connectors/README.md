# AgentZero Sample Connectors

This directory contains documentation and sample implementations for building AgentZero connectors.

## What are Connectors?

Connectors are external services that receive agent responses. When an agent execution completes, AgentZero can dispatch the response to configured connectors via HTTP webhooks, CLI commands, or other transports.

## Directory Structure

```
sample_connectors/
├── docs/
│   ├── CONNECTOR_SPEC.md   # Full connector specification
│   └── QUICKSTART.md       # 5-minute getting started guide
├── webhook_receiver/       # Basic webhook server (Node.js + Python)
│   ├── server.js           # Node.js implementation
│   ├── app.py              # Python implementation
│   └── README.md
└── slack_notifier/         # Slack integration example
    ├── server.js
    └── README.md
```

## Quick Start

1. **Read the spec**: Start with [docs/CONNECTOR_SPEC.md](docs/CONNECTOR_SPEC.md)
2. **Try a sample**: Run `webhook_receiver` to see connectors in action
3. **Build your own**: Use the samples as templates

## Sample Connectors

### 1. Webhook Receiver

A minimal HTTP server that receives and logs webhook callbacks. Great for testing and as a template.

```bash
cd webhook_receiver
npm install && npm start  # Node.js
# or
pip install -r requirements.txt && python app.py  # Python
```

### 2. Slack Notifier

Posts agent responses to Slack channels via incoming webhooks.

```bash
cd slack_notifier
export SLACK_WEBHOOK_URL="https://hooks.slack.com/..."
npm install && npm start
```

## Connector Workflow

```
1. Register Connector
   POST /api/connectors
   ↓
2. Create Scheduled Task (or trigger via API)
   POST /api/cron or POST /api/sessions
   with respond_to: ["your-connector-id"]
   ↓
3. Agent Executes
   ↓
4. AgentZero Dispatches Response
   POST to your connector's callback_url
   ↓
5. Your Connector Processes Response
   (send to Slack, store in DB, trigger workflow, etc.)
```

## Creating Your Own Connector

1. **Create an HTTP server** with a POST endpoint
2. **Accept the standard payload**:
   ```json
   {
     "action": "respond",
     "message": "Agent response text",
     "context": { "session_id": "...", "source": "cron", ... }
   }
   ```
3. **Return success/failure**:
   ```json
   { "success": true, "message": "Processed" }
   ```
4. **Register with AgentZero** via API or UI

## Documentation

- [Connector Specification](docs/CONNECTOR_SPEC.md) - Full technical specification
- [Quick Start Guide](docs/QUICKSTART.md) - Get started in 5 minutes

## Support

For questions or issues, check the main AgentZero repository.
