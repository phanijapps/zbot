# Example Plugin

This is a reference implementation demonstrating the AgentZero plugin bridge protocol.

## Structure

```
plugins/.example/
├── plugin.json    # Plugin manifest (required)
├── package.json   # Node.js dependencies
├── index.js       # Entry point (required)
├── .config.json   # User config + secrets (auto-created)
└── README.md      # This file
```

## Plugin Manifest (plugin.json)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique plugin identifier (used as adapter_id) |
| `name` | string | Yes | Human-readable name |
| `version` | string | No | Semver version string |
| `description` | string | No | Plugin description |
| `entry` | string | No | Entry script (default: "index.js") |
| `enabled` | boolean | No | Whether to auto-start (default: true) |
| `env` | object | No | Environment variables (${VAR} references resolved) |
| `auto_restart` | boolean | No | Auto-restart on crash (default: true) |
| `restart_delay_ms` | number | No | Delay before restart (default: 5000) |

## Bridge Protocol

### From Plugin (stdout)

| Message | Description |
|---------|-------------|
| `hello` | Register with gateway (first message) |
| `pong` | Heartbeat response |
| `ack` | Confirm outbox item delivery |
| `fail` | Report delivery failure |
| `inbound` | Send message to trigger agent |
| `resource_response` | Respond to resource query |
| `capability_response` | Respond to capability invocation |

### To Plugin (stdin)

| Message | Description |
|---------|-------------|
| `hello_ack` | Registration confirmed |
| `ping` | Heartbeat check |
| `outbox_item` | Message to deliver externally |
| `resource_query` | Query a resource |
| `capability_invoke` | Invoke a capability |

## Testing

1. Copy this directory to your plugins folder:
   ```bash
   cp -r plugins/.example ~/Documents/agentzero/plugins/example-plugin
   ```

2. Start the AgentZero daemon

3. Verify the plugin is discovered:
   ```bash
   curl http://localhost:18791/api/plugins
   ```

4. Check connector list (plugin appears as a connector):
   ```bash
   curl http://localhost:18791/api/connectors
   ```

5. Query plugin resource:
   ```bash
   curl -X POST http://localhost:18791/api/connectors/example-plugin/query \
     -H "Content-Type: application/json" \
     -d '{"resource": "status"}'
   ```

6. Invoke plugin capability:
   ```bash
   curl -X POST http://localhost:18791/api/connectors/example-plugin/invoke \
     -H "Content-Type: application/json" \
     -d '{"capability": "echo", "payload": {"message": "Hello!"}}'
   ```

## Creating Your Own Plugin

1. Create a new directory in `~/Documents/agentzero/plugins/`
2. Add `plugin.json` with your plugin configuration
3. Add `package.json` with any npm dependencies
4. Add `index.js` implementing the bridge protocol
5. Restart the daemon or call `POST /api/plugins/discover`

## Environment Variables

You can specify environment variables in `plugin.json`:

```json
{
  "env": {
    "API_TOKEN": "${MY_API_TOKEN}",
    "DEBUG": "true"
  }
}
```

`${VAR_NAME}` references are resolved from the process environment when the plugin starts.

## User Configuration

Each plugin has a `.config.json` file auto-created in its directory when discovered. This file stores:

- `enabled`: Override plugin enabled state
- `settings`: Non-sensitive user settings
- `secrets`: Sensitive values (API tokens, passwords)

```json
{
  "enabled": true,
  "settings": {
    "default_channel": "#general"
  },
  "secrets": {
    "api_token": "secret123"
  }
}
```

### Setting Secrets via API

```bash
# Set a secret (key must match the env var name expected by your plugin code)
curl -X PUT http://localhost:18791/api/plugins/example-plugin/secrets/API_TOKEN \
  -H "Content-Type: application/json" \
  -d '{"value": "your-secret-token"}'

# List secrets (values are not returned)
curl http://localhost:18791/api/plugins/example-plugin/secrets
```

**Important:** Secret keys must match the environment variable names your plugin code expects. For example, if your plugin reads `process.env.API_TOKEN`, the secret key should be `API_TOKEN`.

Secrets override environment variables at runtime, so you can use `${VAR}` in `plugin.json` as a fallback and override via config.
