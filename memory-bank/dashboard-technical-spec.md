# Dashboard Technical Specification

> **Implementation:** See `memory-bank/dashboard-implementation-plan.md` for step-by-step execution plan with copy-paste prompts.

## Document Purpose

This document provides the **architecture, design rationale, and requirements** for the dashboard and session management system. It covers:
- System architecture (triggers, gateway bus, services, consumers)
- Data model (sessions, executions, messages)
- Product vision (multi-channel orchestration hub)
- Current bugs and their root causes
- Required fixes

For **how to implement**, see the implementation plan.

---

## Part -1: Architecture Principles

### Existing Crate Structure (Use This)

```
services/                          ← Domain services (isolated, single responsibility)
  ├── execution-state/             ← Session/Execution state management
  │     ├── types.rs               ← Domain entities (Session, Execution, etc.)
  │     ├── repository.rs          ← Storage (currently SQLite, swappable)
  │     └── service.rs             ← Business logic
  │
  ├── api-logs/                    ← Request/response logging
  ├── daily-sessions/              ← Daily aggregation
  ├── knowledge-graph/             ← Knowledge storage
  ├── search-index/                ← Search functionality
  └── session-archive/             ← Archive old sessions

gateway/                           ← HTTP/WebSocket handlers (presentation)
  └── src/
        ├── http/                  ← REST endpoints
        ├── websocket/             ← Real-time events
        └── execution/             ← Agent execution orchestration

framework/                         ← Core abstractions (traits, shared types)
  ├── zero-core/                   ← Core types
  ├── zero-agent/                  ← Agent trait/types
  ├── zero-session/                ← Session abstractions
  ├── zero-llm/                    ← LLM provider trait
  └── ...

apps/
  └── ui/                          ← Frontend (React)
        └── src/
              ├── services/transport/  ← Backend communication (swappable)
              └── features/ops/        ← Dashboard components
```

### Swappability Points (Already Exist or Easy to Add)

| Concern | Current | Trait Location | Swap To |
|---------|---------|----------------|---------|
| Storage | SQLite | `execution-state/repository.rs` | Postgres, DynamoDB |
| LLM | Anthropic | `framework/zero-llm` | OpenAI, local |
| Transport | HTTP | `apps/ui/src/services/transport` | gRPC, Tauri IPC |
| MCP | stdio | `framework/zero-mcp` | HTTP, WebSocket |

### Where New Capabilities Go

| Capability | Crate | Why |
|------------|-------|-----|
| Session queuing | `services/execution-state/` | Extends session lifecycle |
| Trigger sources | `services/triggers/` (new) | New service, isolated |
| Event bus | `framework/zero-events/` (new) | Core abstraction |
| Stats aggregation | `services/execution-state/` | Already has stats query |
| Queue manager | `services/execution-state/` or `services/queue/` (new) | Depends on scope |

### For Distribution (Future)

```
services/execution-state/
  ├── repository.rs          ← Add trait, impl for SQLite
  ├── repository_sqlite.rs   ← SQLite impl (current code)
  ├── repository_postgres.rs ← Postgres impl (future)
  └── repository_dynamo.rs   ← DynamoDB impl (future)
```

Service code uses trait → swap impl via config, zero business logic changes.

### Dashboard Data Flow

**Current (Polling):**
```
Dashboard → GET /api/v2/sessions/full every 5s
          → GET /api/v2/stats every 5s
```

**Target (Event-Driven via WebSocket):**
```
Dashboard → Initial: GET /api/v2/sessions/full + GET /api/v2/stats
          → Subscribe: WS /api/events
          → On event: Update local state (no polling)
```

### Frontend Transport (Already Swappable)

Location: `apps/ui/src/services/transport/`

```typescript
// interface.ts - Already exists
interface Transport {
  listExecutionSessions(): Promise<TransportResult<ExecutionSession[]>>;
  getExecutionStats(): Promise<TransportResult<ExecutionStats>>;
  subscribe(conversationId: string, callback: EventCallback): UnsubscribeFn;
  // ...
}

// Implementations already exist:
// - http.ts (HttpTransport) ← Current
// - tauri.ts (TauriTransport) ← Stub for desktop
```

---

## Part 0: Product Vision - Multi-Channel Agent Orchestration

The system is an **agent orchestration hub** that manages multiple concurrent sessions from various trigger sources. This is NOT just a chat application.

### Target State (Design For)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AGENT ORCHESTRATION HUB                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Session 1 [WEB] (running)           Session 2 [WEB] (running)              │
│    root ──────────────────┐            root ─────────────────┐              │
│      ├─ research (running)│              └─ stock-agent (running)           │
│      └─ writer (queued)   │                                                 │
│                           │                                                  │
│  Session 3 [CRON] (running)          Session 4 [SIGNAL] (running)           │
│    root ─────────────────┐             root (running, no delegation)        │
│      └─ whatsapp (running)│                                                 │
│                           │                                                  │
│  Session 5 [EMAIL] (queued)          Session 6 [API] (queued)               │
│    root (waiting)                      root (waiting)                        │
│      └─ subagent TBD                     └─ subagent TBD                    │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  STATS: 4 running, 2 queued | 5 executions running, 2 queued, 3 completed   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### System Architecture

```
                              TRIGGERS (intake)
    ┌───────┬───────┬───────┬──────────────┬─────────────────────────┐
    │  Web  │  CLI  │ Cron  │ Rust Plugins │ Foreign Plugins         │
    │       │       │       │              │ (JS, Python, Go bridge) │
    └───┬───┴───┬───┴───┬───┴───────┬──────┴───────────┬─────────────┘
        │       │       │           │                  │
        └───────┴───────┴───────────┴──────────────────┘
                              │
                              ▼
                ┌───────────────────────────┐
                │    ZERO GATEWAY BUS       │
                │   (unified intake)        │
                └─────────────┬─────────────┘
                              │
                              ▼
                ┌───────────────────────────┐
                │       ROOT AGENT          │
                │     (does its magic)      │
                └─────────────┬─────────────┘
                              │
                              ▼
    ┌─────────────────────────────────────────────────────────────────┐
    │                        SINGLE DATABASE                          │
    │                                                                 │
    │   sessions: ALL sessions from ALL sources                      │
    │   agent_executions: ALL executions                             │
    │   messages: ALL messages                                        │
    │                                                                 │
    │   Every session tagged with source (web, cron, signal, etc.)   │
    └─────────────────────────────────────────────────────────────────┘
                              │
                              │ query
                              ▼
                ┌───────────────────────────┐
                │        SERVICES           │
                │   (execution-state, etc.) │
                └─────────────┬─────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
    ┌───────┐           ┌───────────┐         ┌─────────┐
    │  UI   │           │    CLI    │         │ Ext API │
    │       │           │           │         │         │
    │ COMMAND CENTER    │  inspect  │         │ integrate│
    │ - Monitor ALL     │  control  │         │ automate │
    │ - Control ALL     │           │         │          │
    │ - Filter by src   │           │         │          │
    └───────┘           └───────────┘         └─────────┘
```

### Key Points

1. **Single Database** - All triggers write to the same DB
   - Web session? → DB
   - Cron job? → DB
   - Signal message? → DB
   - All tagged with `source` field

2. **UI = Command Center** - Monitors and controls EVERYTHING
   - See all running sessions (from any source)
   - Filter: "Show only cron jobs", "Show only Signal"
   - Control: Pause, resume, cancel any session
   - Stats: Aggregated across all sources

3. **CLI** - Same access, different interface
   - `zero sessions list --source=cron`
   - `zero sessions cancel sess-xxx`

4. **External API** - Programmatic access
   - Other systems can query/control sessions

### Zero Gateway Bus

**Purpose:** Single interface for ALL session intake, regardless of source or language.

**Location:** `framework/zero-gateway/` or `gateway/src/bus/`

```rust
/// The unified intake interface
pub trait GatewayBus: Send + Sync {
    /// Start a new session (or continue existing)
    fn submit(&self, request: SessionRequest) -> Result<SessionHandle>;

    /// Get status of a session
    fn status(&self, session_id: &str) -> Result<SessionStatus>;

    /// Cancel a session
    fn cancel(&self, session_id: &str) -> Result<()>;
}

/// Unified request from any trigger
pub struct SessionRequest {
    pub session_id: Option<String>,  // None = new session, Some = continue
    pub source: TriggerSource,
    pub agent_id: String,
    pub message: String,
    pub priority: Option<u32>,
    pub external_ref: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// What the bus returns
pub struct SessionHandle {
    pub session_id: String,
    pub execution_id: String,
}
```

### Trigger Sources

| Source | Language | Integration |
|--------|----------|-------------|
| Web | Rust | Direct - HTTP handler calls `bus.submit()` |
| CLI | Rust | Direct - CLI calls `bus.submit()` |
| Cron | Rust | Direct - Scheduler calls `bus.submit()` |
| Rust Plugin | Rust | Direct - Plugin calls `bus.submit()` |
| Signal | JS | FFI/IPC - JS process sends via Unix socket/HTTP |
| WhatsApp | Python | FFI/IPC - Python bridge sends via Unix socket/HTTP |
| Custom | Go/Any | FFI/IPC - Bridge sends via Unix socket/HTTP |

### Foreign Plugin Interface (Future)

For non-Rust triggers, expose Gateway Bus via:

```
Option A: HTTP endpoint (simplest)
  POST /api/gateway/submit
  → Any language can call via HTTP

Option B: Unix socket + JSON
  → Lower latency, local only

Option C: gRPC
  → Type-safe, cross-language
```

The Gateway Bus implementation handles all of these and routes to the same internal `bus.submit()` logic.

### Execution Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| **No Delegation** | Root handles everything | Simple Q&A, immediate response |
| **Single Delegation** | Root → one subagent | Research task |
| **Sequential Chain** | Root → A → B → C | Research → Analyze → Write |
| **Parallel Fan-out** | Root → [A, B, C] simultaneously | Multi-source research |
| **Mixed** | Combination of above | Complex workflows |

### Design Requirements

1. **Session-Level Queuing**: Sessions can be `queued` before starting (resource constraints, rate limits)
2. **Source Tracking**: Every session knows its trigger source for filtering/display
3. **True Parallelism**: Multiple sessions run concurrently, independent lifecycles
4. **Priority**: Queued sessions may have priority ordering
5. **Resource Awareness**: System may limit concurrent sessions (configurable)

### Queue Manager (Future)

**Location:** `services/execution-state/` (extend existing) or `services/queue/` (new crate)

```rust
// Configuration
struct QueueConfig {
    max_concurrent_sessions: u32,           // 0 = unlimited
    max_per_source: HashMap<TriggerSource, u32>,  // Per-source limits
}

// Trait for swappable implementations
trait QueueManager: Send + Sync {
    fn should_queue(&self, request: &SessionRequest) -> bool;
    fn enqueue(&self, session_id: &str, priority: u32) -> Result<u32>; // Returns position
    fn dequeue(&self) -> Result<Option<String>>;  // Next session to start
    fn mark_started(&self, session_id: &str) -> Result<()>;
    fn mark_finished(&self, session_id: &str) -> Result<()>;
}

// Implementations:
// - InMemoryQueueManager (single node, uses BinaryHeap)
// - RedisQueueManager (distributed, uses sorted sets)
```

**Flow:**
```
Request → should_queue()?
          ├─ NO  → Start immediately (Running)
          └─ YES → Enqueue (Queued) → Wait for slot → Dequeue → Start
```

### Data Model Extensions Needed

```rust
pub enum SessionStatus {
    Queued,     // NEW: Waiting to start (resource constraint or rate limit)
    Running,    // At least one execution active
    Paused,     // User paused
    Completed,  // All done
    Crashed,    // Root crashed
}

pub struct Session {
    // ... existing fields ...

    // NEW: Trigger source
    pub source: TriggerSource,  // web | cron | signal | email | api | webhook

    // NEW: Priority for queue ordering (lower = higher priority)
    pub priority: Option<u32>,

    // NEW: External reference (email ID, webhook event ID, etc.)
    pub external_ref: Option<String>,
}

pub enum TriggerSource {
    Web,
    Cron,
    Signal,
    Email,
    Api,
    Webhook,
}
```

### Dashboard Display Requirements

1. **Show source badge** for each session (web, cron, signal, etc.)
2. **Filter by source** - "Show only cron jobs", "Show only web sessions"
3. **Queued sessions panel** - Sessions waiting to start
4. **Running sessions panel** - Active sessions with execution hierarchy
5. **Stats breakdown** - By source and by overall totals

---

## Part 1: Data Model

### Entity Relationships

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              SESSION                                     │
│  id: sess-{uuid}                                                        │
│  status: running | paused | completed | crashed                         │
│  root_agent_id: "root"                                                  │
│  created_at, started_at, completed_at                                   │
│  total_tokens_in, total_tokens_out                                      │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ 1:N
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           AGENT EXECUTION                                │
│  id: exec-{uuid}                                                        │
│  session_id: FK → sessions.id                                           │
│  agent_id: "root" | "research-agent" | etc.                             │
│  parent_execution_id: FK → agent_executions.id (NULL for root)          │
│  delegation_type: root | sequential | parallel                          │
│  status: queued | running | paused | crashed | cancelled | completed    │
│  task: "Research X" (for subagents)                                     │
│  tokens_in, tokens_out                                                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ 1:N
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              MESSAGE                                     │
│  id: msg-{uuid}                                                         │
│  execution_id: FK → agent_executions.id                                 │
│  role: user | assistant | system                                        │
│  content: "..."                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Concepts

| Term | Definition |
|------|------------|
| **Session** | Top-level container for a user's work session. One session per chat window/conversation. |
| **Execution** | An agent's participation in a session. One session can have MULTIPLE executions. |
| **Root Execution** | The first execution in a session, created when user sends a message. `delegation_type = 'root'`. |
| **Subagent Execution** | Created when root (or another agent) delegates. Has `parent_execution_id` set. |
| **Turn** | Each user message creates a NEW root execution within the SAME session. |

### Multi-Turn Conversation Example

```
User sends message 1 → Creates exec-001 (root, turn 1)
User sends message 2 → Creates exec-002 (root, turn 2)
User sends message 3 → Creates exec-003 (root, turn 3)
  └─ exec-003 delegates → Creates exec-004 (subagent)

Session: sess-abc
├── exec-001 (root, turn 1, completed)
├── exec-002 (root, turn 2, completed)
├── exec-003 (root, turn 3, completed)
└── exec-004 (subagent of exec-003, completed)
```

**This is CORRECT behavior.** Each user message = new execution. Same session.

---

## Part 2: Execution Lifecycle

### State Machine

```
                    ┌──────────┐
                    │  QUEUED  │ ← Initial state (on creation)
                    └────┬─────┘
                         │ start_execution()
                         ▼
                    ┌──────────┐
            ┌───────│ RUNNING  │───────┐
            │       └────┬─────┘       │
            │            │             │
     user stops     completes       crashes
            │            │             │
            ▼            ▼             ▼
       ┌──────────┐ ┌──────────┐ ┌──────────┐
       │CANCELLED │ │COMPLETED │ │ CRASHED  │
       └──────────┘ └──────────┘ └──────────┘
            ▲            ▲             ▲
            └────────────┴─────────────┘
                   Terminal States
```

### When Does Each Transition Happen?

| Transition | Trigger | Code Location |
|------------|---------|---------------|
| → QUEUED | Execution created | `types.rs:302` `new_root()` |
| QUEUED → RUNNING | `start_execution()` | `lifecycle.rs:84` |
| RUNNING → COMPLETED | Executor returns `Ok()` | `runner.rs:341` → `lifecycle.rs:150` |
| RUNNING → CRASHED | Executor returns `Err()` | `runner.rs:355` → `lifecycle.rs:195` |
| RUNNING → CANCELLED | User stops | `runner.rs:371` → `lifecycle.rs:239` |

### Database Updates Timeline

```
T0: User sends message
    └─ INSERT INTO sessions (status='running')
    └─ INSERT INTO agent_executions (status='queued', delegation_type='root')

T1: Execution starts
    └─ UPDATE agent_executions SET status='running', started_at=NOW()

T2: Agent delegates to subagent
    └─ INSERT INTO agent_executions (status='queued', delegation_type='sequential', parent_execution_id=...)

T3: Subagent starts
    └─ UPDATE agent_executions SET status='running' WHERE id=subagent_exec_id

T4: Root execution completes
    └─ UPDATE agent_executions SET status='completed', completed_at=NOW() WHERE id=root_exec_id
    └─ CALL try_complete_session()
       └─ SELECT COUNT(*) FROM agent_executions WHERE session_id=? AND status IN ('running','queued')
       └─ If count > 0: Session stays 'running'
       └─ If count = 0: UPDATE sessions SET status='completed'

T5: Subagent completes
    └─ UPDATE agent_executions SET status='completed', completed_at=NOW()
    └─ CALL try_complete_session()
       └─ Now count = 0, so session becomes 'completed'
```

---

## Part 3: Session Lifecycle

### State Machine (Extended for Multi-Channel)

```
                    ┌──────────┐
                    │  QUEUED  │ ← Initial state (resource constrained)
                    └────┬─────┘
                         │ resources available / priority reached
                         ▼
                    ┌──────────┐
                    │ RUNNING  │ ← Or direct initial state (no queue)
                    └────┬─────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
    user pauses    all execs done   root crashes
         │               │               │
         ▼               ▼               ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │  PAUSED  │   │COMPLETED │   │ CRASHED  │
    └────┬─────┘   └──────────┘   └──────────┘
         │
    user resumes
         │
         ▼
    ┌──────────┐
    │ RUNNING  │
    └──────────┘
```

### When Sessions Are Queued vs Running Immediately

| Scenario | Initial State | Reason |
|----------|---------------|--------|
| Web chat, no constraints | `Running` | Immediate start |
| Cron job, max concurrency reached | `Queued` | Wait for slot |
| Email trigger, rate limited | `Queued` | Respect rate limit |
| API call with priority=low | `Queued` | Higher priority first |
| Signal message, urgent | `Running` | High priority bypass |

### Critical Function: try_complete_session()

**Location:** `services/execution-state/src/service.rs:213-222`

```rust
pub fn try_complete_session(&self, session_id: &str) -> Result<bool, String> {
    if self.has_running_executions(session_id)? {
        Ok(false)  // Keep session RUNNING
    } else {
        self.complete_session(session_id)?;
        Ok(true)   // Session is now COMPLETED
    }
}
```

**Called when:** ANY execution completes (root or subagent)

### Critical Function: has_running_executions()

**Location:** `services/execution-state/src/service.rs:185-211`

```rust
pub fn has_running_executions(&self, session_id: &str) -> Result<bool, String> {
    // Check RUNNING
    let running = SELECT * FROM agent_executions
                  WHERE session_id=? AND status='running';
    if !running.is_empty() { return Ok(true); }

    // Check QUEUED (pending subagents)
    let queued = SELECT * FROM agent_executions
                 WHERE session_id=? AND status='queued';

    Ok(!queued.is_empty())
}
```

**Why check QUEUED?** Subagent executions are created in QUEUED status synchronously when delegation happens. This prevents race condition where session completes before subagent starts.

---

## Part 4: Delegation Flow

### Sequence Diagram

```
┌────────┐          ┌────────┐          ┌────────┐          ┌────────┐
│  User  │          │  Root  │          │ System │          │Subagent│
└───┬────┘          └───┬────┘          └───┬────┘          └───┬────┘
    │                   │                   │                   │
    │ Send message      │                   │                   │
    │──────────────────>│                   │                   │
    │                   │                   │                   │
    │                   │ Create session    │                   │
    │                   │ Create root exec  │                   │
    │                   │──────────────────>│                   │
    │                   │                   │                   │
    │                   │ Start execution   │                   │
    │                   │ (QUEUED→RUNNING)  │                   │
    │                   │──────────────────>│                   │
    │                   │                   │                   │
    │                   │ Process...        │                   │
    │                   │ Decides to        │                   │
    │                   │ delegate          │                   │
    │                   │                   │                   │
    │                   │ SYNC: Create      │                   │
    │                   │ subagent exec     │                   │
    │                   │ (status=QUEUED)   │                   │
    │                   │──────────────────>│                   │
    │                   │                   │                   │
    │                   │ Send delegation   │                   │
    │                   │ request (async)   │                   │
    │                   │──────────────────>│                   │
    │                   │                   │                   │
    │                   │ Root continues... │                   │
    │                   │ Root completes    │                   │
    │                   │──────────────────>│                   │
    │                   │                   │                   │
    │                   │ try_complete_     │                   │
    │                   │ session()         │                   │
    │                   │──────────────────>│                   │
    │                   │                   │                   │
    │                   │ has_running?      │                   │
    │                   │ YES (subagent     │                   │
    │                   │ is QUEUED)        │                   │
    │                   │<──────────────────│                   │
    │                   │                   │                   │
    │                   │ Session stays     │                   │
    │                   │ RUNNING           │                   │
    │                   │                   │                   │
    │                   │                   │ Start subagent    │
    │                   │                   │ (QUEUED→RUNNING)  │
    │                   │                   │──────────────────>│
    │                   │                   │                   │
    │                   │                   │     Process...    │
    │                   │                   │<──────────────────│
    │                   │                   │                   │
    │                   │                   │ Subagent complete │
    │                   │                   │ (RUNNING→COMPLETE)│
    │                   │                   │──────────────────>│
    │                   │                   │                   │
    │                   │                   │ try_complete_     │
    │                   │                   │ session()         │
    │                   │                   │                   │
    │                   │                   │ has_running?      │
    │                   │                   │ NO (all done)     │
    │                   │                   │                   │
    │                   │                   │ Session→COMPLETED │
    │                   │                   │                   │
```

### Parent-Child Relationship

```sql
-- Root execution (no parent)
INSERT INTO agent_executions (
    id = 'exec-001',
    session_id = 'sess-abc',
    agent_id = 'root',
    parent_execution_id = NULL,     -- No parent
    delegation_type = 'root'
);

-- Subagent execution (has parent)
INSERT INTO agent_executions (
    id = 'exec-002',
    session_id = 'sess-abc',        -- SAME session
    agent_id = 'research-agent',
    parent_execution_id = 'exec-001', -- Links to root
    delegation_type = 'sequential'
);
```

---

## Part 5: What The Dashboard SHOULD Show

### Active Sessions Panel

**Definition:** Sessions with status IN ('running', 'paused') OR executions with status IN ('running', 'queued', 'paused')

**Correct Display:**

```
┌─────────────────────────────────────────────────────────┐
│ Active Sessions                                    [1]  │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ▼ Session sess-abc (running)                          │
│    ├─ root (completed) ────────────────── 1,234 tok    │
│    └─ research-agent (running) ─────────── 567 tok ●   │
│                                                         │
│  ▼ Session sess-def (paused)                           │
│    └─ root (paused) ───────────────────── 890 tok ⏸    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**Key Points:**
1. Group by SESSION, not individual executions
2. Show root even if completed (provides context)
3. Show subagent with its running status
4. Indicate which execution is currently active

### Session History Panel

**Definition:** Sessions with status IN ('completed', 'crashed', 'cancelled')

**Correct Display:**

```
┌─────────────────────────────────────────────────────────┐
│ Session History                                   [15]  │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ▶ sess-xyz (completed) ── 3 turns, 2 subagents        │
│    Jan 31, 2026 ─────────────────────── 5,678 tok      │
│                                                         │
│  ▼ sess-uvw (completed) ── 1 turn, 1 subagent          │
│    ├─ Turn 1: root → research-agent                    │
│    Jan 31, 2026 ─────────────────────── 2,345 tok      │
│                                                         │
│  ▶ sess-rst (crashed) ── 2 turns, 0 subagents          │
│    Jan 30, 2026 ─────────────────────── 1,234 tok      │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**Key Points:**
1. Group by SESSION
2. Show turn count (number of root executions)
3. Show subagent count (non-root executions)
4. Expandable to see execution tree

### Stats Display

**Current (WRONG):**
```
Running: 0    Completed: 2
```

**Correct:**
```
Sessions:    1 running, 2 completed
Executions:  1 running, 7 completed
```

---

## Part 6: Current Bugs Analysis

### Bug 1: Running Count Shows 0

**Symptom:** Research-agent is running but dashboard shows "0 running"

**Root Cause:**
```
Stats Query:  SELECT status, COUNT(*) FROM sessions GROUP BY status
              ↓
              Counts SESSIONS, not EXECUTIONS
              ↓
              Session might be marked 'completed' even if subagent running
```

**Why Session Is Completed:**
The race condition fix creates subagent execution in QUEUED status. But if the fix isn't working or there's another path, session could be marked completed.

**Verification Query:**
```sql
SELECT s.id, s.status as session_status, e.id, e.agent_id, e.status as exec_status
FROM sessions s
JOIN agent_executions e ON e.session_id = s.id
WHERE e.status = 'running';
```

### Bug 2: Root Agent Not Visible

**Symptom:** Only research-agent shown in Active Sessions, root not visible

**Root Cause:**
```typescript
// Frontend filter
const activeSessions = allSessions.filter(s =>
    ACTIVE_STATUSES.includes(s.status)  // ['running', 'paused', 'queued']
);
```

Root execution has `status = 'completed'` → filtered out.

**The Problem:** Frontend shows EXECUTIONS, not SESSIONS. When root completes, it disappears even though session is still active.

### Bug 3: Subagent Count Shows 4 (Wrong)

**Symptom:** Session shows "root with 4 subagents" when it should show fewer

**Root Cause:** The grouping logic is completely broken.

**Frontend Grouping Code:**
```typescript
const getRootConversationId = (convId: string): string => {
  const subIndex = convId.indexOf('-sub-');
  return subIndex > 0 ? convId.substring(0, subIndex) : convId;
};
```

**The Problem:**
1. Backend sends `conversation_id = session_id` (e.g., `sess-abc123`)
2. Frontend looks for `-sub-` suffix
3. `sess-abc123` has no `-sub-` suffix
4. ALL executions with same session_id get grouped together
5. Multiple sessions might accidentally group together if IDs have similar prefixes

**Legacy vs New Format:**
```
OLD FORMAT (expected by frontend):
  conv-abc123           → root conversation
  conv-abc123-sub-001   → subagent 1
  conv-abc123-sub-002   → subagent 2

NEW FORMAT (actual):
  session_id: sess-abc123
  All executions have: conversation_id = sess-abc123
  No -sub- suffix exists!
```

### Bug 4: Stats Count Sessions, UI Shows Executions

**Symptom:** Mismatch between stats panel and active sessions list

**Root Cause:**
```
Stats API:     Counts SESSIONS by status
Active Panel:  Shows EXECUTIONS filtered by status
```

**Example:**
```
Database State:
  Session: sess-abc (status=running)
    ├─ exec-001 root (completed)
    └─ exec-002 research (running)

Stats show:     1 running (session)
Panel shows:    1 item (research-agent execution)
Expected:       Both should show context of the session
```

---

## Part 7: API Analysis

### Legacy API: DELETE ENTIRELY

**Status:** NUKE IT. Not MVP yet, no backward compatibility needed.

**Files to Delete/Modify:**
- `gateway/src/http/handlers.rs`: Remove `LegacyExecutionSession` struct
- `gateway/src/http/handlers.rs`: Remove legacy endpoint handlers
- `apps/ui/src/services/transport/types.ts`: Remove `ExecutionSession` (legacy type)
- `apps/ui/src/features/ops/WebOpsDashboard.tsx`: Remove `getRootConversationId()` and all `-sub-` logic

**Why Legacy Existed:**
- Historical artifact from before Session/Execution model
- Tried to map new model to old field names
- Created confusion and bugs

**Decision:** Clean slate. Use V2 API only.

---

### V2 API: GET /api/executions/v2/sessions/full (THE ONLY API)

**Returns:** `SessionWithExecutions[]` (sessions with nested executions)

**Structure:**
```rust
SessionWithExecutions {
    session: Session {
        id: "sess-abc",
        status: SessionStatus::Running,
        root_agent_id: "root",
        ...
    },
    executions: Vec<AgentExecution>,  // All executions, properly typed
    subagent_count: u32,              // Pre-computed
}
```

**Advantages:**
1. Returns SESSIONS (correct entity)
2. Executions properly nested
3. No misleading field names
4. Subagent count pre-computed
5. Uses proper types, not strings

### Stats API: GET /api/executions/stats/counts

**Returns:** `HashMap<String, u64>`
```json
{
  "running": 1,      // Sessions with status='running'
  "completed": 15,   // Sessions with status='completed'
  "crashed": 2,
  "paused": 0,
  "today_count": 5,
  "today_tokens": 12345
}
```

**Problem:** Only counts sessions. No execution-level stats.

**Needed:**
```json
{
  "sessions_running": 1,
  "sessions_completed": 15,
  "executions_running": 2,
  "executions_queued": 1,
  "executions_completed": 45
}
```

---

## Part 8: Use Cases

### Use Case 1: Simple Chat (No Delegation)

```
User: "Hello"
  └─ Session created (running)
  └─ Execution created (root, queued → running)
  └─ Agent responds
  └─ Execution completed
  └─ try_complete_session() → Session completed

Dashboard Should Show:
  Active: (empty)
  History: 1 session, 1 turn, 0 subagents
  Stats: 0 running, 1 completed
```

### Use Case 2: Multi-Turn Chat (No Delegation)

```
User: "Hello"
  └─ Execution 1 created, completes
User: "Tell me more"
  └─ Execution 2 created, completes
User: "Thanks"
  └─ Execution 3 created, completes

Session: sess-abc
├─ exec-001 (root, completed)
├─ exec-002 (root, completed)
└─ exec-003 (root, completed)

Dashboard Should Show:
  Active: (empty)
  History: 1 session, 3 turns, 0 subagents
  Stats: 0 running, 1 completed
```

### Use Case 3: Single Delegation

```
User: "Research topic X"
  └─ Root execution created, starts
  └─ Root delegates to research-agent
  └─ Subagent execution created (QUEUED)
  └─ Root completes
  └─ try_complete_session() → subagent is QUEUED → session stays running
  └─ Subagent starts (RUNNING)
  └─ Subagent completes
  └─ try_complete_session() → no pending → session completes

Dashboard While Subagent Running:
  Active: 1 session
    └─ root (completed)
    └─ research-agent (running) ●
  Stats: 1 running session, 1 running execution

Dashboard After Complete:
  Active: (empty)
  History: 1 session, 1 turn, 1 subagent
  Stats: 0 running, 1 completed
```

### Use Case 4: Nested Delegation

```
User: "Complex task"
  └─ Root delegates to planner
    └─ Planner delegates to researcher
      └─ Researcher completes
    └─ Planner delegates to writer
      └─ Writer completes
    └─ Planner completes
  └─ Root completes
  └─ Session completes

Session: sess-abc
├─ exec-001 root (completed)
├─ exec-002 planner (completed) [parent: exec-001]
├─ exec-003 researcher (completed) [parent: exec-002]
└─ exec-004 writer (completed) [parent: exec-002]

Dashboard Should Show:
  History: 1 session, 1 turn, 3 subagents
  Expanded:
    root
    └─ planner
       ├─ researcher
       └─ writer
```

### Use Case 5: Parallel Delegation (Future)

```
Root delegates to agent-A AND agent-B simultaneously
Both run in parallel
Both complete
Session completes

Session: sess-abc
├─ exec-001 root (completed)
├─ exec-002 agent-A (completed) [parent: exec-001, type: parallel]
└─ exec-003 agent-B (completed) [parent: exec-001, type: parallel]
```

### Use Case 6: Subagent Crashes

```
User: "Do task"
  └─ Root delegates to subagent
  └─ Subagent crashes
  └─ Error callback sent to root
  └─ Subagent marked CRASHED
  └─ Session still running (root can handle error)
  └─ Root completes
  └─ Session completes

Dashboard While Crashed:
  Active: 1 session
    └─ root (running)
    └─ subagent (crashed) ✗
```

### Use Case 7: Parallel Sessions (Multiple Concurrent Chats)

**Scenario:** User has one chat running with delegation, then opens a NEW chat window to start a second independent session.

```
TIME    CHAT WINDOW 1                    CHAT WINDOW 2
────    ─────────────────────────────    ─────────────────────────────
T0      User: "Research topic A"
        └─ Session sess-001 created
        └─ exec-001 root (running)

T1      Root delegates
        └─ exec-002 research-agent
           (queued → running)
        └─ exec-001 completes

T2                                       User clicks "New Chat"
                                         └─ Session sess-002 created
                                         User: "Summarize B"
                                         └─ exec-003 root (running)

T3      exec-002 still running           exec-003 delegates
                                         └─ exec-004 summarizer (queued)
                                         └─ exec-003 completes

T4      exec-002 completes               exec-004 starts (running)
        └─ sess-001 completes

T5                                       exec-004 completes
                                         └─ sess-002 completes
```

**Database State at T3:**
```
sessions:
  ┌─────────────────────────────────────────────────────────┐
  │ sess-001 (running)  │ sess-002 (running)                │
  └─────────────────────────────────────────────────────────┘

agent_executions:
  ┌─────────────────────────────────────────────────────────┐
  │ Session: sess-001                                        │
  │   exec-001 root        (completed)                       │
  │   exec-002 research    (running)    parent: exec-001     │
  ├─────────────────────────────────────────────────────────┤
  │ Session: sess-002                                        │
  │   exec-003 root        (completed)                       │
  │   exec-004 summarizer  (queued)     parent: exec-003     │
  └─────────────────────────────────────────────────────────┘
```

**Dashboard Should Show at T3:**

```
┌─────────────────────────────────────────────────────────────┐
│ Active Sessions                                         [2] │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ▼ Session sess-001 (running)                              │
│    ├─ root (completed) ──────────────────────── 1,234 tok  │
│    └─ research-agent (running) ──────────────── 567 tok ●  │
│                                                             │
│  ▼ Session sess-002 (running)                              │
│    ├─ root (completed) ──────────────────────── 890 tok    │
│    └─ summarizer (queued) ─────────────────────── 0 tok ⏳  │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ Stats: 2 sessions running, 2 executions active (1 running,  │
│        1 queued), 2 executions completed                    │
└─────────────────────────────────────────────────────────────┘
```

**Critical Requirements:**

1. **Session Isolation:** sess-002's executions MUST NOT appear under sess-001
2. **Independent Lifecycles:** Each session completes independently when its executions finish
3. **Accurate Aggregation:** Stats must aggregate across ALL sessions
4. **No Cross-Contamination:** Grouping must use `session_id`, not string manipulation

**Why Legacy Grouping Fails Here:**

```typescript
// BROKEN: Legacy grouping
const getRootConversationId = (convId: string): string => {
  const subIndex = convId.indexOf('-sub-');
  return subIndex > 0 ? convId.substring(0, subIndex) : convId;
};

// Both sessions return their session_id unchanged (no -sub-)
// If session IDs accidentally share a prefix, they could group together!
// Example: if IDs were generated poorly like "sess-abc-001" and "sess-abc-002"
// Both would group as "sess-abc-001" and "sess-abc-002" (no issue)
// BUT frontend expects conv-xxx-sub-yyy format which doesn't exist
```

**V2 API Solves This:**

```json
GET /api/executions/v2/sessions/full

[
  {
    "session": { "id": "sess-001", "status": "running", ... },
    "executions": [
      { "id": "exec-001", "agent_id": "root", "status": "completed", ... },
      { "id": "exec-002", "agent_id": "research", "status": "running", ... }
    ],
    "subagent_count": 1
  },
  {
    "session": { "id": "sess-002", "status": "running", ... },
    "executions": [
      { "id": "exec-003", "agent_id": "root", "status": "completed", ... },
      { "id": "exec-004", "agent_id": "summarizer", "status": "queued", ... }
    ],
    "subagent_count": 1
  }
]
```

Data comes pre-grouped by session. No client-side grouping needed.

### Use Case 8: High Concurrency (Stress Test)

**Scenario:** Multiple users (or one power user with many tabs) running many parallel sessions.

```
Database State:
  sess-001: root → researcher → analyst (3 levels deep)
  sess-002: root → summarizer
  sess-003: root (no delegation)
  sess-004: root → planner → [writer, reviewer] (parallel subagents)
  sess-005: root (just started)

Stats Should Show:
  Sessions:   3 running, 2 completed
  Executions: 4 running, 2 queued, 8 completed
```

**Key Insight:** The V2 API returns `SessionWithExecutions[]` which naturally handles this because each session is its own object with its own executions array.

---

## Part 9: Required Fixes

### Fix 0: Extend Data Model for Multi-Channel (Future-Ready)

**Purpose:** Enable the full orchestration hub vision

**Changes to `SessionStatus` enum:**
```rust
pub enum SessionStatus {
    Queued,     // NEW: Waiting for resources/priority
    Running,
    Paused,
    Completed,
    Crashed,
}
```

**Changes to `Session` struct:**
```rust
pub struct Session {
    // ... existing fields ...

    /// Trigger source (web, cron, signal, email, api, webhook)
    pub source: String,  // Or enum TriggerSource

    /// Priority for queue ordering (lower = higher priority, None = default)
    pub priority: Option<u32>,

    /// External reference ID (email message ID, webhook event ID, etc.)
    pub external_ref: Option<String>,
}
```

**Changes to `DashboardStats`:**
```rust
pub struct DashboardStats {
    // Session counts by status
    pub sessions_queued: u64,    // NEW
    pub sessions_running: u64,
    pub sessions_paused: u64,
    pub sessions_completed: u64,
    pub sessions_crashed: u64,

    // Session counts by source (NEW)
    pub sessions_by_source: HashMap<String, u64>,  // {"web": 3, "cron": 1, ...}

    // Execution counts
    pub executions_queued: u64,
    pub executions_running: u64,
    pub executions_completed: u64,

    // Daily stats
    pub today_count: u64,
    pub today_tokens: u64,
}
```

**Database Schema Changes:**
```sql
-- Add columns to sessions table
ALTER TABLE sessions ADD COLUMN source TEXT DEFAULT 'web';
ALTER TABLE sessions ADD COLUMN priority INTEGER;
ALTER TABLE sessions ADD COLUMN external_ref TEXT;

-- Update status check constraint to include 'queued'
-- (implementation depends on SQLite version)
```

**Note:** These are design requirements. Implementation can be phased:
- Phase 1: Fix current dashboard bugs (use V2 API, fix stats)
- Phase 2: Add source tracking
- Phase 3: Add session-level queuing with priority

---

### Fix 1: Use V2 API ONLY (Delete Legacy Endpoints)

**Action:**
- Frontend calls `/api/executions/v2/sessions/full` ONLY
- DELETE `/api/executions/sessions` endpoint entirely
- DELETE all legacy handler code

**Impact:**
- Data comes pre-grouped by session
- No string manipulation needed
- Correct types throughout
- No confusion between old/new APIs

### Fix 2: Add Execution Stats

**Change:** Extend `DashboardStats` to include execution counts

```rust
pub struct DashboardStats {
    // Session counts
    pub sessions_running: u64,
    pub sessions_paused: u64,
    pub sessions_completed: u64,
    pub sessions_crashed: u64,

    // Execution counts (NEW)
    pub executions_running: u64,
    pub executions_queued: u64,
    pub executions_completed: u64,

    // Daily stats
    pub today_count: u64,
    pub today_tokens: u64,
}
```

### Fix 3: Rewrite Active Sessions Display

**Change:** Show sessions with their executions, not flat execution list

```typescript
// Group active executions by session
const activeSessionsGrouped = useMemo(() => {
  // Get unique session IDs from active executions
  const sessionIds = new Set(
    allExecutions
      .filter(e => ['running', 'queued', 'paused'].includes(e.status))
      .map(e => e.session_id)
  );

  // For each session, get all its executions
  return Array.from(sessionIds).map(sessionId => ({
    sessionId,
    executions: allExecutions.filter(e => e.session_id === sessionId)
  }));
}, [allExecutions]);
```

### Fix 4: NUKE All Legacy Code

**Delete from `WebOpsDashboard.tsx`:**
```typescript
// DELETE: Legacy grouping function
const getRootConversationId = (convId: string): string => { ... }

// DELETE: Legacy buildConversationGroups function
function buildConversationGroups(sessions: ExecutionSession[]): ConversationGroup[] { ... }

// DELETE: Any reference to 'conversation_id', 'parent_session_id' (legacy field names)
```

**Delete from `handlers.rs`:**
```rust
// DELETE: Entire LegacyExecutionSession struct
pub struct LegacyExecutionSession { ... }

// DELETE: From<AgentExecution> for LegacyExecutionSession impl

// DELETE: Legacy endpoint handlers that return LegacyExecutionSession
```

**Delete from `types.ts`:**
```typescript
// DELETE: ExecutionSession interface (legacy)
export interface ExecutionSession { ... }

// DELETE: ConversationGroup interface (legacy grouping)
export interface ConversationGroup { ... }
```

**Replace with:** V2 API types only. No mapping, no translation, no grouping logic.

### Fix 5: Fix Message Fetching

**Already Done:** `getMessages()` now routes based on ID format:
- `exec-*` → `/api/executions/{id}/messages`
- `web-*` → `/api/conversations/{id}/messages`

---

## Part 10: File Changes Required

| File | Change |
|------|--------|
| `services/execution-state/src/types.rs` | Add execution counts to `DashboardStats` |
| `services/execution-state/src/repository.rs` | Query execution counts in `get_dashboard_stats()` |
| `apps/ui/src/services/transport/http.ts` | Add `listSessionsFull()` method for V2 API |
| `apps/ui/src/services/transport/types.ts` | Add `SessionWithExecutions` type |
| `apps/ui/src/features/ops/WebOpsDashboard.tsx` | Complete rewrite of data fetching and rendering |

---

## Part 11: Verification Queries

### Check Session vs Execution Status Mismatch

```sql
SELECT
    s.id as session_id,
    s.status as session_status,
    COUNT(CASE WHEN e.status = 'running' THEN 1 END) as running_execs,
    COUNT(CASE WHEN e.status = 'queued' THEN 1 END) as queued_execs,
    COUNT(CASE WHEN e.status = 'completed' THEN 1 END) as completed_execs
FROM sessions s
LEFT JOIN agent_executions e ON e.session_id = s.id
GROUP BY s.id, s.status
HAVING (s.status = 'completed' AND (running_execs > 0 OR queued_execs > 0))
    OR (s.status = 'running' AND running_execs = 0 AND queued_execs = 0);
```

### Check Orphaned Executions

```sql
SELECT e.*
FROM agent_executions e
LEFT JOIN sessions s ON e.session_id = s.id
WHERE s.id IS NULL;
```

### Check Execution Hierarchy

```sql
WITH RECURSIVE exec_tree AS (
    SELECT id, session_id, agent_id, parent_execution_id, 0 as depth
    FROM agent_executions
    WHERE parent_execution_id IS NULL

    UNION ALL

    SELECT e.id, e.session_id, e.agent_id, e.parent_execution_id, t.depth + 1
    FROM agent_executions e
    JOIN exec_tree t ON e.parent_execution_id = t.id
)
SELECT * FROM exec_tree ORDER BY session_id, depth, id;
```

---

## Summary

The dashboard is broken because:

1. **It uses the legacy API** which returns flat executions with wrong field names
2. **It groups using string manipulation** that doesn't work with new `sess-uuid` format
3. **Stats count sessions** but UI shows executions
4. **Active panel filters executions** not sessions, losing context

**Solution: NUKE LEGACY, USE V2 ONLY**

- Delete `LegacyExecutionSession` and all legacy endpoints
- Delete `getRootConversationId()` and `-sub-` grouping logic
- Delete legacy type definitions
- Use V2 API (`/api/executions/v2/sessions/full`) exclusively
- Data comes pre-grouped by session, no client-side manipulation needed

---

## Implementation Phases

### Phase 1: Fix Current Bugs (Immediate) - NUKE LEGACY

**Goal:** Make dashboard work correctly. No backward compatibility.

| Task | Files | Action |
|------|-------|--------|
| Delete legacy API | `handlers.rs` | Remove `LegacyExecutionSession`, legacy endpoints |
| Delete legacy types | `types.ts` | Remove `ExecutionSession` (old type) |
| Delete legacy grouping | `WebOpsDashboard.tsx` | Remove `getRootConversationId()`, `-sub-` logic |
| Use V2 API only | `http.ts` | Point to `/api/executions/v2/sessions/full` |
| Add proper types | `types.ts` | Add `SessionWithExecutions` from V2 |
| Rewrite dashboard | `WebOpsDashboard.tsx` | Session-centric, no string manipulation |
| Add execution stats | `types.rs`, `repository.rs` | Both session AND execution counts |

### Phase 2: Multi-Channel Foundation (Near-term)

**Goal:** Enable sessions from different sources

| Task | Files | Impact |
|------|-------|--------|
| Add `source` to Session | `types.rs`, schema | Track trigger channel |
| Display source badge | `WebOpsDashboard.tsx` | Visual indicator |
| Filter by source | `WebOpsDashboard.tsx` | "Show only cron jobs" |
| Stats by source | `repository.rs`, `types.rs` | Breakdown in stats panel |

### Phase 3: Session Queuing (Future)

**Goal:** Resource management, priority ordering

| Task | Files | Impact |
|------|-------|--------|
| Add `Queued` to SessionStatus | `types.rs`, schema | Session-level queuing |
| Add `priority` field | `types.rs`, schema | Queue ordering |
| Queue manager service | New service | Enforce max concurrency |
| Queued sessions panel | `WebOpsDashboard.tsx` | Show waiting sessions |

### Phase 4: External Integrations (Future)

**Goal:** Cron, Signal, Email, Webhook triggers

| Task | Files | Impact |
|------|-------|--------|
| Cron trigger service | New service | Scheduled sessions |
| Webhook endpoint | New handler | External event triggers |
| Email ingestion | New service | Email → session |
| `external_ref` tracking | `types.rs` | Link to source event |

---

## Design Principles

1. **Session is the unit of work** - Dashboard shows sessions, not raw executions
2. **Source-agnostic core** - Same session/execution model regardless of trigger
3. **Execution hierarchy within session** - Root → subagents tree structure
4. **Independent lifecycles** - Sessions don't affect each other
5. **Stats at both levels** - Session counts AND execution counts
6. **Future-ready schema** - Add fields now even if unused initially
