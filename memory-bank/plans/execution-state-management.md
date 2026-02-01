# Execution State Management

## Problem Statement

Agent executions have no persistent state tracking. If the daemon crashes while an agent is running, the execution is lost with no way to resume.

## Goals

1. **Crash Recovery**: Resume interrupted executions after daemon restart
2. **User Control**: Pause, resume, and cancel executions
3. **Visibility**: Track execution state for Command Control panel
4. **Cascade Control**: Pause/cancel propagates to subagents

---

## Architecture Placement

Following the layer structure in AGENTS.md:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ apps/ui/                                                                │
│   └── features/command/           UI components for Command Control     │
├─────────────────────────────────────────────────────────────────────────┤
│ gateway/                                                                │
│   ├── execution/runner.rs         Emit state changes, save checkpoints │
│   ├── websocket/handler.rs        Pause/resume/cancel WebSocket cmds   │
│   └── http/execution.rs           HTTP API for session management      │
├─────────────────────────────────────────────────────────────────────────┤
│ services/                                                               │
│   └── execution-state/  [NEW]     Standalone state & token tracking    │
│       ├── types.rs                Session, Checkpoint, TokenMetrics    │
│       ├── repository.rs           Database operations                  │
│       ├── service.rs              Business logic                       │
│       └── handlers.rs             HTTP handlers                        │
├─────────────────────────────────────────────────────────────────────────┤
│ runtime/                                                                │
│   └── agent-runtime/executor.rs   Track tokens, emit to callback       │
├─────────────────────────────────────────────────────────────────────────┤
│ framework/                                                              │
│   └── zero-core/events.rs         Add TokenUpdate event type           │
└─────────────────────────────────────────────────────────────────────────┘
```

### Why a New Service?

The `api-logs` service handles **historical log records**. Execution state is different:
- **Mutable state** (RUNNING → PAUSED → RUNNING)
- **Token metrics** (continuously updated counters)
- **Checkpoints** (restore points for recovery)

Creating `services/execution-state/` keeps concerns separated and follows the existing service pattern.

---

## Execution States

```
QUEUED → RUNNING → PAUSED ⇄ RUNNING → COMPLETED
                 → CRASHED ⇄ RUNNING
                 → CANCELLED
```

| State | Description | Can Transition To |
|-------|-------------|-------------------|
| `Queued` | Created, not started | Running |
| `Running` | Actively executing | Paused, Completed, Crashed, Cancelled |
| `Paused` | User paused | Running, Cancelled |
| `Crashed` | Daemon died mid-execution | Running (resume), Cancelled |
| `Cancelled` | User cancelled | (terminal) |
| `Completed` | Finished successfully | (terminal) |

---

## New Service: `services/execution-state/`

### Cargo.toml

```toml
[package]
name = "execution-state"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
rusqlite = "0.32"
axum = "0.8"
```

### types.rs

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Queued,
    Running,
    Paused,
    Crashed,
    Cancelled,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSession {
    pub id: String,
    pub conversation_id: String,
    pub agent_id: String,
    pub parent_session_id: Option<String>,
    pub status: ExecutionStatus,

    // Timing
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    // Tokens
    pub tokens_in: u64,
    pub tokens_out: u64,

    // Recovery
    pub checkpoint: Option<Checkpoint>,

    // Outcome
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub llm_turn: u32,
    pub last_message_id: String,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub context_snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUpdate {
    pub session_id: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTokenSummary {
    pub date: String,
    pub total_in: u64,
    pub total_out: u64,
    pub session_count: u64,
    pub failure_count: u64,
}
```

### repository.rs (Trait Pattern)

```rust
use rusqlite::Connection;

/// Trait for database access - gateway implements this
pub trait StateDbProvider: Send + Sync {
    fn get_connection(&self) -> &Connection;
}

pub struct StateRepository<D: StateDbProvider> {
    db: Arc<D>,
}

impl<D: StateDbProvider> StateRepository<D> {
    // Session CRUD
    pub fn create_session(&self, session: &ExecutionSession) -> Result<()>;
    pub fn get_session(&self, id: &str) -> Result<ExecutionSession>;
    pub fn update_status(&self, id: &str, status: ExecutionStatus) -> Result<()>;
    pub fn update_tokens(&self, id: &str, tokens_in: u64, tokens_out: u64) -> Result<()>;
    pub fn save_checkpoint(&self, id: &str, checkpoint: &Checkpoint) -> Result<()>;

    // Queries
    pub fn get_by_status(&self, status: ExecutionStatus) -> Result<Vec<ExecutionSession>>;
    pub fn get_children(&self, parent_id: &str) -> Result<Vec<ExecutionSession>>;
    pub fn get_resumable(&self) -> Result<Vec<ExecutionSession>>;  // Paused + Crashed
    pub fn get_running(&self) -> Result<Vec<ExecutionSession>>;

    // Aggregates
    pub fn get_daily_summary(&self, date: &str) -> Result<DailyTokenSummary>;
}
```

### Schema SQL

```sql
CREATE TABLE IF NOT EXISTS execution_sessions (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    parent_session_id TEXT,

    status TEXT NOT NULL DEFAULT 'queued',

    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,

    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,

    checkpoint TEXT,  -- JSON
    error TEXT,

    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_session_id) REFERENCES execution_sessions(id) ON DELETE SET NULL
);

CREATE INDEX idx_sessions_status ON execution_sessions(status);
CREATE INDEX idx_sessions_conversation ON execution_sessions(conversation_id);
CREATE INDEX idx_sessions_parent ON execution_sessions(parent_session_id);
CREATE INDEX idx_sessions_created ON execution_sessions(created_at);
```

---

## Gateway Integration

### execution/runner.rs

```rust
use execution_state::{ExecutionSession, ExecutionStatus, StateService};

impl ExecutionRunner {
    pub async fn invoke(&self, ...) -> Result<EventStream> {
        // 1. Create session record
        let session = ExecutionSession::new(conversation_id, agent_id, parent_session_id);
        self.state_service.create_session(&session)?;

        // 2. Update to running
        self.state_service.update_status(&session.id, ExecutionStatus::Running)?;

        // 3. Execute with state tracking
        let handle = ExecutionHandle::new(session.id.clone(), self.state_service.clone());
        let result = self.execute_with_tracking(handle, ...).await;

        // 4. Update final status
        match result {
            Ok(_) => self.state_service.update_status(&session.id, ExecutionStatus::Completed)?,
            Err(e) => {
                self.state_service.update_status(&session.id, ExecutionStatus::Crashed)?;
                self.state_service.set_error(&session.id, &e.to_string())?;
            }
        }
    }
}
```

### ExecutionHandle (Control Interface)

```rust
pub struct ExecutionHandle {
    session_id: String,
    state_service: Arc<StateService>,
    cancel_token: CancellationToken,
    pause_flag: Arc<AtomicBool>,
}

impl ExecutionHandle {
    pub async fn pause(&self) -> Result<()> {
        self.pause_flag.store(true, Ordering::SeqCst);
        self.state_service.update_status(&self.session_id, ExecutionStatus::Paused)?;

        // Cascade to children
        for child in self.state_service.get_children(&self.session_id)? {
            // Emit pause command for child session
        }
        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        self.pause_flag.store(false, Ordering::SeqCst);
        self.state_service.update_status(&self.session_id, ExecutionStatus::Running)?;
        Ok(())
    }

    pub async fn cancel(&self) -> Result<()> {
        self.cancel_token.cancel();
        self.state_service.update_status(&self.session_id, ExecutionStatus::Cancelled)?;

        // Cascade to children
        for child in self.state_service.get_children(&self.session_id)? {
            // Emit cancel command for child session
        }
        Ok(())
    }

    pub fn update_tokens(&self, tokens_in: u64, tokens_out: u64) -> Result<()> {
        self.state_service.update_tokens(&self.session_id, tokens_in, tokens_out)
    }

    pub fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        self.state_service.save_checkpoint(&self.session_id, checkpoint)
    }
}
```

### websocket/handler.rs

```rust
// New WebSocket commands
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientCommand {
    Invoke { ... },
    Stop { ... },

    // NEW
    Pause { session_id: String },
    Resume { session_id: String },
    Cancel { session_id: String },
}
```

---

## Runtime Integration

### agent-runtime/executor.rs

Track tokens from LLM responses and emit via callback:

```rust
impl AgentExecutor {
    async fn call_llm(&mut self, ...) -> Result<LlmResponse> {
        let response = self.llm_client.chat_completion(...).await?;

        // Track tokens
        self.tokens_in += response.usage.prompt_tokens;
        self.tokens_out += response.usage.completion_tokens;

        // Emit token update via callback
        if let Some(callback) = &self.token_callback {
            callback(TokenUpdate {
                session_id: self.session_id.clone(),
                tokens_in: self.tokens_in,
                tokens_out: self.tokens_out,
            });
        }

        Ok(response)
    }
}
```

---

## Framework Changes

### zero-core/events.rs

Add token event type:

```rust
#[derive(Debug, Clone, Serialize)]
pub enum AgentEvent {
    // Existing...
    Started { agent_id: String },
    Token { delta: String },
    ToolCall { ... },
    ToolResult { ... },
    Completed { result: String },
    Error { message: String },

    // NEW
    TokenUpdate {
        session_id: String,
        tokens_in: u64,
        tokens_out: u64
    },
    StatusChanged {
        session_id: String,
        status: String,
    },
}
```

---

## Crash Recovery

### On Daemon Startup (gateway/state.rs)

```rust
impl AppState {
    pub async fn recover_sessions(&self) -> Result<()> {
        // Mark RUNNING sessions as CRASHED
        let running = self.state_service.get_by_status(ExecutionStatus::Running)?;
        for session in running {
            self.state_service.update_status(&session.id, ExecutionStatus::Crashed)?;
            tracing::warn!("Session {} marked crashed (daemon restart)", session.id);
        }
        Ok(())
    }
}
```

### Resume Flow

```rust
pub async fn resume_session(&self, session_id: &str) -> Result<EventStream> {
    let session = self.state_service.get_session(session_id)?;

    match session.status {
        ExecutionStatus::Paused | ExecutionStatus::Crashed => {
            let checkpoint = session.checkpoint.ok_or("No checkpoint")?;

            // Restore executor state from checkpoint
            let mut executor = self.create_executor_from_checkpoint(&checkpoint)?;

            self.state_service.update_status(session_id, ExecutionStatus::Running)?;

            // Continue from checkpoint
            executor.continue_from(checkpoint.llm_turn).await
        }
        _ => Err("Session not resumable".into())
    }
}
```

---

## Files Summary

| Layer | File | Changes |
|-------|------|---------|
| **services/** | `execution-state/` | **NEW CRATE** - Session state, tokens, checkpoints |
| **gateway** | `Cargo.toml` | Add `execution-state` dependency |
| **gateway** | `database/schema.rs` | Add `execution_sessions` table |
| **gateway** | `state.rs` | Add StateService, crash recovery |
| **gateway** | `execution/runner.rs` | Emit state changes, checkpointing |
| **gateway** | `execution/handle.rs` | **NEW** - Control interface (pause/resume/cancel) |
| **gateway** | `websocket/handler.rs` | Add pause/resume/cancel commands |
| **gateway** | `http/sessions.rs` | **NEW** - Session management API |
| **runtime** | `agent-runtime/executor.rs` | Track tokens, emit updates |
| **framework** | `zero-core/events.rs` | Add TokenUpdate, StatusChanged events |

---

## Implementation Order

### Week 1: Service Foundation
1. Create `services/execution-state/` crate
2. Define types, repository trait, schema
3. Add to workspace Cargo.toml

### Week 2: Gateway Integration
4. Implement StateDbProvider in gateway
5. Wire StateService into AppState
6. Update runner to create sessions and update status

### Week 3: Token Tracking
7. Add token tracking to executor
8. Emit TokenUpdate events
9. Update tokens in database

### Week 4: Control Commands
10. Add ExecutionHandle with pause/resume/cancel
11. Add WebSocket commands
12. Implement cascade to subagents

### Week 5: Checkpointing & Recovery
13. Save checkpoints during execution
14. Add crash recovery on startup
15. Implement resume_session

---

## Verification

1. **Session lifecycle**: QUEUED → RUNNING → COMPLETED
2. **Pause/Resume**: Status changes correctly, execution pauses
3. **Cancel**: Execution stops, subagents cancelled
4. **Token tracking**: IN/OUT updated after each LLM call
5. **Crash recovery**: Kill daemon → restart → sessions marked CRASHED
6. **Resume**: Crashed session resumes from checkpoint
