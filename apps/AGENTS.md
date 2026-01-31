# Apps

Runnable applications built on the gateway.

## Structure

```
apps/
├── daemon/     # HTTP/WebSocket server (zerod)
├── cli/        # Terminal UI client
└── ui/         # Web dashboard (React)
```

## Daemon

Standalone HTTP/WebSocket server running the full agent platform.

```bash
# Run with dashboard
cargo run -p daemon -- --static-dir ./dist

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

## CLI

Terminal UI client for interacting with agents.

```bash
# Connect to local daemon
cargo run -p cli

# Connect to remote daemon
cargo run -p cli -- --url http://remote:18791
```

**Features:**
- Rich TUI interface (ratatui)
- WebSocket streaming
- Conversation management
- Agent selection

## UI

Web dashboard for Agent Zero. React 19 + TypeScript + Vite.

```bash
# Install dependencies
cd apps/ui && npm install

# Start dev server (port 3000)
npm run dev

# Build for production (outputs to workspace dist/)
npm run build
```

See [ui/AGENTS.md](ui/AGENTS.md) for detailed frontend documentation.

## Data Directory

All apps use `~/Documents/agentzero/` by default:

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
cargo watch -x 'run -p daemon -- --static-dir ./dist'

# Build release
cargo build --release -p daemon -p cli
```
