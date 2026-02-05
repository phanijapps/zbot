# Slack Notifier Connector

Forwards AgentZero agent responses to Slack channels via incoming webhooks.

## Setup

### 1. Create Slack Webhook

1. Go to [Slack API](https://api.slack.com/apps)
2. Create a new app or select existing one
3. Enable "Incoming Webhooks"
4. Add a webhook to your workspace
5. Copy the webhook URL

### 2. Configure Environment

```bash
export SLACK_WEBHOOK_URL="https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX"
```

Optional environment variables:
- `PORT` - Server port (default: 8081)
- `SLACK_CHANNEL` - Override the default channel
- `SLACK_USERNAME` - Bot display name (default: "AgentZero")
- `SLACK_ICON_EMOJI` - Bot emoji (default: ":robot_face:")

### 3. Start Server

```bash
npm install
npm start
```

### 4. Register Connector

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "slack-notifier",
    "name": "Slack Notifier",
    "transport": {
      "type": "http",
      "callback_url": "http://localhost:8081/webhook",
      "method": "POST",
      "headers": {}
    },
    "enabled": true
  }'
```

### 5. Test

```bash
curl -X POST http://localhost:18791/api/connectors/slack-notifier/test
```

## Usage with Cron Jobs

Create a scheduled task that sends responses to Slack:

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-slack-report",
    "name": "Daily Slack Report",
    "schedule": "0 9 * * *",
    "agent_id": "root",
    "message": "Generate a summary of today'"'"'s activities",
    "respond_to": ["slack-notifier"],
    "enabled": true
  }'
```

## Message Format

The connector sends formatted Slack blocks:

```
┌─────────────────────────────────────┐
│ 🤖 Agent Response                   │
├─────────────────────────────────────┤
│ [Agent's response message here]     │
│                                     │
│ Source: cron | Session: sess-xxx    │
└─────────────────────────────────────┘
```

## Customization

Edit `server.js` to customize:
- Message formatting
- Add attachments
- Filter messages
- Add threading support
- Integrate with Slack Web API for more features

## Troubleshooting

### Messages not appearing?

1. Check `SLACK_WEBHOOK_URL` is set correctly
2. Verify webhook is active in Slack app settings
3. Check server logs for errors

### Rate limited?

Slack has rate limits. Consider:
- Adding message batching
- Using queue for high-volume scenarios
