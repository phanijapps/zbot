# AgentZero — Workspace Root

z-Bot is a multipurpose AI agent that lives on the desktop and connects to any OpenAI-compatible API.

## Workspace Layout

```
framework/   — zero-* Rust library crates (publishable as zero-agent-framework)
runtime/     — agent-runtime, agent-tools (execution engine + built-in tools)
services/    — api-logs, daily-sessions, execution-state, knowledge-graph
stores/      — zero-stores* persistence layer (traits, domain types, SQLite impl)
gateway/     — gateway-* sub-crates + gateway shell (HTTP/WS network layer)
discovery/   — LAN mDNS advertisement
apps/        — daemon (zbotd), cli (zbot), ui (React dashboard)
```

## Dependency Order (bottom → top)

```
zero-core
  ├── zero-llm, zero-tool, zero-mcp, zero-session, zero-prompt
  └── zero-middleware (re-exports agent-runtime::middleware)
        └── zero-agent
              └── zero-app (aggregator prelude)

zero-stores-domain (serde only)
  └── zero-stores-traits
        └── zero-stores
              └── zero-stores-sqlite (SQLite + rusqlite + sqlite-vec)
                    └── zero-stores-conformance (test harness)

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
├── conversations.db      # SQLite (gateway-database / zero-stores-sqlite)
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
