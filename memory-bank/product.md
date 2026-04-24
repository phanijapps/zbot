# z-Bot — Product Definition

z-Bot is a goal-oriented AI agent that lives on your desktop. It analyzes intent, plans autonomously, delegates to specialist subagents, self-learns across sessions, and works with any OpenAI-compatible provider. It is token-intensive by design — the agent does the work so you don't have to.

## Interfaces

### Web Dashboard
Browser-based interface served by the daemon at `http://localhost:18791`. Full-featured management of agents, providers, skills, conversations, and observability.

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

### 6. Memory Brain — Persistent Intelligence

The memory layer is z-Bot's cognitive system. Every session teaches it something. Agents learn from experience, avoid past mistakes, and reuse existing work. See [components/memory-layer/overview.md](components/memory-layer/overview.md) for full architecture.

**Five Memory Loops** (all active):
1. **System recall** — facts, episodes, graph entities injected as system message on every first message
2. **Intent + memory** — corrections and strategies enriches intent analysis before planning
3. **Subagent priming** — corrections, skills, ward context injected when spawning subagents
4. **Mid-session refresh** — new relevant facts injected every N turns during long sessions
5. **Post-session distillation** — LLM extracts facts, entities, relationships, episodes with verification

**Agent Tools**: All agents (root + subagents) have WardTool, MemoryTool, GrepTool — they can enter wards, recall memory, and search code.

**Recall Output** (priority order):
- Rules (corrections — ALWAYS followed)
- Warnings (past failures — avoid these)
- Preferences & instructions
- Past experiences (with outcome + strategy)
- Domain knowledge

**Accuracy Layer**:
- Fact verification: distilled facts grounded against tool outputs (confidence scaled by match ratio)
- Fact dedup: 60% word overlap check prevents near-duplicates under different keys
- Entity normalization: file basename matching, alias tracking
- Relationship dedup: unique index on (source, target, type)

**Policies**: High-priority rules injected as memory facts (correction category, confidence 1.0, global scope). Surface at top of every recall. Currently via SQL; UI planned.

**Ward Knowledge** (auto-generated after each session):
- `ward.md` — curated rules only (max 5 corrections, 3 strategies, 2 warnings, deduped)
- `core_docs.md` — all code files with function signatures (scans entire ward recursively)
- `structure.md` — directory tree

**Storage**: SQLite (memory_facts, episodes, recall_log, embeddings) + Knowledge Graph (entities, relationships)
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

### 8. Observability Dashboard
Full execution visibility via a List + Detail split layout:

**Session List (left panel):**
- Filterable list of root sessions with status badges
- Agent count, duration, token usage per session
- Real-time polling for running sessions

**Timeline Tree (right panel):**
- Hierarchical narrative: root → subagent → tool calls
- Contextual icons per tool type (Terminal, FileEdit, Brain, Globe, etc.)
- Click to expand nodes and see full arguments/results
- Subagent delegations collapsible with task description
- Error nodes highlighted in red

**Operations Dashboard:**
- Real-time session monitoring and management
- Pause, resume, cancel running sessions
- Resume crashed sessions at the subagent level (smart resume)

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
   - Shell-first execution with dedicated write_file / edit_file tools
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
