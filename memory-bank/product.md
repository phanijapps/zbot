# Agent Zero - Product Definition

## Vision

Agent Zero is an AI agent platform with a web dashboard and CLI. Build specialized AI assistants with custom instructions, integrate with multiple LLM providers, extend capabilities through skills and MCP servers.

## Interfaces

### Web Dashboard
Browser-based interface for managing agents, providers, and conversations. Served by the daemon at `http://localhost:18791`.

### CLI (zero)
Command-line interface for scripting, automation, and terminal-based workflows.

## Target Users

- **Developers**: Building AI-powered workflows and automation
- **Power Users**: Creating specialized assistants for specific tasks
- **Teams**: Managing multiple AI agents with different capabilities

## Core Features

### 1. Chat Interface
Conversational interface with real-time streaming responses and tool execution visibility.

### 2. Agent Management
Create AI agents with custom instructions, provider/model selection, and capability configuration.

### 3. Provider Management
Multi-provider support: OpenAI, Anthropic, DeepSeek, Z.AI, Groq, Ollama, any OpenAI-compatible API. Set a default provider for the root agent.

### 4. Skill System
Reusable skills with frontmatter metadata and markdown instructions following the Agent Skills specification.

### 5. MCP Server Integration
Model Context Protocol servers for external tool access.

### 6. Scheduled Tasks
Cron-based scheduling for automated agent invocations.

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 19 + TypeScript + Vite |
| UI | Tailwind CSS v4 + Radix UI |
| Backend | Rust daemon (Axum + tokio) |
| Database | SQLite |
| API | HTTP REST + WebSocket |

## Architecture

```
┌────────────────┐     ┌────────────────┐
│  Web Browser   │     │      CLI       │
│  (Dashboard)   │     │    (zero)      │
└───────┬────────┘     └───────┬────────┘
        │                      │
        └──────────┬───────────┘
                   │
        ┌──────────┴──────────┐
        │    Daemon (zerod)   │
        │  HTTP :18791        │
        │  WebSocket :18790   │
        └──────────┬──────────┘
                   │
        ┌──────────┴──────────┐
        │  ~/Documents/       │
        │    agentzero/       │
        └─────────────────────┘
```

## Storage

**Data Directory**: `~/Documents/agentzero/`

```
agentzero/
├── agents/                 # Agent configs
├── agents_data/            # Per-agent workspace
├── skills/                 # Skill definitions
├── db/                     # SQLite databases
├── providers.json          # LLM providers
└── mcps.json               # MCP configs
```

## Key Differentiators

1. **Local-first**: Full data control, runs on your machine
2. **Multi-provider**: Not locked to single LLM vendor
3. **Extensible**: Skills + MCP servers for unlimited capabilities
4. **Open Standards**: Agent Skills and MCP specifications
5. **Simple deployment**: Single daemon binary + static web files
