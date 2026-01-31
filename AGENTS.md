# Agent Zero

AI agent platform with Web dashboard and CLI interfaces.

## Quick Reference

| Command | Purpose |
|---------|---------|
| `cd apps/ui && npm install` | Install frontend dependencies |
| `npm run dev` | Frontend dev server (port 3000) |
| `npm run build` | Build frontend to `dist/` |
| `npm run daemon` | Run daemon with cargo-watch |
| `cargo run -p daemon -- --static-dir ./dist` | Run daemon serving dashboard |
| `cargo check --workspace` | Verify Rust code |

## Architecture

```
agentzero/
├── framework/      # Core abstractions (zero-* crates)
├── runtime/        # Execution engine (agent-runtime, agent-tools)
├── services/       # Data services (logs, search, archive)
├── gateway/        # HTTP/WebSocket server
├── apps/           # Applications (daemon, cli, ui)
├── dist/           # Frontend build output
└── memory-bank/    # Documentation
```

## Layer Dependencies

```
apps/ → gateway/ → runtime/ → framework/
                 ↘ services/ ↗
```

Lower layers never import upper layers. Services are standalone.

## Layer Documentation

| Layer | Purpose | Docs |
|-------|---------|------|
| `framework/` | Core traits and abstractions | [framework/AGENTS.md](framework/AGENTS.md) |
| `runtime/` | Agent execution engine | [runtime/AGENTS.md](runtime/AGENTS.md) |
| `services/` | Standalone data services | [services/AGENTS.md](services/AGENTS.md) |
| `gateway/` | HTTP/WebSocket API | [gateway/AGENTS.md](gateway/AGENTS.md) |
| `apps/` | Runnable applications | [apps/AGENTS.md](apps/AGENTS.md) |

## Ports

| Port | Service |
|------|---------|
| 18791 | HTTP API + Web UI |
| 18790 | WebSocket (streaming) |
| 3000 | Vite dev server (development only) |

## Data Directory

All data stored in `~/Documents/agentzero/`:

```
agentzero/
├── conversations.db      # SQLite database
├── agents/{name}/        # Agent configs
├── skills/{name}/        # Skill definitions
├── providers.json        # LLM providers
└── mcps.json             # MCP configs
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
cargo run -p daemon -- --static-dir ./dist
# Access at http://localhost:18791
```
