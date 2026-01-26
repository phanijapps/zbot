# Technical Map

Quick reference for key modules, architectural decisions, and known fixes/workarounds.

## Key Modules

### Rust Crates (`crates/`)

| Crate | Purpose | Key Files |
|-------|---------|-----------|
| `zero-core` | Core traits: Agent, Tool, Session, Event | `src/agent.rs`, `src/tool.rs`, `src/event.rs` |
| `zero-llm` | LLM abstractions, OpenAI client | `src/llm.rs`, `src/openai.rs` |
| `zero-agent` | Agent implementations | `src/llm_agent.rs`, `src/workflow/sequential_agent.rs` |
| `zero-session` | Session trait, in-memory impl | `src/session.rs` |
| `zero-mcp` | MCP protocol client | `src/client.rs`, `src/bridge.rs` |

### Application Crates (`application/`)

| Crate | Purpose | Key Files |
|-------|---------|-----------|
| `agent-runtime` | Agent executor, config loading | `src/executor.rs`, `src/config.rs` |
| `agent-tools` | Built-in tools (Read, Write, Glob, Grep) | `src/lib.rs` |
| `workflow-executor` | Workflow loading and execution | `src/loader.rs`, `src/executor.rs` |
| `daily-sessions` | SQLite-based session storage | `src/repository.rs` |
| `knowledge-graph` | Entity/relationship memory | `src/lib.rs` |

### Frontend Features (`src/features/`)

| Feature | Purpose | Key Files |
|---------|---------|-----------|
| `workflow-ide` | Visual workflow builder | `WorkflowIDEPage.tsx`, `workflowStore.ts`, `useWorkflowExecution.ts` |
| `agent-channels` | Discord-like chat UI | `AgentChannelPanel.tsx` |
| `agents` | Agent management | `AgentIDEPage.tsx` |
| `providers` | LLM provider config | `ProvidersPage.tsx` |

## Key Architectural Decisions

### 1. Workflow as Orchestrator Tools
Subagents are registered as tools for the orchestrator LLM. The orchestrator decides which subagent to call based on its instructions and the user's request.

### 2. Flow-Level Orchestrator Config
Orchestrator configuration (provider, model, system prompt) is stored at flow level, not as a node. Legacy orchestrator nodes are auto-migrated on workflow load.

### 3. Event-Based Execution Visualization
Workflow execution emits `agent_start` and `agent_end` lifecycle events with metadata. Frontend listens on Tauri event channels to update node visual status in real-time.

### 4. Frontend-Generated Invocation IDs
To prevent race conditions where events arrive before listeners are set up, the frontend generates `invocationId` before calling `invoke()` and sets up listeners first.

### 5. Multi-Vault Architecture
Each vault is a self-contained directory with agents, skills, providers, MCPs. Global config (`~/.config/agentzero/`) stores vault registry and shared utilities.

### 6. Instructions in AGENTS.md Only
Agent/subagent instructions live in `AGENTS.md` files, not in `config.yaml`. This keeps configs lean and instructions version-control friendly.

## Critical Fixes & Workarounds

### Workflow Execution

**Issue**: Nodes don't transition during execution
**Fix**: Added lifecycle events to `SequentialAgent` (`crates/zero-agent/src/workflow/sequential_agent.rs:85-95`):
```rust
start_event.metadata.insert("agent_lifecycle", json!("start"));
start_event.metadata.insert("agent_id", json!(agent_name));
```

**Issue**: Events emitted before frontend listeners are ready (race condition)
**Fix**: Generate `invocationId` on frontend, set up `listen()` calls BEFORE `invoke()`, spawn streaming as background task (`src-tauri/src/commands/workflow.rs:45-60`).

**Issue**: Stop button doesn't stop execution
**Fix**: Added `ACTIVE_EXECUTIONS` map with `AtomicBool` cancellation flags. Backend checks flag in streaming loop (`src-tauri/src/commands/workflow.rs:25-40`).

**Issue**: Subagent file tools fail with "Agent ID not found in state"
**Fix**: Share orchestrator's `agent_id` with subagents via session state (`application/workflow-executor/src/executor.rs:125`):
```rust
session.state_mut().set("app:agent_id", json!(self.workflow.definition.id));
```

### Workflow IDE State

**Issue**: "Unsaved changes" shown immediately after loading
**Fix**: Filter `onNodesChange`/`onEdgesChange` to only mark dirty for meaningful changes (position, remove, add) (`src/features/workflow-ide/stores/workflowStore.ts:85-95`).

**Issue**: Run button disabled for saved workflows
**Fix**: Call `setIsDirty(false)` after loading workflow data (`src/features/workflow-ide/WorkflowIDEPage.tsx:180`).

### Event Struct Field

**Issue**: TypeScript compilation error - field `custom` not found on Event
**Fix**: Use `metadata` field instead of `custom`. The Event struct has `metadata: HashMap<String, Value>`.

## Tauri Event Channels

| Channel Pattern | Purpose |
|-----------------|---------|
| `workflow-stream://{invocationId}` | Main execution stream (tokens, tool calls, done/error) |
| `workflow-node://{workflowId}` | Node status updates (running, completed, failed) |
| `agent-stream://{sessionId}` | Legacy agent execution events |

## Database Schema

**Location**: `{vault}/db/agent_channels.db`

| Table | Purpose |
|-------|---------|
| `daily_sessions` | Day-based session grouping |
| `messages` | Chat messages with tool calls/results |
| `kg_entities` | Knowledge graph entities |
| `kg_relationships` | Entity relationships |

## File Structure Conventions

```
{vault}/agents/{agent-name}/
├── config.yaml           # Metadata only (no instructions)
├── AGENTS.md             # Instructions (source of truth)
├── .workflow-layout.json # Visual node positions
└── .subagents/           # Subagent configs
    └── {subagent-name}/
        ├── config.yaml
        └── AGENTS.md
```
