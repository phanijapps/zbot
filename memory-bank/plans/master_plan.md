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

- [x] 1.1 Add Tool Policy Framework (permissions, risk levels) ✓
- [x] 1.2 Add WebFetchTool (HTTP requests) ✓
- [x] 1.3 Add MemoryTool (persistent key-value) ✓
- [x] 1.4 Improve system prompt ✓
- [x] 1.5 Update documentation ✓

### Phase 2: Orchestrator Core
**Goal:** Replace workflow execution with orchestrator

- [ ] 2.1 Define Capability abstraction
- [ ] 2.2 Implement CapabilityRegistry
- [ ] 2.3 Implement TaskGraph
- [ ] 2.4 Implement Orchestrator (plan/route/execute)
- [ ] 2.5 Add ExecutionTrace
- [x] 2.6 Remove workflow-ide feature ✓

### Phase 3: Gateway Extraction ✅
**Goal:** Separate runtime from Tauri into standalone daemon

- [x] 3.1 Create `application/gateway/` crate (HTTP + WebSocket APIs) ✓
- [x] 3.2 Create `application/daemon/` binary (standalone server) ✓
- [x] 3.3 Connect gateway to agent runtime (ExecutionRunner, RuntimeService) ✓
- [x] 3.4 Refactor Tauri to connect via gateway ✓
  - Gateway client module in Tauri
  - Settings UI for gateway mode
  - Automatic execution routing
  - Status bar indicator
  - Daemon auto-start

See `memory-bank/plans/phase3_gateway.md` for detailed plan.

### Phase 4: CLI Interface (In Progress)
**Goal:** Full-featured CLI for headless operation

- [x] 4.1 Create zero-cli crate ✓
- [x] 4.2 Daemon management commands ✓
- [x] 4.3 Agent invocation commands ✓
- [x] 4.4 Interactive chat mode (TUI) ✓

See `application/zero-cli/` for implementation.

### Phase 5: Web Dashboard (In Progress)
**Goal:** Standalone web UI

- [x] 5.1 Refactor services to use gateway client ✓
  - Created transport abstraction layer (`src/services/transport/`)
  - HttpTransport for web (fetch + WebSocket)
  - TauriTransport wrapper for existing IPC
  - Auto-detection of runtime environment
- [x] 5.2 Build as standalone web app ✓
  - `vite.web.config.ts` for web-only builds
  - `index.web.html` entry point
  - `App.web.tsx` web-specific app component
  - `WebChatPanel.tsx` transport-based chat
  - npm scripts: `dev:web`, `build:web`, `preview:web`
- [x] 5.3 Serve from daemon ✓
  - `--static-dir` flag for serving dashboard
  - `--no-dashboard` flag to disable
  - Static file serving via tower-http

See `src/services/transport/` for transport layer implementation.

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

**Branch:** `cc`
**Phase:** 5 (Web Dashboard) - In Progress
**Next Step:** Test and iterate on web dashboard, remove Tauri dependency

### Phase 1 Completed ✅
- 1.1 Tool Policy Framework (risk levels, permissions)
- 1.2 WebFetchTool (HTTP with security)
- 1.3 MemoryTool (persistent key-value)
- 1.4 System prompt improvements
- 1.5 Documentation updates

### Phase 2 Completed ✅
- 2.1 Capability abstraction (`crates/zero-core/src/capability.rs`)
  - CapabilityKind (Tool, Skill, McpServer, SubAgent)
  - CapabilityDescriptor with routing score
  - CapabilityProvider trait
- 2.2 CapabilityRegistry (`crates/zero-core/src/registry.rs`)
  - UnifiedCapabilityRegistry for all capability kinds
  - Capability routing with scoring
- 2.3 TaskGraph (`crates/zero-agent/src/orchestrator/task_graph.rs`)
  - DAG with cycle detection
  - Topological sort, parallel groups
- 2.4 Orchestrator (`crates/zero-agent/src/orchestrator/mod.rs`)
  - OrchestratorAgent with execute_graph()
  - Capability-based routing
- 2.5 ExecutionTrace (`crates/zero-agent/src/orchestrator/trace.rs`)
  - TraceEvent, TraceMetrics, TraceBuilder
- 2.6 Remove workflow-ide ✅

### Phase 3 Completed ✅
- 3.1 Gateway crate (`application/gateway/`)
  - HTTP API with Axum (health, agents, conversations)
  - WebSocket handler for real-time streaming
  - EventBus for broadcast
- 3.2 Daemon binary (`application/daemon/`)
  - Standalone server with CLI args
  - Signal handling, graceful shutdown
- 3.3 Executor integration
  - ExecutionRunner converts StreamEvent to GatewayEvent
  - RuntimeService wraps execution lifecycle
- 3.4 Tauri gateway integration
  - Gateway client module (WebSocket)
  - Settings UI (use_gateway, ports, auto_start)
  - ConversationService routes to gateway/direct
  - StatusBar with connection indicator
  - Daemon auto-start capability

See `memory-bank/plans/phase3_gateway.md` for detailed implementation plan.

### Phase 4 Completed ✅
- 4.1 zero-cli crate (`application/zero-cli/`)
  - Binary name: `zero`
  - Uses ratatui for rich TUI
  - Uses crossterm for terminal handling
- 4.2 Daemon management commands
  - `zero status` - Check gateway status
  - `zero daemon status` - Check if daemon is running
  - `zero daemon info` - Show daemon details
- 4.3 Agent invocation commands
  - `zero agents` - List available agents
  - `zero agents -v` - List with descriptions
  - `zero invoke <agent> "message"` - Single message invocation with streaming
- 4.4 Interactive chat mode (TUI)
  - `zero chat <agent>` - Interactive chat with rich UI
  - Features: message history, tool call visualization, iteration tracking
  - Keybinds: `i` to input, `Enter` to send, `Esc` to cancel, `Ctrl+C` to quit

### Phase 5 In Progress
- 5.1 Transport abstraction layer (`src/services/transport/`)
  - `HttpTransport` - Web-native fetch + WebSocket
  - `TauriTransport` - Wrapper for existing IPC
  - Auto-detection via `__TAURI__` global
- 5.2 Web-only build configuration
  - `vite.web.config.ts` - Standalone build config
  - `index.web.html` - Web entry HTML
  - `src/main.web.tsx` - Web entry point
  - `src/App.web.tsx` - Web app component
  - `src/features/agent/WebChatPanel.tsx` - Transport-based chat
  - npm scripts: `dev:web`, `build:web`, `preview:web`
- 5.3 Daemon static file serving
  - `--static-dir PATH` - Serve dashboard from path
  - `--no-dashboard` - Disable dashboard serving
