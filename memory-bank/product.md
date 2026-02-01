# Agent Zero — Product Definition

## Vision

Agent Zero is a local-first AI agent platform that puts you in control. Run sophisticated AI assistants on your own machine with full data ownership, multi-provider flexibility, and unlimited extensibility through skills and MCP servers.

## Core Principles

1. **Local-First** — Your data stays on your machine. No cloud lock-in.
2. **Provider-Agnostic** — Use any LLM: OpenAI, Anthropic, DeepSeek, Ollama, self-hosted.
3. **Extensible** — Skills and MCP servers let you add any capability.
4. **Open Standards** — Built on Agent Skills and Model Context Protocol specifications.
5. **Simple Deployment** — Single daemon binary + static web files.

## Interfaces

### Web Dashboard
Browser-based interface served by the daemon at `http://localhost:18791`. Full-featured management of agents, providers, skills, and conversations.

### CLI (zero)
Command-line interface for scripting, automation, and terminal-based workflows. Connects to the same daemon as the web dashboard.

## Target Users

| User | Use Case |
|------|----------|
| **Developers** | Building AI-powered workflows, automation, code assistants |
| **Power Users** | Creating specialized assistants for research, writing, analysis |
| **Teams** | Managing multiple agents with different capabilities and contexts |
| **Privacy-Conscious** | Running AI locally without sending data to third parties |

## Core Features

### 1. Chat Interface
Conversational interface with real-time streaming. See tool calls as they happen. Continue conversations across sessions with full history preserved in SQLite.

### 2. Agent Management
Create agents with:
- Custom system instructions (AGENTS.md files)
- Provider and model selection
- Temperature and token limits
- Skill and MCP server assignments

### 3. Provider Management
Supported providers:
- OpenAI (GPT-4, GPT-4o, etc.)
- Anthropic (Claude 3, Claude 3.5)
- DeepSeek
- Groq
- Ollama (local models)
- Any OpenAI-compatible API

Set a default provider for the root agent. Per-agent provider overrides supported.

### 4. Skill System
Reusable instruction packages following the Agent Skills specification:

```markdown
---
name: code-review
description: Reviews code for quality and bugs
category: development
---

# Code Review Skill

When reviewing code:
1. Check for security vulnerabilities
2. Identify performance issues
3. Suggest improvements
...
```

Skills are stored in `~/Documents/agentzero/skills/{name}/SKILL.md`.

### 5. MCP Server Integration
Connect to external tools via Model Context Protocol servers. Configure in `mcps.json`:

```json
{
  "servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-filesystem"]
    }
  }
}
```

### 6. Persistent Memory
Per-agent key-value storage for facts, preferences, and context:

```
memory set user_name "Alice"
memory get user_name
memory search preferences
```

Stored in `agents_data/{agent_id}/memory.json`.

### 7. Operations Dashboard
Real-time monitoring and management of agent sessions:

**Statistics Panel:**
- Active sessions count (running, queued)
- Completed/crashed session counts
- Sessions by trigger source (web, cli, api, cron, plugin)

**Session List:**
- All sessions with status indicators
- Execution hierarchy (root agent + subagents)
- Turn counts and timing information
- Filter by source and status
- Auto-refresh every 5 seconds

**Session Management:**
- View session details and execution tree
- Cancel running sessions
- Track subagent delegation in real-time

### 8. Multi-Turn Session Management
Conversations persist across multiple turns within a session:

- **Session Continuity**: Multiple messages share the same session until `/new`
- **Context Preservation**: Full conversation history maintained per session
- **Session Reset**: `/new` command starts fresh session
- **Source Tracking**: Sessions tagged with origin (web, cli, api, etc.)

### 9. Scheduled Tasks (Planned)
Cron-based scheduling for automated agent invocations. Define recurring tasks that run agents on a schedule.

## Technology Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19 + TypeScript + Vite |
| UI | Tailwind CSS v4 + Radix UI |
| Backend | Rust (Axum + tokio) |
| Database | SQLite (rusqlite) |
| API | HTTP REST + WebSocket |

## Data Model

### Conversations
Persisted to SQLite with full message history:
- Conversation metadata (agent, timestamps)
- Messages (role, content, tool calls)
- Automatic context loading (last 50 messages)

### Agent Memory
JSON-based key-value store per agent:
- Store facts: `memory set fact_key "value"`
- Tag-based organization
- Full-text search across values

### Agent Configuration
File-based configuration:
- `config.yaml` — Model, provider, temperature
- `AGENTS.md` — System instructions (markdown)

## Differentiators

| Feature | Agent Zero | Cloud AI Platforms |
|---------|------------|-------------------|
| Data Location | Local machine | Cloud servers |
| Provider Lock-in | None | Usually locked |
| Offline Capable | Yes (with Ollama) | No |
| Cost | API costs only | Subscription + API |
| Customization | Unlimited | Limited |
| Privacy | Full control | Varies |

## Roadmap Highlights

1. **v0.1** — Core chat, providers, skills ✓
2. **v0.2** — Persistent memory, SQLite conversations ✓
3. **v0.3** — MCP integration, CLI improvements ✓
4. **v0.4** — Operations Dashboard, Session Management ✓
   - Real-time session monitoring
   - Multi-turn conversation support
   - Trigger source tracking
   - Subagent delegation visibility
   - Comprehensive test suite (290+ tests)
5. **v0.5** — Scheduled tasks, multi-agent workflows
6. **v1.0** — Stable API, documentation, packaging
