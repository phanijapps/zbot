# Apps

Runnable applications built on the gateway.

## Binaries

### zerod (daemon)

Standalone HTTP/WebSocket server running the full agent platform.

```bash
# Run with dashboard
cargo run -p daemon -- --static-dir ./ui/dist

# Run on custom port
cargo run -p daemon -- --port 8080

# Run with config directory
cargo run -p daemon -- --config-dir ~/.agentzero
```

**CLI Options:**
- `--static-dir` - Path to static files (dashboard)
- `--port` - HTTP port (default: 18791)
- `--ws-port` - WebSocket port (default: 18790)
- `--config-dir` - Configuration directory

### zero-cli

Terminal UI client for interacting with agents.

```bash
# Connect to local daemon
cargo run -p zero-cli

# Connect to remote daemon
cargo run -p zero-cli -- --url http://remote:18791
```

**Features:**
- Rich TUI interface (ratatui)
- WebSocket streaming
- Conversation management
- Agent selection

## Data Directory

Both apps use `~/Documents/agentzero/` by default:

```
agentzero/
├── conversations.db      # SQLite database
├── agents/{name}/        # Agent configs
├── skills/{name}/        # Skill definitions
├── providers.json        # LLM providers
└── mcps.json             # MCP servers
```

## Development

```bash
# Run daemon with auto-reload
cargo watch -x 'run -p daemon -- --static-dir ./ui/dist'

# Build release
cargo build --release -p daemon -p zero-cli
```
