# Slack Plugin

Two-way Slack integration for AgentZero. Receives messages from channels and responds via AI agents.

## Features

- **Inbound**: Forward Slack messages to AgentZero agents
- **Outbound**: Send messages, reactions, and ephemeral responses
- **Threading**: Maintains conversation threads in Slack
- **Socket Mode**: No public endpoint required

## Setup

### 1. Create Slack App

1. Go to https://api.slack.com/apps
2. Click "Create New App" → "From scratch"
3. Name it (e.g., "AgentZero Bot") and select your workspace

### 2. Enable Socket Mode

1. Go to "Socket Mode" in left sidebar
2. Enable Socket Mode
3. Generate an App-Level Token with `connections:write` scope
4. Copy the token (starts with `xapp-`)

### 3. Configure OAuth Scopes

Go to "OAuth & Permissions" and add these Bot Token Scopes:

| Scope | Description |
|-------|-------------|
| `chat:write` | Send messages |
| `channels:history` | Read channel messages |
| `groups:history` | Read private channel messages |
| `im:history` | Read direct messages |
| `mpim:history` | Read group DMs |
| `channels:read` | List channels |
| `users:read` | List users |
| `reactions:write` | Add emoji reactions |

### 4. Subscribe to Events

Go to "Event Subscriptions" → "Subscribe to bot events":

| Event | Description |
|-------|-------------|
| `message.channels` | Messages in public channels |
| `message.groups` | Messages in private channels |
| `message.im` | Direct messages |
| `app_mention` | When bot is @mentioned |

### 5. Install App

1. Go to "Install App" in sidebar
2. Click "Install to Workspace"
3. Copy the Bot User OAuth Token (starts with `xoxb-`)

### 6. Configure Plugin

The plugin auto-creates a `.config.json` file on first discovery. Set secrets via API:

```bash
# Set the bot token (key must match env var name expected by plugin)
curl -X PUT http://localhost:18791/api/plugins/slack/secrets/SLACK_BOT_TOKEN \
  -H "Content-Type: application/json" \
  -d '{"value": "xoxb-your-bot-token"}'

# Set the app token
curl -X PUT http://localhost:18791/api/plugins/slack/secrets/SLACK_APP_TOKEN \
  -H "Content-Type: application/json" \
  -d '{"value": "xapp-your-app-token"}'
```

Or edit the config file directly at `~/Documents/agentzero/plugins/slack/.config.json`:

```json
{
  "enabled": true,
  "secrets": {
    "SLACK_BOT_TOKEN": "xoxb-your-bot-token",
    "SLACK_APP_TOKEN": "xapp-your-app-token"
  }
}
```

**Note:** Secret keys must match the environment variable names expected by the plugin code (`SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN`).

### 7. Restart AgentZero

The daemon will auto-discover and start the Slack plugin.

## Usage

### Direct Messages

DM the bot directly - it will respond using the configured agent.

### Channel Messages

In channels, the bot responds when:
- Mentioned with `@BotName`
- In a thread where it was previously mentioned

### Capabilities

The plugin exposes these capabilities to agents:

| Capability | Description |
|------------|-------------|
| `send_message` | Send message to channel/user |
| `send_ephemeral` | Send visible-only-to-user message |
| `add_reaction` | Add emoji reaction to message |

### Resources

| Resource | Description |
|----------|-------------|
| `channels` | List channels bot is in |
| `users` | List workspace users |
| `team` | Get workspace info |

## Example: Send Message from Agent

Agents can use the connector to send messages:

```json
{
  "capability": "send_message",
  "payload": {
    "channel": "C1234567890",
    "text": "Hello from AgentZero!",
    "thread_ts": "1234567890.123456"
  }
}
```

## Troubleshooting

### Plugin shows as "Failed"

1. Check logs: `tail -f ~/Documents/agentzero/logs/daemon.log`
2. Verify tokens are set in config:
   ```bash
   cat ~/Documents/agentzero/plugins/slack/.config.json
   ```
3. Run manually to see errors:
   ```bash
   cd ~/Documents/agentzero/plugins/slack
   SLACK_BOT_TOKEN="xoxb-..." SLACK_APP_TOKEN="xapp-..." node index.js
   ```

### Bot not responding in channels

1. Ensure bot is invited to the channel: `/invite @BotName`
2. Check if `channels:history` scope is granted
3. Verify events are subscribed

### Socket Mode connection fails

1. Verify App-Level Token has `connections:write` scope
2. Check if Socket Mode is enabled in app settings
3. Ensure network allows WebSocket connections
