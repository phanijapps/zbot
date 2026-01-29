# AgentZero Master Plan

## Vision

A simple, powerful AI agent platform where users state goals and the orchestrator handles the rest. No workflow design needed - the AI plans, routes to capabilities, executes, and delivers.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                              ZERO DAEMON                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                         GATEWAY                                │  │
│  │     WebSocket :18790  │  HTTP :18791  │  Events Bus           │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                │                                     │
│  ┌─────────────────────────────▼─────────────────────────────────┐  │
│  │                      ORCHESTRATOR                              │  │
│  │  User Goal → Interpret → Plan → Route → Execute → Deliver     │  │
│  └─────────────────────────────┬─────────────────────────────────┘  │
│                                │                                     │
│  ┌─────────────────────────────▼─────────────────────────────────┐  │
│  │                   CAPABILITY REGISTRY                          │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐   │  │
│  │  │ Skills  │  │  Tools  │  │   MCPs  │  │   Sub-Agents    │   │  │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
         ▲              ▲                    ▲
         │              │                    │
   ┌─────┴────┐  ┌──────┴──────┐  ┌─────────┴─────────┐
   │ zero CLI │  │ Web Dashboard│  │   Tauri App       │
   └──────────┘  └─────────────┘  └───────────────────┘
```

---

## Phases

### Phase 1: Foundation (Current)
**Goal:** Strengthen core crates with missing capabilities

- [ ] 1.1 Add Tool Policy Framework (permissions, risk levels)
- [ ] 1.2 Add WebFetchTool (HTTP requests)
- [ ] 1.3 Add MemoryTool (persistent key-value)
- [ ] 1.4 Improve system prompt
- [ ] 1.5 Update documentation

### Phase 2: Orchestrator Core
**Goal:** Replace workflow execution with orchestrator

- [ ] 2.1 Define Capability abstraction
- [ ] 2.2 Implement CapabilityRegistry
- [ ] 2.3 Implement TaskGraph
- [ ] 2.4 Implement Orchestrator (plan/route/execute)
- [ ] 2.5 Add ExecutionTrace
- [ ] 2.6 Remove workflow-ide feature

### Phase 3: Gateway Extraction
**Goal:** Separate runtime from Tauri into standalone daemon

- [ ] 3.1 Create zero-gateway crate
- [ ] 3.2 Create zero-daemon binary
- [ ] 3.3 WebSocket + HTTP API
- [ ] 3.4 Refactor Tauri to connect via gateway

### Phase 4: CLI Interface
**Goal:** Full-featured CLI for headless operation

- [ ] 4.1 Create zero-cli crate
- [ ] 4.2 Daemon management commands
- [ ] 4.3 Agent invocation commands
- [ ] 4.4 Interactive chat mode

### Phase 5: Web Dashboard
**Goal:** Standalone web UI

- [ ] 5.1 Refactor services to use gateway client
- [ ] 5.2 Build as standalone web app
- [ ] 5.3 Serve from daemon

---

## Phase 1 Details

### 1.1 Tool Policy Framework

**Files:**
- `crates/zero-core/src/policy.rs` (new)
- `crates/zero-core/src/tool.rs` (modify)
- `crates/zero-core/src/lib.rs` (modify)

**Deliverables:**
- `ToolRiskLevel` enum (Safe, Moderate, Dangerous, Critical)
- `ToolPermissions` struct
- `Tool::permissions()` method
- `Tool::validate()` method

### 1.2 WebFetchTool

**Files:**
- `application/agent-tools/src/tools/web.rs` (new)
- `application/agent-tools/src/tools/mod.rs` (modify)
- `application/agent-tools/src/lib.rs` (modify)

**Deliverables:**
- HTTP GET/POST/PUT/DELETE support
- Security: blocked hosts, size limits, timeouts
- Headers and body support

### 1.3 MemoryTool

**Files:**
- `application/agent-tools/src/tools/memory.rs` (new)

**Deliverables:**
- Persistent key-value storage
- Actions: get, set, delete, list, search
- Tags for organization
- Stored in agent's data directory

### 1.4 System Prompt Improvements

**Files:**
- `src-tauri/templates/system_prompt.md` (modify)

**Deliverables:**
- Tool call style guidelines
- Memory usage instructions
- Better error handling guidance
- Clearer capability sections

### 1.5 Documentation Updates

**Files:**
- `memory-bank/architecture.md` (update)
- `memory-bank/product.md` (update)
- `crates/zero-core/AGENTS.md` (new)
- `application/agent-tools/AGENTS.md` (new)

---

## Success Criteria

### Phase 1
- [ ] All existing tests pass
- [ ] New tools have tests
- [ ] Policy framework integrated
- [ ] Documentation current

### Phase 2
- [ ] Orchestrator handles all current use cases
- [ ] Workflow IDE removed
- [ ] Execution traces visible in UI

### Phase 3
- [ ] Daemon runs standalone
- [ ] Tauri connects via WebSocket
- [ ] No functionality regression

### Phase 4
- [ ] CLI can invoke agents
- [ ] Interactive chat works
- [ ] Daemon management works

### Phase 5
- [ ] Web dashboard functional
- [ ] Same features as Tauri app
- [ ] Accessible from any browser

---

## What Gets Removed

| Component | When | Replacement |
|-----------|------|-------------|
| `src/features/workflow-ide/` | Phase 2 | Orchestrator |
| `application/workflow-executor/` | Phase 2 | Orchestrator |
| XY Flow dependency | Phase 2 | None needed |
| Tauri IPC (direct) | Phase 3 | Gateway WebSocket |

---

## Current Status

**Branch:** `timeline_zero`
**Phase:** 1 (Foundation)
**Next Step:** 1.1 Tool Policy Framework
