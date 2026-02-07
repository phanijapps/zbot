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
cargo run -p cli
```

## UI

Web dashboard. React 19 + TypeScript + Vite. See [ui/AGENTS.md](ui/AGENTS.md).

```bash
cd apps/ui && npm install && npm run dev
```

## Data Directory

All apps use `~/Documents/agentzero/` by default:

```
agentzero/
├── conversations.db      # SQLite database
├── INSTRUCTIONS.md       # Custom system prompt
├── agents/{name}/        # Agent configs
├── wards/                # Code Wards (persistent project dirs)
│   ├── .venv/            #   Shared Python venv
│   ├── scratch/          #   Default ward
│   └── {name}/           #   Agent-named projects
├── skills/{name}/        # Skill definitions
├── providers.json        # LLM providers
├── mcps.json             # MCP servers
├── connectors.json       # External connectors
└── cron_jobs.json        # Scheduled tasks
```
