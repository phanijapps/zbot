# AgentZero — Workspace Root

z-Bot is a multipurpose AI agent that lives on the desktop and connects to any OpenAI-compatible API.

## Workspace Layout

```
runtime/     — agent-primitives, agent-runtime, agent-tools (shared primitives + execution engine + built-in tools)
services/    — api-logs, daily-sessions, execution-state, knowledge-graph
stores/      — zbot-stores* persistence layer (traits, domain types, SQLite impl)
gateway/     — gateway-* sub-crates + gateway shell (HTTP/WS network layer)
discovery/   — LAN mDNS advertisement
apps/        — daemon (zbotd), cli (zbot), ui (React dashboard)
```

## Dependency Order (bottom → top)

```
agent-primitives
  ├── agent-tools
  └── agent-runtime
        └── gateway-execution

zbot-stores-domain (serde only)
  └── zbot-stores-traits
        └── zbot-stores
              └── zbot-stores-sqlite (SQLite + rusqlite + sqlite-vec)
                    └── zbot-stores-conformance (test harness)

services/* (execution-state, api-logs, knowledge-graph, daily-sessions)
runtime/* (agent-runtime, agent-tools)
gateway/* sub-crates
gateway (shell — wires everything together)
discovery
apps/daemon (zbotd binary)
apps/cli (zbot binary)
```

## Common Commands

```bash
cargo check --workspace              # Fast type-check all crates
cargo test --workspace               # Run all tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all

npm run daemon:watch   # Run daemon, hot-reload on code changes
npm run dev            # React UI on port 3000 (from apps/ui/)
```

## Key Ports

| Port  | Protocol  | Purpose              |
|-------|-----------|----------------------|
| 18791 | HTTP      | REST API + static UI |
| 18790 | WebSocket | Real-time streaming  |

## Data Directory

All apps default to `~/Documents/agentzero/`:

```
agentzero/
├── conversations.db      # SQLite (zbot-stores-sqlite)
├── config/               # SOUL.md, INSTRUCTIONS.md, OS.md, shards/
├── agents/{name}/        # Agent YAML configs
├── wards/                # Code project directories
├── skills/{name}/        # Skill markdown files
├── providers.json        # LLM provider configs
├── mcps.json             # MCP server configs
├── connectors.json       # External connectors
└── cron_jobs.json        # Scheduled tasks
```

Also see `CLAUDE.md` for behavioral guidelines and development patterns.
