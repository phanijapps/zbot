# Smart Session Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a subagent crashes, resume retries only that subagent instead of restarting the root agent.

**Architecture:** Add `child_session_id` column to `agent_executions` to link delegated executions to their child sessions. Smart resume in `ExecutionRunner::resume()` detects crashed subagents, reactivates root session/execution, and re-spawns only the crashed subagent using its child session's message history.

**Tech Stack:** Rust (SQLite, tokio), TypeScript/React (UI)

---

### Task 1: Add `child_session_id` Column to Schema and Model

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs:9` (bump version), `:193-199` (add migration)
- Modify: `services/execution-state/src/types.rs:432-481` (struct), `:505-528` (constructor)
- Modify: `services/execution-state/src/repository.rs:392-420` (INSERT), `:509-526` (SELECT), `:827-848` (row mapping)
- Test: `services/execution-state/src/repository.rs` (existing test module)

- [ ] **Step 1: Add field to `AgentExecution` struct**

In `services/execution-state/src/types.rs`, add after the `log_path` field (line ~478):

```rust
    /// Child session ID (for delegated executions only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_session_id: Option<String>,
```

- [ ] **Step 2: Update `new_root()` constructor**

In `services/execution-state/src/types.rs`, add `child_session_id: None,` to the `new_root()` constructor (line ~501, after `log_path: None`).

- [ ] **Step 3: Update `new_delegated()` constructor**

In `services/execution-state/src/types.rs`, add `child_session_id: None,` to the `new_delegated()` constructor (line ~527, after `log_path: None`).

- [ ] **Step 4: Add migration in schema.rs**

In `gateway/gateway-database/src/schema.rs`:

Bump `SCHEMA_VERSION` from 14 to 15 (line 9).

Add migration block after the v14 migration (after line ~199):

```rust
// v14 → v15: Add child_session_id to agent_executions for smart resume
if version < 15 {
    let _ = conn.execute(
        "ALTER TABLE agent_executions ADD COLUMN child_session_id TEXT",
        [],
    );
}
```

- [ ] **Step 5: Add `child_session_id` to CREATE TABLE**

In `gateway/gateway-database/src/schema.rs`, in the `agent_executions` CREATE TABLE statement (line ~270-290), add before the FOREIGN KEY lines:

```sql
child_session_id TEXT,
```

And add a foreign key:

```sql
FOREIGN KEY (child_session_id) REFERENCES sessions(id) ON DELETE SET NULL
```

- [ ] **Step 6: Update repository INSERT**

In `services/execution-state/src/repository.rs`, update `create_execution()` (line ~392-420):

Add `child_session_id` to the column list and VALUES placeholders (as the 15th column), and add to params:

```rust
execution.child_session_id,
```

- [ ] **Step 7: Update repository SELECT columns**

In `services/execution-state/src/repository.rs`, update ALL SELECT statements that read from `agent_executions` to include `child_session_id` as the 15th column. This affects:
- `get_root_execution()` (line ~509)
- Any other `SELECT ... FROM agent_executions` queries

Search for all occurrences of `SELECT id, session_id, agent_id` in the repository to find them all.

- [ ] **Step 8: Update `row_to_execution()`**

In `services/execution-state/src/repository.rs` (line ~827-848), add at the end of the struct construction:

```rust
child_session_id: row.get(14)?,
```

- [ ] **Step 9: Verify compilation**

Run: `cargo check -p execution-state -p gateway-database`
Expected: compiles with no errors (may have warnings about unused field)

- [ ] **Step 10: Commit**

```bash
git add services/execution-state/src/types.rs services/execution-state/src/repository.rs gateway/gateway-database/src/schema.rs
git commit -m "feat: add child_session_id column to agent_executions"
```

---

### Task 2: Persist `child_session_id` During Delegation Spawn

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs:66-103`
- Modify: `services/execution-state/src/repository.rs` (new update method)
- Modify: `services/execution-state/src/service.rs` (new service method)

- [ ] **Step 1: Add `set_child_session_id()` to repository**

In `services/execution-state/src/repository.rs`, add a new method:

```rust
pub fn set_child_session_id(&self, execution_id: &str, child_session_id: &str) -> Result<(), String> {
    self.db.with_connection(|conn| {
        conn.execute(
            "UPDATE agent_executions SET child_session_id = ?1 WHERE id = ?2",
            params![child_session_id, execution_id],
        )?;
        Ok(())
    })
}
```

- [ ] **Step 2: Add `set_child_session_id()` to service**

In `services/execution-state/src/service.rs`, add:

```rust
pub fn set_child_session_id(&self, execution_id: &str, child_session_id: &str) -> Result<(), String> {
    self.repo.set_child_session_id(execution_id, child_session_id)
}
```

- [ ] **Step 3: Set `child_session_id` in `spawn_delegated_agent()`**

In `gateway/gateway-execution/src/delegation/spawn.rs`, after the child session is created and the execution_id is established (after line 93), add:

```rust
// Link the pre-created execution to its child session (for smart resume)
if let Err(e) = state_service.set_child_session_id(&execution_id, &child_session_id) {
    tracing::warn!("Failed to set child_session_id on execution: {}", e);
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p execution-state -p gateway-execution`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/delegation/spawn.rs services/execution-state/src/repository.rs services/execution-state/src/service.rs
git commit -m "feat: persist child_session_id during delegation spawn"
```

---

### Task 3: Add Service Methods for Smart Resume

**Files:**
- Modify: `services/execution-state/src/repository.rs` (new query)
- Modify: `services/execution-state/src/service.rs` (new methods + update `resume_session`)
- Test: `services/execution-state/src/service.rs` (existing test module at bottom of file)

- [ ] **Step 1: Write test for `get_last_crashed_subagent()`**

In `services/execution-state/src/service.rs`, add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_get_last_crashed_subagent() {
    let service = setup_service();
    let session = mock_running_session();
    service.create_session(&session).unwrap();

    // Create root execution
    let root_exec = AgentExecution::new_root(&session.id, "root");
    service.create_execution(&root_exec).unwrap();

    // Create completed subagent
    let mut sub1 = AgentExecution::new_delegated(
        &session.id, "planner", &root_exec.id, DelegationType::Sequential, "Plan task",
    );
    service.create_execution(&sub1).unwrap();
    service.complete_execution(&sub1.id).unwrap();

    // Create crashed subagent with child_session_id
    let child_session = Session::new_child("researcher", &session.id);
    service.create_session_from(&child_session).unwrap();

    let mut sub2 = AgentExecution::new_delegated(
        &session.id, "researcher", &root_exec.id, DelegationType::Sequential, "Research task",
    );
    service.create_execution(&sub2).unwrap();
    service.set_child_session_id(&sub2.id, &child_session.id).unwrap();
    service.start_execution(&sub2.id).unwrap();
    service.crash_execution(&sub2.id, "LLM 500 error").unwrap();

    // Should find the crashed researcher
    let crashed = service.get_last_crashed_subagent(&session.id).unwrap();
    assert!(crashed.is_some());
    let crashed = crashed.unwrap();
    assert_eq!(crashed.agent_id, "researcher");
    assert_eq!(crashed.child_session_id, Some(child_session.id));
}

#[test]
fn test_get_last_crashed_subagent_none_when_root_only() {
    let service = setup_service();
    let session = mock_running_session();
    service.create_session(&session).unwrap();

    let root_exec = AgentExecution::new_root(&session.id, "root");
    service.create_execution(&root_exec).unwrap();
    service.crash_execution(&root_exec.id, "LLM 500 error").unwrap();

    // Root crash should return None (not a subagent)
    let crashed = service.get_last_crashed_subagent(&session.id).unwrap();
    assert!(crashed.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p execution-state test_get_last_crashed_subagent -- --nocapture`
Expected: FAIL — method `get_last_crashed_subagent` not found

- [ ] **Step 3: Add `get_last_crashed_subagent()` to repository**

In `services/execution-state/src/repository.rs`, add:

```rust
pub fn get_last_crashed_subagent(&self, session_id: &str) -> Result<Option<AgentExecution>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, agent_id, parent_execution_id,
                    delegation_type, task, status,
                    started_at, completed_at,
                    tokens_in, tokens_out, checkpoint, error, log_path, child_session_id
             FROM agent_executions
             WHERE session_id = ? AND status = 'crashed' AND parent_execution_id IS NOT NULL
             ORDER BY started_at DESC
             LIMIT 1",
        )?;

        let execution = stmt
            .query_row(params![session_id], |row| Self::row_to_execution(row))
            .optional()?;

        Ok(execution)
    })
}
```

- [ ] **Step 4: Add `get_last_crashed_subagent()` to service**

In `services/execution-state/src/service.rs`, add:

```rust
/// Find the most recently crashed subagent execution for a session.
/// Returns None if only the root execution crashed or no crashes exist.
pub fn get_last_crashed_subagent(&self, session_id: &str) -> Result<Option<AgentExecution>, String> {
    self.repo.get_last_crashed_subagent(session_id)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p execution-state test_get_last_crashed_subagent -- --nocapture`
Expected: PASS

- [ ] **Step 6: Update `resume_session()` to accept crashed sessions**

In `services/execution-state/src/service.rs`, find `resume_session()` (line ~141-150). Change the status check from:

```rust
if session.status != SessionStatus::Paused {
    return Err(format!("Cannot resume session in {} state", session.status.as_str()));
}
```

To:

```rust
if session.status != SessionStatus::Paused && session.status != SessionStatus::Crashed {
    return Err(format!("Cannot resume session in {} state", session.status.as_str()));
}
```

- [ ] **Step 7: Verify compilation and tests**

Run: `cargo test -p execution-state -- --nocapture`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add services/execution-state/src/repository.rs services/execution-state/src/service.rs
git commit -m "feat: add get_last_crashed_subagent query and allow resume of crashed sessions"
```

---

### Task 4: Smart Resume Logic in `ExecutionRunner::resume()`

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs:1099-1110` (resume method)
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs` (extract re-spawn helper)

This is the core change. The current `resume()` method just unpauses handles. The new version detects crashed subagents and re-spawns them.

- [ ] **Step 1: Understand the current `resume()` signature and context**

Read `gateway/gateway-execution/src/runner.rs:1099-1110`. The method has access to `self` which is `ExecutionRunner` containing:
- `self.state_service` — StateService
- `self.handles` — Arc<RwLock<HashMap<String, ExecutionHandle>>>
- All the services needed to spawn delegated agents (same as in `spawn_delegated_agent`)

- [ ] **Step 2: Implement smart resume in `runner.rs`**

Replace the `resume()` method at `gateway/gateway-execution/src/runner.rs:1099-1110` with:

```rust
/// Resume a paused or crashed execution by session ID.
///
/// For crashed sessions with a crashed subagent: re-spawns only the crashed
/// subagent using its child session's message history, avoiding root re-evaluation.
/// For paused sessions or root-only crashes: falls through to current behavior.
pub async fn resume(&self, session_id: &str) -> Result<(), String> {
    // Check for crashed subagent first
    if let Ok(Some(crashed_exec)) = self.state_service.get_last_crashed_subagent(session_id) {
        if let Some(child_session_id) = &crashed_exec.child_session_id {
            tracing::info!(
                session_id = %session_id,
                crashed_agent = %crashed_exec.agent_id,
                child_session = %child_session_id,
                "Smart resume: re-spawning crashed subagent instead of root"
            );
            return self.resume_crashed_subagent(session_id, &crashed_exec).await;
        }
    }

    // Fallback: standard resume (paused sessions or root-only crashes)
    self.state_service.resume_session(session_id)?;

    let handles = self.handles.read().await;
    for handle in handles.values() {
        handle.resume();
    }

    Ok(())
}
```

- [ ] **Step 3: Implement `resume_crashed_subagent()` method**

Add this method to `ExecutionRunner` in `runner.rs`. This method:
1. Reactivates root session and execution
2. Cancels the old crashed subagent execution
3. Ensures pending_delegations is incremented
4. Re-spawns the subagent via `spawn_delegated_agent`

```rust
/// Re-spawn a crashed subagent without re-running the root agent.
async fn resume_crashed_subagent(
    &self,
    session_id: &str,
    crashed_exec: &AgentExecution,
) -> Result<(), String> {
    let child_session_id = crashed_exec.child_session_id.as_ref()
        .ok_or("No child_session_id on crashed execution")?;

    // 1. Reactivate root session and execution
    self.state_service.reactivate_session(session_id)?;
    if let Ok(Some(root_exec)) = self.state_service.get_root_execution(session_id) {
        self.state_service.reactivate_execution(&root_exec.id)?;
    }

    // 2. Cancel the old crashed execution
    self.state_service.cancel_execution(&crashed_exec.id)?;

    // 3. Reactivate the child session
    self.state_service.reactivate_session(child_session_id)?;

    // 4. Ensure pending_delegations is at least 1
    //    (the crash may or may not have called complete_delegation)
    self.state_service.register_delegation(session_id)?;

    // 5. Request continuation so root agent processes the callback when subagent finishes
    self.state_service.request_continuation(session_id)?;

    // 6. Build a DelegationRequest from the crashed execution's data
    let parent_execution_id = crashed_exec.parent_execution_id.as_ref()
        .ok_or("No parent_execution_id on crashed execution")?;

    let task = crashed_exec.task.as_ref()
        .ok_or("No task on crashed execution")?;

    // Create new child execution
    let new_exec = AgentExecution::new_delegated(
        session_id,
        &crashed_exec.agent_id,
        parent_execution_id,
        crashed_exec.delegation_type.clone(),
        task,
    );
    self.state_service.create_execution(&new_exec)?;
    self.state_service.set_child_session_id(&new_exec.id, child_session_id)?;

    let request = DelegationRequest {
        parent_agent_id: parent_execution_id.clone(), // Will be resolved from root exec
        session_id: session_id.to_string(),
        parent_execution_id: parent_execution_id.clone(),
        child_agent_id: crashed_exec.agent_id.clone(),
        child_execution_id: new_exec.id.clone(),
        task: task.clone(),
        context: None,
        max_iterations: None,
        output_schema: None,
        skills: vec![],
        complexity: None,
    };

    // 7. Re-spawn the subagent
    spawn_delegated_agent(
        &request,
        self.event_bus.clone(),
        self.agent_service.clone(),
        self.provider_service.clone(),
        self.mcp_service.clone(),
        self.skill_service.clone(),
        self.paths.clone(),
        self.conversation_repo.clone(),
        self.handles.clone(),
        self.delegation_registry.clone(),
        self.delegation_tx.clone(),
        self.log_service.clone(),
        self.state_service.clone(),
        self.workspace_cache.clone(),
        None, // No delegation permit needed for resume
        self.memory_repo.clone(),
        self.embedding_client.clone(),
        self.memory_recall.clone(),
        self.rate_limiters.clone(),
    ).await?;

    Ok(())
}
```

**Important:** The `spawn_delegated_agent` function will load the child session's existing messages as part of its normal flow. The child session already has the task message and any partial progress from the first attempt. The new execution will continue from where the subagent left off.

- [ ] **Step 4: Verify field access — check what fields ExecutionRunner has**

Before compiling, verify that `ExecutionRunner` has access to all the services passed to `spawn_delegated_agent`. Read the `ExecutionRunner` struct definition and its `new()` constructor to confirm. The fields needed are:
- `event_bus`, `agent_service`, `provider_service`, `mcp_service`, `skill_service`
- `paths`, `conversation_repo`, `handles`, `delegation_registry`, `delegation_tx`
- `log_service`, `state_service`, `workspace_cache`
- `memory_repo`, `embedding_client`, `memory_recall`, `rate_limiters`

If any are missing from the struct, they need to be added. Check `runner.rs` for the struct definition (search for `pub struct ExecutionRunner`).

- [ ] **Step 5: Fix `parent_agent_id` in DelegationRequest**

The `parent_agent_id` in the DelegationRequest should be the root agent's ID, not the parent execution ID. Fix by reading the root execution:

```rust
let root_agent_id = self.state_service.get_root_execution(session_id)?
    .map(|e| e.agent_id)
    .unwrap_or_else(|| "root".to_string());

// Then in DelegationRequest:
parent_agent_id: root_agent_id,
```

- [ ] **Step 6: Add `cancel_execution()` if it doesn't exist**

Check if `StateService` has a `cancel_execution()` method. If not, add to repository and service:

```rust
// repository.rs
pub fn cancel_execution(&self, execution_id: &str) -> Result<(), String> {
    self.db.with_connection(|conn| {
        conn.execute(
            "UPDATE agent_executions SET status = 'cancelled', completed_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().to_rfc3339(), execution_id],
        )?;
        Ok(())
    })
}

// service.rs
pub fn cancel_execution(&self, execution_id: &str) -> Result<(), String> {
    self.repo.cancel_execution(execution_id)
}
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: compiles. Fix any missing imports or field access issues.

- [ ] **Step 8: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs services/execution-state/src/repository.rs services/execution-state/src/service.rs
git commit -m "feat: smart resume detects crashed subagent and re-spawns only it"
```

---

### Task 5: UI — Show Resume Button for Crashed Sessions

**Files:**
- Modify: `apps/ui/src/features/ops/WebOpsDashboard.tsx:167-169`

- [ ] **Step 1: Update `canResume` condition**

In `apps/ui/src/features/ops/WebOpsDashboard.tsx`, find line ~167-169:

```tsx
const canPause = session.status === "running";
const canResume = session.status === "paused";
const canCancel = session.status === "running" || session.status === "paused";
```

Change `canResume` to:

```tsx
const canResume = session.status === "paused" || session.status === "crashed";
```

- [ ] **Step 2: Verify the resume handler already works for this case**

The `onResume` handler calls `transport.resumeSession(sessionId)` which sends `{ type: "resume", session_id }`. This is the same message format the backend already handles. No transport changes needed.

- [ ] **Step 3: Build the UI**

Run: `cd apps/ui && npm run build`
Expected: builds successfully

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/ops/WebOpsDashboard.tsx
git commit -m "feat: show Resume button for crashed sessions"
```

---

### Task 6: Integration Test — End-to-End Smart Resume Verification

**Files:**
- Test: `services/execution-state/src/service.rs` (add integration-style test)

- [ ] **Step 1: Write test for the full resume flow state transitions**

In `services/execution-state/src/service.rs` test module, add a test that simulates the full lifecycle:

```rust
#[test]
fn test_smart_resume_state_transitions() {
    let service = setup_service();

    // 1. Create root session (running)
    let session = mock_running_session();
    service.create_session(&session).unwrap();

    // 2. Create root execution
    let root_exec = AgentExecution::new_root(&session.id, "root");
    service.create_execution(&root_exec).unwrap();
    service.start_execution(&root_exec.id).unwrap();

    // 3. Register a delegation and create subagent
    service.register_delegation(&session.id).unwrap();
    let child_session = Session::new_child("researcher", &session.id);
    service.create_session_from(&child_session).unwrap();

    let sub_exec = AgentExecution::new_delegated(
        &session.id, "researcher", &root_exec.id,
        DelegationType::Sequential, "Research task",
    );
    service.create_execution(&sub_exec).unwrap();
    service.set_child_session_id(&sub_exec.id, &child_session.id).unwrap();
    service.start_execution(&sub_exec.id).unwrap();

    // 4. Complete root execution (it has pending delegations)
    service.complete_execution(&root_exec.id).unwrap();
    service.request_continuation(&session.id).unwrap();

    // 5. Crash the subagent
    service.crash_execution(&sub_exec.id, "LLM 500 error").unwrap();
    service.crash_session(&session.id).unwrap();

    // Verify crashed state
    let s = service.get_session(&session.id).unwrap().unwrap();
    assert_eq!(s.status, SessionStatus::Crashed);

    // 6. Find crashed subagent
    let crashed = service.get_last_crashed_subagent(&session.id).unwrap().unwrap();
    assert_eq!(crashed.agent_id, "researcher");
    assert_eq!(crashed.child_session_id.as_ref().unwrap(), &child_session.id);

    // 7. Simulate what resume_crashed_subagent does:
    service.reactivate_session(&session.id).unwrap();
    service.reactivate_execution(&root_exec.id).unwrap();
    service.cancel_execution(&sub_exec.id).unwrap();
    service.reactivate_session(&child_session.id).unwrap();
    service.register_delegation(&session.id).unwrap();
    service.request_continuation(&session.id).unwrap();

    // Verify post-resume state
    let s = service.get_session(&session.id).unwrap().unwrap();
    assert_eq!(s.status, SessionStatus::Running);
    assert!(s.pending_delegations >= 1);
    assert!(s.continuation_needed);

    let old_exec = service.get_execution(&sub_exec.id).unwrap().unwrap();
    assert_eq!(old_exec.status, ExecutionStatus::Cancelled);
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p execution-state test_smart_resume_state_transitions -- --nocapture`
Expected: PASS

- [ ] **Step 3: Verify full workspace compiles**

Run: `cargo check --workspace`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add services/execution-state/src/service.rs
git commit -m "test: add integration test for smart resume state transitions"
```

---

### Task 7: Final Verification

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test --workspace`
Expected: all tests pass

- [ ] **Step 2: Run UI build**

Run: `cd apps/ui && npm run build`
Expected: builds successfully

- [ ] **Step 3: Verify the decision log**

The spec notes: "If multiple subagents crash (parallel delegations), only the most recently started one is retried." This is implemented by the `ORDER BY started_at DESC LIMIT 1` in `get_last_crashed_subagent()`. No additional work needed.

- [ ] **Step 4: Final commit (if any fixups needed)**

```bash
git add -A
git commit -m "fix: address any remaining compilation or test issues"
```
