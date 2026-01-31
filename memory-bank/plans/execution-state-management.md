# Execution State Management

## Problem Statement

Agent executions (root and subagents) have no persistent state tracking. If the daemon crashes or is killed while an agent is running:
- The execution is lost
- User loses context of what was happening
- No way to resume from where it stopped

## Goals

1. **Crash Recovery**: Resume interrupted executions after daemon restart
2. **User Control**: Pause, resume, and cancel executions
3. **Visibility**: Dashboard showing all execution states
4. **Cascade Control**: Pause/cancel propagates to subagents

---

## Execution States

```
                    ┌─────────────┐
                    │   QUEUED    │ ← Initial state when created
                    └──────┬──────┘
                           │ start()
                           ▼
                    ┌─────────────┐
          ┌────────►│  RUNNING    │◄────────┐
          │         └──────┬──────┘         │
          │                │                │
          │    ┌───────────┼───────────┐    │
          │    │           │           │    │
          │    ▼           ▼           ▼    │
     resume() ┌─────┐  ┌────────┐  ┌───────┐│
          │   │PAUSED│  │CRASHED │  │CANCEL-││ resume()
          │   └──┬──┘  └────────┘  │  LED  ││
          │      │                 └───────┘│
          │      │ (user action)            │
          └──────┘                          │
                                            │
                    ┌─────────────┐          │
                    │  COMPLETED  │◄─────────┘
                    └─────────────┘     (normal finish)
```

### State Definitions

| State | Description | Transitions |
|-------|-------------|-------------|
| `QUEUED` | Created but not started | → RUNNING |
| `RUNNING` | Actively executing | → PAUSED, COMPLETED, CRASHED, CANCELLED |
| `PAUSED` | User-initiated pause | → RUNNING (resume), CANCELLED |
| `CRASHED` | Daemon died during execution | → RUNNING (resume), CANCELLED |
| `CANCELLED` | User cancelled | Terminal state |
| `COMPLETED` | Finished successfully | Terminal state |

---

## Database Schema

### New Table: `execution_sessions`

```sql
CREATE TABLE execution_sessions (
    id TEXT PRIMARY KEY,                    -- Session UUID
    conversation_id TEXT NOT NULL,          -- Link to conversation
    agent_id TEXT NOT NULL,                 -- Which agent
    parent_session_id TEXT,                 -- For subagents

    -- State management
    status TEXT NOT NULL DEFAULT 'QUEUED',  -- QUEUED|RUNNING|PAUSED|CRASHED|CANCELLED|COMPLETED

    -- Timing
    created_at TEXT NOT NULL,
    started_at TEXT,
    paused_at TEXT,
    completed_at TEXT,

    -- Recovery data
    last_checkpoint TEXT,                   -- JSON: last known good state
    checkpoint_message_id TEXT,             -- Last processed message

    -- Metadata
    error_message TEXT,                     -- If crashed/cancelled with error
    metadata TEXT,                          -- JSON: additional context

    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_session_id) REFERENCES execution_sessions(id) ON DELETE SET NULL
);

CREATE INDEX idx_sessions_status ON execution_sessions(status);
CREATE INDEX idx_sessions_conversation ON execution_sessions(conversation_id);
CREATE INDEX idx_sessions_parent ON execution_sessions(parent_session_id);
```

### Checkpoint Data Structure

```json
{
  "llm_turn": 5,
  "last_message_id": "msg-123",
  "pending_tool_calls": [
    {"id": "tc-1", "name": "read_file", "args": {...}, "status": "pending"}
  ],
  "context_state": {...},
  "subagent_sessions": ["session-abc", "session-def"]
}
```

---

## Implementation Plan

### Phase 1: Core State Tracking

#### 1.1 Add ExecutionSession Model

**File:** `gateway/src/models/execution_session.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub paused_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_checkpoint: Option<Checkpoint>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub llm_turn: u32,
    pub last_message_id: String,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub context_state: Value,
    pub subagent_sessions: Vec<String>,
}
```

#### 1.2 Add Session Repository

**File:** `gateway/src/database/sessions.rs`

```rust
impl SessionRepository {
    pub fn create(&self, session: &ExecutionSession) -> Result<()>;
    pub fn update_status(&self, id: &str, status: ExecutionStatus) -> Result<()>;
    pub fn save_checkpoint(&self, id: &str, checkpoint: &Checkpoint) -> Result<()>;
    pub fn get(&self, id: &str) -> Result<ExecutionSession>;
    pub fn get_by_conversation(&self, conv_id: &str) -> Result<Vec<ExecutionSession>>;
    pub fn get_resumable(&self) -> Result<Vec<ExecutionSession>>; // CRASHED or PAUSED
    pub fn get_children(&self, parent_id: &str) -> Result<Vec<ExecutionSession>>;
}
```

#### 1.3 Integrate with Execution Runner

**File:** `gateway/src/execution/runner.rs`

Modify `invoke()` to:
1. Create ExecutionSession with status QUEUED
2. Update to RUNNING when starting
3. Save checkpoints after each LLM turn
4. Update to COMPLETED/CRASHED on finish/error

```rust
async fn invoke(&self, ...) -> Result<EventStream> {
    // Create session record
    let session = ExecutionSession::new(conversation_id, agent_id, parent_session_id);
    self.session_repo.create(&session)?;

    // Update to running
    self.session_repo.update_status(&session.id, ExecutionStatus::Running)?;

    // Execute with checkpointing
    let result = self.execute_with_checkpoints(session.id, ...).await;

    // Update final status
    match result {
        Ok(_) => self.session_repo.update_status(&session.id, ExecutionStatus::Completed)?,
        Err(e) => {
            self.session_repo.update_status(&session.id, ExecutionStatus::Crashed)?;
            self.session_repo.set_error(&session.id, &e.to_string())?;
        }
    }
}
```

### Phase 2: Pause/Resume/Cancel

#### 2.1 Add Control Commands

**File:** `gateway/src/websocket/handler.rs`

New WebSocket commands:
```typescript
{ type: "pause", session_id: string }
{ type: "resume", session_id: string }
{ type: "cancel", session_id: string }
```

#### 2.2 ExecutionHandle Enhancement

**File:** `gateway/src/execution/runner.rs`

```rust
pub struct ExecutionHandle {
    session_id: String,
    cancel_token: CancellationToken,
    pause_signal: Arc<AtomicBool>,
    session_repo: Arc<SessionRepository>,
}

impl ExecutionHandle {
    pub async fn pause(&self) -> Result<()> {
        self.pause_signal.store(true, Ordering::SeqCst);
        self.session_repo.update_status(&self.session_id, ExecutionStatus::Paused)?;
        // Pause all child sessions
        for child in self.session_repo.get_children(&self.session_id)? {
            self.pause_child(&child.id).await?;
        }
        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        self.pause_signal.store(false, Ordering::SeqCst);
        self.session_repo.update_status(&self.session_id, ExecutionStatus::Running)?;
        // Resume is handled by executor checking pause_signal
        Ok(())
    }

    pub async fn cancel(&self) -> Result<()> {
        self.cancel_token.cancel();
        self.session_repo.update_status(&self.session_id, ExecutionStatus::Cancelled)?;
        // Cancel all child sessions
        for child in self.session_repo.get_children(&self.session_id)? {
            self.cancel_child(&child.id).await?;
        }
        Ok(())
    }
}
```

#### 2.3 Executor Pause Loop

```rust
async fn execute_llm_loop(&self, ...) -> Result<()> {
    loop {
        // Check pause signal
        while self.handle.is_paused() {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if self.handle.is_cancelled() {
                return Err(ExecutionCancelled);
            }
        }

        // Check cancel
        if self.handle.is_cancelled() {
            return Err(ExecutionCancelled);
        }

        // Normal LLM call...
        let response = self.llm_client.call(...).await?;

        // Save checkpoint after each turn
        self.save_checkpoint()?;
    }
}
```

### Phase 3: Crash Recovery

#### 3.1 Startup Recovery Check

**File:** `gateway/src/state.rs`

On daemon startup:
```rust
impl AppState {
    pub async fn recover_crashed_sessions(&self) -> Result<()> {
        // Find sessions that were RUNNING when daemon died
        let crashed = self.session_repo.get_by_status(ExecutionStatus::Running)?;

        for session in crashed {
            // Mark as CRASHED (not RUNNING anymore)
            self.session_repo.update_status(&session.id, ExecutionStatus::Crashed)?;
            tracing::warn!("Session {} marked as crashed (daemon restart)", session.id);
        }

        Ok(())
    }
}
```

#### 3.2 Resume from Checkpoint

**File:** `gateway/src/execution/runner.rs`

```rust
pub async fn resume_session(&self, session_id: &str) -> Result<EventStream> {
    let session = self.session_repo.get(session_id)?;

    match session.status {
        ExecutionStatus::Paused | ExecutionStatus::Crashed => {
            // Load checkpoint
            let checkpoint = session.last_checkpoint
                .ok_or("No checkpoint available")?;

            // Restore state
            let mut executor = self.create_executor_from_checkpoint(&checkpoint).await?;

            // Update status
            self.session_repo.update_status(session_id, ExecutionStatus::Running)?;

            // Continue execution from checkpoint
            executor.continue_from(checkpoint.llm_turn).await
        }
        _ => Err("Session not resumable".into())
    }
}
```

### Phase 4: HTTP API

**File:** `gateway/src/http/sessions.rs`

```rust
// GET /api/sessions - List sessions with filters
// GET /api/sessions/:id - Get session details
// POST /api/sessions/:id/pause - Pause session
// POST /api/sessions/:id/resume - Resume session
// POST /api/sessions/:id/cancel - Cancel session
// DELETE /api/sessions/:id - Delete session record
// GET /api/sessions/resumable - Get all paused/crashed sessions
```

---

## UI Integration

### Sessions Dashboard

New page: `/sessions` showing:

1. **Active Sessions** - Currently RUNNING
2. **Paused Sessions** - User-paused, can resume
3. **Crashed Sessions** - Need attention, can resume or delete
4. **Recent Completed** - History

Each session card shows:
- Agent name
- Status badge (color-coded)
- Duration / time paused
- Last activity
- Actions: Pause/Resume/Cancel/Delete

### Real-time Updates

WebSocket events for session state changes:
```typescript
{ type: "session_state_changed", session_id: string, status: string }
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `gateway/src/database/schema.rs` | Add execution_sessions table |
| `gateway/src/database/sessions.rs` | **NEW** Session repository |
| `gateway/src/models/execution_session.rs` | **NEW** Session model |
| `gateway/src/execution/runner.rs` | Session lifecycle, checkpointing |
| `gateway/src/websocket/handler.rs` | Pause/resume/cancel commands |
| `gateway/src/http/sessions.rs` | **NEW** Session HTTP API |
| `gateway/src/state.rs` | Startup recovery |
| `apps/ui/src/features/sessions/` | **NEW** Sessions dashboard |

---

## Implementation Order

1. **Week 1**: Core state tracking
   - Database schema
   - Session model and repository
   - Basic status updates in runner

2. **Week 2**: Checkpointing
   - Checkpoint data structure
   - Save checkpoints during execution
   - Crash detection on startup

3. **Week 3**: Control commands
   - Pause/resume/cancel
   - Cascade to subagents
   - WebSocket commands

4. **Week 4**: Resume from checkpoint
   - Load checkpoint
   - Restore executor state
   - Continue execution

5. **Week 5**: UI
   - Sessions dashboard
   - Real-time status updates
   - Action buttons

---

## Verification

### Test Cases

1. **Normal execution**: Status transitions QUEUED → RUNNING → COMPLETED
2. **User pause**: RUNNING → PAUSED → RUNNING (resume) → COMPLETED
3. **User cancel**: RUNNING → CANCELLED
4. **Crash recovery**: Kill daemon while RUNNING → restart → status is CRASHED → resume → COMPLETED
5. **Subagent cascade**: Pause parent → all children paused

### Manual Testing

```bash
# Start a long-running agent task
# Kill daemon: Ctrl+C or kill -9
# Restart daemon
# Check /api/sessions/resumable
# Resume session via UI or API
# Verify it continues from checkpoint
```
