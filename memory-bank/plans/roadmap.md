# Agent Zero — Development Roadmap

## Current Status

**Version:** 0.3.0-dev
**Branch:** v1
**Status:** Core functionality stable, focusing on robustness

### Recently Completed

- [x] Crate reorganization (layered architecture)
- [x] UI moved under apps/
- [x] Skill caching and preloading
- [x] Agent delegation workflow in system prompt
- [x] MCP server integration
- [x] Execution logs with tree view
- [x] Compact session status rows with drill-down

### In Progress

- [ ] **Execution State Management** — [Plan](execution-state-management.md)
- [ ] **Logs Dashboard V2** — [Plan](logs-dashboard-v2.md)

---

## Architecture

```
agentzero/
├── framework/      # Core abstractions (zero-* crates)
├── runtime/        # Execution engine
├── services/       # Standalone data services
├── gateway/        # HTTP/WebSocket server
├── apps/           # Applications (daemon, cli, ui)
└── dist/           # Frontend build output
```

---

## Phase 1: Foundation ✓

**Goal:** Stable core with single-agent conversations

- [x] Gateway with HTTP/WebSocket APIs
- [x] Agent executor with LLM loop
- [x] Tool execution with streaming events
- [x] Conversation persistence (SQLite)
- [x] Multi-provider support
- [x] Built-in tools (file, shell, memory)
- [x] Web dashboard with streaming chat

---

## Phase 2: Integration ✓

**Goal:** External tools and enhanced UX

- [x] MCP manager integration
- [x] Tool discovery from MCP servers
- [x] Skill system with caching
- [x] Agent delegation workflow
- [x] Execution logs tree view

---

## Phase 3: Robustness (Current)

**Goal:** Production-grade reliability

### 3.1 Execution State Management
- [ ] Session status tracking (Queued/Running/Paused/Crashed/Completed)
- [ ] Crash recovery on daemon restart
- [ ] Pause/Resume/Cancel commands
- [ ] Checkpoint saving during execution
- [ ] Subagent cascade (pause parent = pause children)

**Plan:** [execution-state-management.md](execution-state-management.md)

### 3.2 Logs Dashboard V2
- [ ] Dedicated `/logs` monitoring page
- [ ] Timeline view (chronological activity stream)
- [ ] Tree view (delegation hierarchy)
- [ ] Table view (sortable, filterable)
- [ ] Real-time live tailing
- [ ] Session detail panel with actions

**Plan:** [logs-dashboard-v2.md](logs-dashboard-v2.md)

### 3.3 Error Handling
- [ ] Graceful degradation
- [ ] Retry with backoff
- [ ] User-friendly error messages
- [ ] Error reporting/logging

---

## Phase 4: Multi-Agent

**Goal:** Orchestrated multi-agent workflows

### 4.1 Orchestrator Patterns
- [ ] Sequential execution
- [ ] Parallel fan-out
- [ ] Conditional routing
- [ ] Loop with termination

### 4.2 Shared State
- [ ] State passing between agents
- [ ] State snapshots
- [ ] Rollback capability

---

## Phase 5: Production

**Goal:** Ready for real-world deployment

### 5.1 Scheduled Tasks
- [ ] Cron-based scheduling
- [ ] Task history
- [ ] Retry on failure

### 5.2 Security
- [ ] Tool permission enforcement
- [ ] Sandbox for shell commands
- [ ] Rate limiting
- [ ] Audit logging

### 5.3 Distribution
- [ ] Single binary release
- [ ] Docker image
- [ ] Cross-platform installers

---

## Milestones

| Milestone | Target | Status |
|-----------|--------|--------|
| v0.1.0 | Core chat working | ✓ |
| v0.2.0 | SQLite + Memory | ✓ |
| v0.3.0 | MCP + Logs | ✓ |
| v0.4.0 | Execution State Management | In Progress |
| v0.5.0 | Logs Dashboard V2 | Planned |
| v0.6.0 | Multi-agent orchestration | Planned |
| v1.0.0 | Production ready | Planned |

---

## Architecture Decisions Log

### ADR-001: Remove Tauri
**Date:** 2025-01-29
**Decision:** Replace Tauri desktop app with web dashboard + daemon
**Rationale:** Simpler deployment, better browser capabilities, easier debugging

### ADR-002: SQLite for Conversations
**Date:** 2025-01-29
**Decision:** Use SQLite instead of in-memory storage
**Rationale:** Persistence, ACID transactions, query capability

### ADR-003: Layered Crate Architecture
**Date:** 2025-01-31
**Decision:** Organize crates into framework → runtime → services → gateway → apps
**Rationale:** Clear dependencies, separation of concerns, maintainability

### ADR-004: UI as App
**Date:** 2025-01-31
**Decision:** Move UI under apps/ with dist/ at workspace root
**Rationale:** Consistent structure, UI is an application like daemon and CLI
