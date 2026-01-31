# Agent Zero

AI agent platform with Web dashboard and CLI interfaces.

## Quick Reference

| Command | Purpose |
|---------|---------|
| `npm install` | Install frontend dependencies |
| `npm run dev` | Frontend dev server (port 3000) |
| `npm run build` | Build frontend to `dist/` |
| `npm run daemon` | Run daemon with cargo-watch |
| `cargo run -p zerod -- --static-dir ./dist` | Run daemon serving built frontend |
| `cargo check --workspace` | Verify Rust code |
| `npx tsc --noEmit` | Verify TypeScript |

## Architecture

- **Frontend**: React 19 + TypeScript + Vite
- **Backend**: Rust daemon (gateway + agent runtime)
- **Database**: SQLite (conversations.db)
- **API**: HTTP REST + WebSocket streaming

See `memory-bank/` for detailed documentation:
- `product.md` - Product definition
- `architecture.md` - Technical architecture
- `plans/roadmap.md` - Development roadmap

## Key Directories

```
src/                          # Frontend (React)
├── features/
│   ├── agent/                # Chat + agent management
│   ├── skills/               # Skill management
│   ├── integrations/         # Provider management
│   ├── logs/                 # Execution logs dashboard
│   └── cron/                 # Scheduled tasks
├── services/transport/       # HTTP/WebSocket transport
└── shared/                   # UI components, types

crates/zero-*/                # Framework crates
application/
├── daemon/                   # Main daemon binary (zerod)
├── gateway/                  # HTTP + WebSocket server
├── agent-runtime/            # Agent executor
├── agent-tools/              # Built-in tools
├── api-logs/                 # Execution logging service
└── zero-cli/                 # CLI tool
```

## Data Directory

All data stored in `~/Documents/agentzero/`:

```
agentzero/
├── conversations.db          # SQLite database
├── agents/{name}/            # Agent configs
│   ├── config.yaml           # Metadata
│   └── AGENTS.md             # Instructions
├── agents_data/{id}/         # Per-agent data
│   └── memory.json           # Persistent memory
├── skills/{name}/
│   └── SKILL.md              # Skill instructions
├── providers.json            # LLM providers
└── mcps.json                 # MCP configs
```

## Running

**Development (2 terminals):**
```bash
# Terminal 1: Daemon with auto-reload
npm run daemon

# Terminal 2: Frontend with hot reload
npm run dev
```

**Production:**
```bash
npm run build
cargo run -p zerod -- --static-dir ./dist
# Access at http://localhost:18791
```

## Ports

| Port | Service |
|------|---------|
| 18791 | HTTP API + Web UI |
| 18790 | WebSocket (streaming) |
| 3000 | Vite dev server (development only) |

## Conventions

1. Instructions in `AGENTS.md` files, not `config.yaml`
2. Single data directory: `~/Documents/agentzero/`
3. Frontend generates invocation IDs before backend calls
4. All state persisted to SQLite or JSON files
