# Agent Zero

AI agent platform with Web dashboard and CLI interfaces. Agents manage persistent code projects (wards), use tools, delegate to subagents, and learn across sessions.

## Quick Reference

| Command | Purpose |
|---------|---------|
| `cd apps/ui && npm install` | Install frontend dependencies |
| `npm run dev` | Frontend dev server (port 3000) |
| `npm run build` | Build frontend to `dist/` |
| `npm run daemon` | Run daemon with cargo-watch |
| `cargo run -p daemon -- --static-dir ./dist` | Run daemon serving dashboard |
| `cargo test --workspace` | Run all tests (~300+) |
| `cargo check --workspace` | Verify Rust code |

## Architecture

```
agentzero/
├── framework/      # Core abstractions (zero-* crates)
├── runtime/        # Execution engine (agent-runtime, agent-tools)
├── services/       # Data services (logs, search, state, archive)
├── gateway/        # HTTP/WebSocket server (10 sub-crates + thin shell)
├── apps/           # Applications (daemon, cli, ui)
├── dist/           # Frontend build output
└── memory-bank/    # Documentation and plans
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
| `framework/` | Core traits and abstractions (9 crates) | [framework/AGENTS.md](framework/AGENTS.md) |
| `runtime/` | Agent executor, LLM client, tools | [runtime/AGENTS.md](runtime/AGENTS.md) |
| `services/` | Execution state, logs, search, archive | [services/AGENTS.md](services/AGENTS.md) |
| `gateway/` | HTTP/WS APIs, execution engine, events (10 sub-crates) | [gateway/AGENTS.md](gateway/AGENTS.md) |
| `apps/` | Daemon, CLI, Web UI | [apps/AGENTS.md](apps/AGENTS.md) |

## Key Concepts

| Concept | Description |
|---------|-------------|
| **Session** | Top-level user work session, groups multiple executions |
| **Execution** | Single agent turn (root or delegated subagent) |
| **Ward** | Agent-managed persistent project directory |
| **Skill** | Reusable instruction package (SKILL.md) |
| **MCP** | Model Context Protocol — external tool servers |
| **Delegation** | Root agent spawning subagents for tasks |
| **Continuation** | Root auto-resuming after all delegations complete |

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
├── conversations.db      # SQLite database (WAL mode, r2d2 pool)
├── INSTRUCTIONS.md       # Custom system prompt (auto-created)
├── agents/{name}/        # Agent configs (config.yaml + AGENTS.md)
├── wards/                # Code Wards (persistent project directories)
│   ├── .venv/            #   Shared Python venv
│   ├── scratch/          #   Default ward for quick tasks
│   └── {ward-name}/      #   Agent-named projects
├── skills/{name}/        # Skill definitions (SKILL.md)
├── providers.json        # LLM providers
├── mcps.json             # MCP configs
├── connectors.json       # External service connectors
└── cron_jobs.json        # Scheduled tasks
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
