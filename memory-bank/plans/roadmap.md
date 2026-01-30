# Agent Zero — Development Roadmap

## Current Status

**Version:** 0.2.0-dev (Timeline Zero)
**Branch:** timeline_zero
**Status:** Core functionality working, active development

### Completed

- [x] Gateway architecture (HTTP + WebSocket)
- [x] Agent execution with streaming
- [x] Provider management with default selection
- [x] Skill system (file-based, frontmatter metadata)
- [x] Built-in tools (file, shell, memory, introspection)
- [x] SQLite conversation persistence
- [x] Per-agent memory storage
- [x] Web dashboard (React + Vite)
- [x] Real-time streaming UI

### In Progress

- [ ] MCP server integration
- [ ] CLI improvements
- [ ] Generative UI (request_input, show_content)

---

## Phase 1: Foundation (Current)

**Goal:** Stable core with single-agent conversations

### 1.1 Core Execution ✓
- [x] Gateway with HTTP/WebSocket APIs
- [x] Agent executor with LLM loop
- [x] Tool execution with streaming events
- [x] Conversation persistence (SQLite)
- [x] Agent memory (JSON key-value)

### 1.2 Provider System ✓
- [x] Multi-provider support (OpenAI-compatible)
- [x] Default provider selection
- [x] Connection testing
- [x] Per-agent provider override

### 1.3 Tool System ✓
- [x] Tool registry with permissions
- [x] Built-in tools (file, shell, memory)
- [x] Introspection tools (list_skills, list_tools)
- [x] FileSystemContext for path resolution

### 1.4 Web Dashboard ✓
- [x] Chat interface with streaming
- [x] Provider management
- [x] Agent management (basic)
- [x] Skill listing

---

## Phase 2: Integration

**Goal:** External tools and enhanced UX

### 2.1 MCP Integration
- [ ] MCP manager with lazy initialization
- [ ] Tool discovery from MCP servers
- [ ] MCP tool execution
- [ ] Configuration UI

### 2.2 Generative UI
- [x] GenerativeCanvas component
- [ ] request_input tool implementation
- [ ] show_content tool implementation
- [ ] Form validation and submission

### 2.3 CLI Enhancement
- [ ] Interactive chat mode
- [ ] Agent invocation commands
- [ ] Configuration commands
- [ ] Output formatting

### 2.4 Skill Enhancement
- [ ] Skill search and filtering
- [ ] Skill installation from registry
- [ ] Skill dependency management
- [ ] Hot reload on change

---

## Phase 3: Multi-Agent

**Goal:** Orchestrated multi-agent workflows

### 3.1 Subagent System
- [ ] Subagent tool for delegation
- [ ] Context passing between agents
- [ ] Result aggregation
- [ ] Error propagation

### 3.2 Orchestrator Patterns
- [ ] Sequential execution
- [ ] Parallel fan-out
- [ ] Conditional routing
- [ ] Loop with termination

### 3.3 State Management
- [ ] Shared state between agents
- [ ] State snapshots
- [ ] Rollback capability

---

## Phase 4: Production

**Goal:** Ready for real-world deployment

### 4.1 Scheduled Tasks
- [ ] Cron-based scheduling
- [ ] Task history
- [ ] Retry on failure
- [ ] Notification on completion

### 4.2 Security
- [ ] Tool permission enforcement
- [ ] Sandbox for shell commands
- [ ] Rate limiting
- [ ] Audit logging

### 4.3 Performance
- [ ] Response caching
- [ ] Connection pooling
- [ ] Memory optimization
- [ ] Startup time reduction

### 4.4 Distribution
- [ ] Single binary release
- [ ] Docker image
- [ ] Homebrew formula
- [ ] Windows installer

---

## Phase 5: Advanced Features

**Goal:** Power user capabilities

### 5.1 Knowledge Graph
- [ ] Entity extraction
- [ ] Relationship mapping
- [ ] Semantic search
- [ ] Context injection

### 5.2 Voice Integration
- [ ] Speech-to-text input
- [ ] Text-to-speech output
- [ ] Wake word detection
- [ ] Continuous conversation

### 5.3 Plugin System
- [ ] Plugin API
- [ ] Plugin marketplace
- [ ] Sandboxed execution
- [ ] Version management

---

## Technical Debt

### High Priority
- [ ] Error handling consistency
- [ ] Logging standardization
- [ ] Test coverage (unit + integration)
- [ ] API documentation

### Medium Priority
- [ ] Code documentation
- [ ] Performance profiling
- [ ] Memory leak detection
- [ ] Dependency updates

### Low Priority
- [ ] Code style unification
- [ ] Unused code removal
- [ ] Comment cleanup
- [ ] Example improvements

---

## Milestones

| Milestone | Target | Status |
|-----------|--------|--------|
| v0.1.0 | Core chat working | ✓ |
| v0.2.0 | SQLite + Memory | ✓ |
| v0.3.0 | MCP + Generative UI | In Progress |
| v0.4.0 | Multi-agent | Planned |
| v0.5.0 | Scheduled tasks | Planned |
| v1.0.0 | Production ready | Planned |

---

## Architecture Decisions Log

### ADR-001: Remove Tauri
**Date:** 2025-01-29
**Decision:** Replace Tauri desktop app with web dashboard + daemon
**Rationale:**
- Simpler deployment (no native installers)
- Better browser capabilities
- Easier debugging
- Cross-platform without builds

### ADR-002: SQLite for Conversations
**Date:** 2025-01-29
**Decision:** Use SQLite instead of in-memory HashMap
**Rationale:**
- Persistence across restarts
- ACID transactions
- Query capability
- Single file, portable

### ADR-003: Single Daemon Process
**Date:** 2025-01-29
**Decision:** Combine gateway and runtime in single process
**Rationale:**
- Simpler deployment
- No IPC complexity
- Shared state
- Easier debugging

### ADR-004: File-based Agent Config
**Date:** 2025-01-28
**Decision:** Store agent instructions in AGENTS.md files
**Rationale:**
- Human-readable
- Version control friendly
- Separates behavior from metadata
- Supports markdown rendering
