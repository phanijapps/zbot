<p align="center">
  <img src="apps/ui/public/logo.svg" alt="z-Bot" width="280" />
</p>

<p align="center">
  <strong>Local-first AI agent platform</strong><br>
  Build, customize, and run AI assistants entirely on your machine.
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#license">License</a>
</p>

---

## Why z-Bot?

Most AI platforms lock you into their cloud, their pricing, and their rules. z-Bot is different:

- **Your data stays local** — Conversations, memory, and configs live on your machine
- **Any LLM provider** — OpenAI, Anthropic, DeepSeek, Groq, Ollama, or any OpenAI-compatible API
- **Fully extensible** — Add skills, tools, and MCP servers without touching core code
- **Open architecture** — Rust backend + React frontend, designed for hackability

## Features

| Feature | Description |
|---------|-------------|
| **Multi-Provider** | Switch between LLM providers without changing your agents |
| **Skills System** | Reusable instruction sets that extend agent capabilities |
| **MCP Integration** | Connect external tools via Model Context Protocol |
| **Agent Delegation** | Agents can spawn and coordinate with specialized subagents |
| **Persistent Memory** | Per-agent key-value storage for facts and preferences |
| **Real-time Streaming** | WebSocket-based event streaming for responsive UX |
| **Execution Logs** | Full visibility into agent reasoning and tool calls |

## Quick Start

### Prerequisites

- **Node.js 18+** and npm
- **Rust 1.88+** with cargo
- An LLM API key (OpenAI, Anthropic, etc.)

### Install & Run

```bash
# Clone the repository
git clone https://github.com/phanijapps/agentzero.git
cd agentzero

# Install frontend dependencies
cd apps/ui && npm install && cd ../..

# Development (two terminals)
npm run daemon    # Terminal 1: Backend with auto-reload
npm run dev       # Terminal 2: Frontend dev server

# Open http://localhost:3000
```

### Production Build

```bash
npm run build
cargo run -p daemon --release -- --static-dir ./dist

# Open http://localhost:18791
```

### First Run

1. Navigate to **Integrations** → Add your LLM provider
2. Click **Set as Default** on your preferred provider
3. Start chatting with the root agent

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                        CLIENTS                          │
│  ┌─────────────────┐           ┌─────────────────┐      │
│  │  Web Dashboard  │           │       CLI       │      │
│  │  (React + Vite) │           │     (ratatui)   │      │
│  └────────┬────────┘           └────────┬────────┘      │
└───────────┼─────────────────────────────┼───────────────┘
            │         HTTP / WebSocket    │
            └──────────────┬──────────────┘
                           │
┌──────────────────────────┴──────────────────────────────┐
│                     DAEMON (zerod)                      │
│  ┌────────────────────────────────────────────────────┐ │
│  │  HTTP API :18791  │  WebSocket :18790  │  Static   │ │
│  └────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────┐ │
│  │  Agent Runtime  │  Tool Registry  │  MCP Manager  │ │
│  └────────────────────────────────────────────────────┘ │
└──────────────────────────┬──────────────────────────────┘
                           │
┌──────────────────────────┴──────────────────────────────┐
│                      DATA LAYER                         │
│  ~/Documents/zbot/                                      │
│  ├── conversations.db     # SQLite database             │
│  ├── agents/{name}/       # Agent configurations        │
│  ├── skills/{name}/       # Skill definitions           │
│  ├── providers.json       # LLM provider configs        │
│  └── mcps.json            # MCP server configs          │
└─────────────────────────────────────────────────────────┘
```

## Commands

| Command | Description |
|---------|-------------|
| `npm run dev` | Start frontend dev server (port 3000) |
| `npm run build` | Build frontend to `dist/` |
| `npm run daemon` | Run daemon with auto-reload |
| `cargo run -p daemon` | Run daemon |
| `cargo run -p cli` | Run terminal UI client |
| `cargo check --workspace` | Type-check all Rust code |

## Ports

| Port | Service |
|------|---------|
| 18791 | HTTP API + Web UI |
| 18790 | WebSocket streaming |
| 3000 | Vite dev server (development only) |

## Documentation

| Document | Description |
|----------|-------------|
| [AGENTS.md](AGENTS.md) | Code organization and layer structure |
| [memory-bank/architecture.md](memory-bank/architecture.md) | Technical architecture details |
| [memory-bank/product.md](memory-bank/product.md) | Product vision and features |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19, TypeScript, Vite, Tailwind CSS v4 |
| Backend | Rust, Axum, tokio, SQLite |
| Protocol | HTTP REST, WebSocket, MCP |

## License

MIT

---

<p align="center">
  <sub>Built with Rust and React</sub>
</p>
