# Agent Zero

A local-first AI agent platform with web dashboard and CLI.

Build specialized AI assistants with custom instructions, integrate multiple LLM providers, and extend capabilities through skills and MCP servers—all running on your machine.

## Features

- **Multi-Provider Support** — OpenAI, Anthropic, DeepSeek, Groq, Ollama, any OpenAI-compatible API
- **Extensible Skills** — Reusable instruction sets following the Agent Skills specification
- **MCP Integration** — Connect external tools via Model Context Protocol servers
- **Persistent Memory** — Per-agent key-value storage for facts and preferences
- **Real-time Streaming** — WebSocket-based event streaming for responsive UX
- **Local Data** — SQLite conversations, file-based configs, full data ownership

## Quick Start

### Prerequisites

- Node.js 18+ and npm
- Rust 1.75+ with cargo
- An LLM API key (OpenAI, Anthropic, etc.)

### Installation

```bash
git clone https://github.com/phanijapps/agentzero.git
cd agentzero
npm install
```

### Running

**Development (two terminals):**

```bash
# Terminal 1: Daemon with auto-reload
npm run daemon

# Terminal 2: Frontend dev server
npm run dev
```

Open http://localhost:3000 (proxies to daemon).

**Production:**

```bash
npm run build
cargo run -p zerod -- --static-dir ./dist
```

Open http://localhost:18791.

### First Run

1. Go to **Integrations** → Add your LLM provider (OpenAI, Anthropic, etc.)
2. Click **Set as Default** on your preferred provider
3. Start chatting with the root agent

## Architecture

```
┌────────────────┐     ┌────────────────┐
│  Web Browser   │     │      CLI       │
│   (React)      │     │    (zero)      │
└───────┬────────┘     └───────┬────────┘
        │ HTTP/WS              │ HTTP/WS
        └──────────┬───────────┘
                   │
        ┌──────────┴──────────┐
        │   Daemon (zerod)    │
        │  HTTP API  :18791   │
        │  WebSocket :18790   │
        └──────────┬──────────┘
                   │
        ┌──────────┴──────────┐
        │ ~/Documents/agentzero│
        │  SQLite + Files     │
        └─────────────────────┘
```

## Project Structure

```
agentzero/
├── src/                          # Frontend (React 19 + TypeScript)
│   ├── features/
│   │   ├── agent/                # Chat interface
│   │   ├── skills/               # Skill management
│   │   ├── integrations/         # Provider config
│   │   └── cron/                 # Scheduled tasks
│   ├── services/transport/       # HTTP/WebSocket client
│   └── shared/                   # UI components, types
├── crates/                       # Zero Framework
│   ├── zero-core/                # Core traits
│   ├── zero-agent/               # Agent implementations
│   ├── zero-session/             # Session management
│   └── zero-mcp/                 # MCP protocol
├── application/                  # Application crates
│   ├── daemon/                   # Main binary (zerod)
│   ├── gateway/                  # HTTP + WebSocket server
│   ├── agent-runtime/            # Agent executor
│   ├── agent-tools/              # Built-in tools
│   └── zero-cli/                 # CLI tool
└── memory-bank/                  # Documentation
```

## Data Directory

All data stored in `~/Documents/agentzero/`:

```
agentzero/
├── conversations.db              # Conversations & messages
├── agents/{name}/
│   ├── config.yaml               # Model, temperature, etc.
│   └── AGENTS.md                 # System instructions
├── agents_data/{id}/
│   └── memory.json               # Persistent memory
├── skills/{name}/
│   └── SKILL.md                  # Skill instructions
├── providers.json                # LLM providers
└── mcps.json                     # MCP configs
```

## Commands

| Command | Description |
|---------|-------------|
| `npm install` | Install frontend dependencies |
| `npm run dev` | Vite dev server (port 3000) |
| `npm run build` | Build frontend to `dist/` |
| `npm run daemon` | Run daemon with auto-reload |
| `cargo run -p zerod` | Run daemon |
| `cargo check --workspace` | Type-check Rust |
| `npx tsc --noEmit` | Type-check TypeScript |

## Ports

| Port | Service |
|------|---------|
| 18791 | HTTP API (+ static files in production) |
| 18790 | WebSocket streaming |
| 3000 | Vite dev server (development) |

## Documentation

- [Product Definition](memory-bank/product.md) — Vision, features, users
- [Architecture](memory-bank/architecture.md) — Technical design
- [Roadmap](memory-bank/plans/roadmap.md) — Development phases

## License

MIT
