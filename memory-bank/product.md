# z-Bot — Product Definition

## Interfaces

### Web Dashboard
Browser-based interface served by the daemon at `http://localhost:18791`. Full-featured management of agents, providers, skills, and conversations.

### CLI (zero)
Command-line interface for scripting, automation, and terminal-based workflows. Connects to the same daemon as the web dashboard.

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

Skills are stored in `~/Documents/zbot/skills/{name}/SKILL.md`.

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

### 6. Persistent Memory & Learning

Hybrid memory system combining key-value storage, semantic search, and automatic session distillation.

**Manual Memory** (key-value store):
```
memory set user_name "Alice"           # agent-scoped
memory get user_name
memory search preferences
memory set scope=ward key=purpose ...  # ward-scoped
```

**Structured Facts** (automatic + manual):
```
memory save_fact category=preference key=user.language content="User prefers Rust"
memory recall query="language preferences" limit=5
memory graph query="zbot"              # knowledge graph entities
```

**Tiers**: Global shared → Agent → Ward → Session (ephemeral)

**Memory Evolution Features**:
- **Hybrid Search**: FTS5 BM25 keyword search (30%) + cosine vector similarity (70%) with confidence, recency, and mention-count boosting
- **Embedding Providers**: Configurable backends — local ONNX (fastembed, default, zero cost), OpenAI-compatible APIs (Ollama, OpenAI, Voyage)
- **Embedding Cache**: SHA-256 hash-based dedup prevents re-embedding unchanged content
- **Session Distillation**: After sessions complete, an LLM automatically extracts durable facts (preferences, decisions, patterns, entities, instructions, corrections) and stores them with embeddings
- **Smart Recall**: At session start, relevant facts are retrieved via hybrid search and injected as context
- **Pre-Compaction Flush**: Before context window trimming, the agent gets a warning turn to save important facts
- **Knowledge Graph**: Distillation extracts entities and relationships (person, project, tool, concept, file) into a graph store for structured queries
- **Fact Dedup**: UNIQUE constraint on (agent_id, scope, key) — repeated mentions update content and bump mention_count

### 7. Code Wards
Agent-managed persistent project directories. The agent autonomously creates, names, and navigates wards — code persists across sessions.

- `ward(action="use", name="stock-tracker")` — switch to a ward
- `ward(action="list")` — see all wards with descriptions
- `ward(action="create", name="my-app")` — create a new project
- Shared Python venv across all wards
- Per-ward node_modules (Node convention)
- Ward memory for project context (tech stack, build commands)
- `scratch` ward for quick one-off tasks

### 8. Operations Dashboard
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

### 9. Multi-Turn Session Management
Conversations persist across multiple turns within a session:

- **Session Continuity**: Multiple messages share the same session until `/new`
- **Context Preservation**: Full conversation history maintained per session
- **Session Reset**: `/new` command starts fresh session
- **Source Tracking**: Sessions tagged with origin (web, cli, api, etc.)

### 10. Scheduled Tasks (Planned)
Cron-based scheduling for automated agent invocations. Define recurring tasks that run agents on a schedule.

## Technology Stack

| Layer | Technology |
|-------|------------|
| Frontend | React 19 + TypeScript + Vite |
| UI | Tailwind CSS v4 + Radix UI |
| Backend | Rust (Axum + tokio) |
| Database | SQLite (rusqlite + r2d2 pool, WAL mode) |
| API | HTTP REST + WebSocket |

## Roadmap

### Completed
1. **v0.1** — Core chat, providers, skills
2. **v0.2** — Persistent memory, SQLite conversations
3. **v0.3** — MCP integration, CLI improvements
4. **v0.4** — Operations Dashboard, Session Management
   - Real-time session monitoring
   - Multi-turn conversation support
   - Trigger source tracking
   - Subagent delegation visibility
5. **v0.5** — Responsive Architecture + Code Wards
   - Real streaming (no simulated delays)
   - SQLite WAL mode + r2d2 connection pool
   - Batch DB writes, RwLock caching
   - Parallel tool execution + output truncation
   - Gateway crate decomposition (13 crates)
   - Code Wards (persistent project directories)
   - Shell-first execution + apply_patch
   - Session Tree (continuous conversation, subagent isolation)
   - Goal-oriented execution (scoring, stuck-detection, safety valve)
   - 300+ tests across all crates
6. **v0.6** — Memory Evolution
   - Embedding providers: local fastembed (default) + OpenAI-compatible
   - Hybrid search: FTS5 BM25 + vector cosine similarity
   - Session distillation: auto-extract facts from completed sessions
   - Smart recall: inject relevant facts at session start
   - Pre-compaction memory flush: save facts before context trim
   - Knowledge graph integration: entity/relationship extraction during distillation
   - Old standalone knowledge_graph tools removed (5 tools → unified `graph` action in memory tool)
   - 620+ tests across all crates

### Planned
7. **v0.7** — Creative Hub + Lifecycle
   - Code Wards Phase 4: cross-ward code discovery, pattern learning
   - Skill loading & unloading lifecycle (TTL, LRU, dependencies)
8. **v0.8** — Context & Safety
   - Context window management (auto-compaction, token counting)
   - Agent sandbox (process isolation)
   - Concurrent access: SQLite for shared state, inter-agent message queue
9. **v1.0** — Stable API, documentation, packaging

### Stretch Goals (Memory)
- **Contradiction Detection**: New fact conflicts with existing (same key, different content) — flag for resolution
- **Confidence Decay**: Nightly decay of unmaintained facts. Below 0.3 → archived. Prevents stale memory buildup
- **Cross-Agent Gossip**: Agent A saves shared fact → all agents see it at next session start. Collective intelligence
- **Memory Compression**: When a key has 5+ updates, LLM merges them into one consolidated fact
- **Dream Mode**: During idle time, agent reviews its own memory, finds connections, generates new insights. Runs as cron job
- **Memory Diff**: Show user what the agent learned this session: "I learned 3 new facts: [...]". Transparency
