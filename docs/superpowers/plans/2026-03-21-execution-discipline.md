# Execution Discipline Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce sequential delegation per session, kill orphaned subagents on root completion/crash, add graceful failure acceptance, cap subagent plans at 5 steps, and stop polling behavior.

**Architecture:** Per-session delegation queue in the handler, orphan cancellation via ExecutionHandle.stop(), update_plan tool enhancements (failed status, plan replacement warning, subagent cap), prompt updates for no-polling and failure acceptance.

**Tech Stack:** Rust, tokio (Semaphore, mpsc), serde_json

**Spec:** `docs/superpowers/specs/2026-03-21-execution-discipline-design.md`

---

## File Structure

| File | Change |
|------|--------|
| `gateway/gateway-execution/src/delegation/registry.rs` | Add `get_by_session_id()`, add `list_all()` |
| `gateway/gateway-execution/src/delegation/context.rs` | Add `child_conversation_id` to DelegationContext |
| `gateway/gateway-execution/src/runner.rs` | Per-session queue in handler, semaphore 3→2, cancel_session_delegations, call cancel on root finish |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Store child_conversation_id in context |
| `runtime/agent-tools/src/tools/execution/update_plan.rs` | Add "failed" status, plan replacement warning, subagent plan cap |
| `gateway/gateway-execution/src/invoke/executor.rs` | Set is_delegated in initial_state for subagents |
| `gateway/templates/shards/planning_autonomy.md` | Failure protocol, no polling, one-at-a-time delegation |

---

## Chunk 1: Delegation Queue + Orphan Cancellation

### Task 1: Add child_conversation_id to DelegationContext and registry helpers

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/context.rs`
- Modify: `gateway/gateway-execution/src/delegation/registry.rs`
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs`

- [ ] **Step 1: Add child_conversation_id to DelegationContext**

In `context.rs`, add field to `DelegationContext` struct (line 54-74):
```rust
pub child_conversation_id: Option<String>,
```

Update `DelegationContext::new()` to initialize it as `None`.

Add a builder method:
```rust
pub fn with_child_conversation_id(mut self, id: String) -> Self {
    self.child_conversation_id = Some(id);
    self
}
```

- [ ] **Step 2: Store child_conversation_id when registering delegation**

In `spawn.rs`, find where `DelegationContext` is created and registered (around line 104-112). After constructing the context, add `.with_child_conversation_id(child_conversation_id.clone())` before `delegation_registry.register()`.

- [ ] **Step 3: Add get_by_session_id to DelegationRegistry**

In `registry.rs`, add method:
```rust
/// Get all active delegations for a session.
/// Returns vec of (child_conversation_id, DelegationContext) pairs.
pub fn get_by_session_id(&self, session_id: &str) -> Vec<(String, DelegationContext)> {
    let delegations = self.delegations.read().unwrap();
    delegations
        .iter()
        .filter(|(_, ctx)| ctx.session_id == session_id)
        .map(|(conv_id, ctx)| (conv_id.clone(), ctx.clone()))
        .collect()
}
```

Note: `DelegationContext` must derive `Clone`. Check if it does — if not, add `#[derive(Clone)]`.

- [ ] **Step 4: Compile and test**

Run: `cargo check -p gateway-execution`
Run: `cargo test -p gateway-execution`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(delegation): add child_conversation_id to context, session lookup to registry"
```

---

### Task 2: Per-session sequential delegation queue

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Change semaphore from 3 to 2**

Find `Semaphore::new(3)` (line 177) and change to `Semaphore::new(2)`.

- [ ] **Step 2: Add per-session queue to delegation handler**

Find `spawn_delegation_handler` (line 191-252). This method spawns a task that loops on `rx.recv().await` and calls `spawn_delegated_agent` for each request.

Rewrite the handler body to maintain a per-session queue:

```rust
fn spawn_delegation_handler(&self, mut rx: mpsc::UnboundedReceiver<DelegationRequest>) {
    // ... clone all the service references (keep existing clones) ...
    let delegation_semaphore = self.delegation_semaphore.clone();

    tokio::spawn(async move {
        // Per-session queue: only one delegation runs at a time per session
        let mut active_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut queued: std::collections::HashMap<String, std::collections::VecDeque<DelegationRequest>> = std::collections::HashMap::new();

        // Channel for delegation completions (session_id notifications)
        let (done_tx, mut done_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        loop {
            tokio::select! {
                // New delegation request
                Some(request) = rx.recv() => {
                    let session_id = request.session_id.clone();

                    if active_sessions.contains(&session_id) {
                        // Queue it — another delegation is running for this session
                        tracing::info!(
                            session_id = %session_id,
                            agent = %request.child_agent_id,
                            "Queuing delegation (active delegation in progress)"
                        );
                        queued.entry(session_id).or_default().push_back(request);
                    } else {
                        // Spawn immediately
                        active_sessions.insert(session_id.clone());
                        spawn_delegation(
                            request,
                            /* pass all cloned services */
                            done_tx.clone(),
                        );
                    }
                }
                // Delegation completed — check queue for next
                Some(session_id) = done_rx.recv() => {
                    active_sessions.remove(&session_id);

                    // Pop next queued request for this session
                    if let Some(queue) = queued.get_mut(&session_id) {
                        if let Some(next_request) = queue.pop_front() {
                            active_sessions.insert(session_id.clone());
                            spawn_delegation(
                                next_request,
                                /* pass all cloned services */
                                done_tx.clone(),
                            );
                        }
                        if queue.is_empty() {
                            queued.remove(&session_id);
                        }
                    }
                }
                else => break,
            }
        }
    });
}
```

The `spawn_delegation` helper wraps `spawn_delegated_agent` and sends the completion notification:

```rust
// Inside the handler, define as a closure or extract to a function
// After spawn_delegated_agent completes (success or failure), send done_tx.send(session_id)
```

Read the EXISTING handler code first. It clones many services (event_bus, agent_service, etc.). Keep all those clones. The key change: wrap the spawn in a task that notifies `done_tx` when complete.

- [ ] **Step 3: Compile and test**

Run: `cargo check -p gateway-execution`
Run: `cargo test -p gateway-execution`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(delegation): sequential per-session queue, semaphore 3→2"
```

---

### Task 3: Cancel orphans on root completion/crash

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Add cancel_session_delegations function**

```rust
/// Cancel all in-flight delegations for a session.
/// Called when root execution completes or crashes.
async fn cancel_session_delegations(
    session_id: &str,
    delegation_registry: &DelegationRegistry,
    handles: &RwLock<HashMap<String, ExecutionHandle>>,
    state_service: &StateService<DatabaseManager>,
) {
    // Find all active delegations for this session
    let active = delegation_registry.get_by_session_id(session_id);

    if active.is_empty() {
        return;
    }

    tracing::info!(
        session_id = %session_id,
        count = active.len(),
        "Cancelling orphaned delegations"
    );

    for (child_conv_id, ctx) in &active {
        // Stop the execution handle
        let handles_guard = handles.read().await;
        if let Some(handle) = handles_guard.get(child_conv_id) {
            handle.stop();
        }

        // Remove from registry
        delegation_registry.remove(child_conv_id);

        // Decrement pending_delegations
        if let Err(e) = state_service.complete_delegation(session_id) {
            tracing::debug!("Failed to complete delegation tracking: {}", e);
        }
    }
}
```

- [ ] **Step 2: Call cancel on root completion/crash**

In the root execution's spawn task (inside `invoke()`), find both the Ok and Err branches after `execute_stream`. In BOTH branches, after `complete_execution` or `crash_execution`, add:

```rust
// Cancel orphaned delegations
cancel_session_delegations(
    &session_id,
    &delegation_registry_clone,  // need to clone this into the spawn block
    &handles,
    &state_service,
).await;
```

Make sure `delegation_registry` is cloned into the spawn block (it may not be currently — add it to the clones before `tokio::spawn`).

- [ ] **Step 3: Drain queued delegations on cancel**

In the delegation handler (from Task 2), when `cancel_session_delegations` removes active delegations, the `done_tx` notification will fire, and the handler will try to pop the next queued item.

To prevent this: add a "cancelled sessions" set. When a session is cancelled, add it to the set. When processing queue completions, check if the session is cancelled before popping the next item. If cancelled, drain and discard the queue, marking each discarded request's `child_execution_id` as Cancelled.

Alternatively, simpler: expose the queue through a shared reference and let `cancel_session_delegations` drain it directly. Since the handler owns the queue in its async task, this requires a shared `Arc<Mutex<HashMap<String, VecDeque<DelegationRequest>>>>`.

Choose the approach that works with the existing code structure. Read the code first.

- [ ] **Step 4: Compile and test**

Run: `cargo check -p gateway-execution`
Run: `cargo test -p gateway-execution`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(delegation): cancel orphaned delegations on root completion/crash"
```

---

## Chunk 2: Plan Tool Enhancements

### Task 4: Add "failed" status + plan replacement warning + subagent cap

**Files:**
- Modify: `runtime/agent-tools/src/tools/execution/update_plan.rs`
- Modify: `gateway/gateway-execution/src/invoke/executor.rs` (set is_delegated)
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs` (set is_delegated)

- [ ] **Step 1: Add "failed" to status enum in schema**

In `update_plan.rs`, find the status enum in `parameters_schema` (line ~66):
```rust
"enum": ["pending", "in_progress", "completed"]
```
Change to:
```rust
"enum": ["pending", "in_progress", "completed", "failed"]
```

- [ ] **Step 2: Add plan replacement warning**

In the `execute` method (line ~77-104), before storing the plan, check for replacement:

```rust
// Check for plan replacement (existing plan with completed/failed steps being reset)
if let Some(existing) = ctx.get_state("app:plan") {
    if let Some(existing_steps) = existing.get("plan").and_then(|p| p.as_array()) {
        let has_progress = existing_steps.iter().any(|s| {
            let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
            status == "completed" || status == "failed"
        });
        let new_steps = args.get("plan").and_then(|p| p.as_array());
        let all_pending = new_steps.map(|steps| {
            steps.iter().all(|s| {
                s.get("status").and_then(|v| v.as_str()) == Some("pending")
            })
        }).unwrap_or(false);

        if has_progress && all_pending {
            // Still save the plan, but include a warning
            tracing::warn!("Plan replacement detected — existing plan had progress");
            // The warning will be included in the response
        }
    }
}
```

Include the warning in the response JSON if detected.

- [ ] **Step 3: Add subagent plan cap**

After the plan replacement check, check if this is a delegated executor:

```rust
let is_delegated = ctx.get_state("app:is_delegated")
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

if is_delegated {
    if let Some(plan) = args.get_mut("plan").and_then(|p| p.as_array_mut()) {
        if plan.len() > 5 {
            tracing::info!("Subagent plan truncated from {} to 5 steps", plan.len());
            plan.truncate(5);
            // Add truncation notice to response
        }
    }
}
```

Note: `args` is passed as `&Value`. To mutate it, you may need to clone it first. Read the existing execute method to understand how args flows.

- [ ] **Step 4: Set is_delegated in delegated executors**

In `gateway/gateway-execution/src/delegation/spawn.rs`, after building the executor config (inside the `build()` call or after it), the executor's initial_state needs `app:is_delegated = true`.

The cleanest way: in `gateway/gateway-execution/src/invoke/executor.rs`, in the `build()` method, check if delegation context is present (via hook_context or a new parameter) and set `app:is_delegated`.

Alternatively: in `spawn.rs`, after `builder.build()` creates the executor, the executor config is already built. Since `ExecutorConfig.initial_state` is set inside `build()`, the simplest approach is to add the state in `build()` based on a new builder method:

```rust
// In ExecutorBuilder:
pub fn with_delegated(mut self, is_delegated: bool) -> Self {
    self.is_delegated = is_delegated;
    self
}
```

Then in `build()`, if `self.is_delegated`, add to initial_state:
```rust
executor_config = executor_config.with_initial_state("app:is_delegated", serde_json::Value::Bool(true));
```

In `spawn.rs`, call `.with_delegated(true)` on the builder.

- [ ] **Step 5: Compile and test**

Run: `cargo check --workspace`
Run: `cargo test -p agent-tools -p gateway-execution`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(plan): add failed status, replacement warning, subagent cap at 5 steps"
```

---

## Chunk 3: Prompt Updates

### Task 5: Update planning autonomy shard

**Files:**
- Modify: `gateway/templates/shards/planning_autonomy.md`

- [ ] **Step 1: Replace the Self-Healing section and add no-polling guidance**

Find the "Self-Healing on Failure" section and replace with:

```markdown
## When a Delegation Fails

1. Read the structured crash report. Note what was accomplished (completed steps, files created).
2. Retry the FAILED STEP once with a simpler, more focused task description.
3. If the retry also fails, mark the step "failed" in your plan and move to the next step.
4. NEVER re-create the plan from scratch. Update step statuses on the existing plan.
5. If more than half your steps have failed, call respond() with what you have.
6. Include in your response: what succeeded, what failed, and why.
```

Add to the "Sequential by Default" section or create a new section:

```markdown
## After Delegating

After calling delegate_to_agent, the system will AUTOMATICALLY resume you when the delegation completes.
- Do NOT call execution_graph(status) to check progress.
- Do NOT use Start-Sleep or any polling.
- Do NOT delegate multiple steps at once. Delegate ONE step, wait for the result, then delegate the next.
- Your next turn will include the delegation result or crash report.
```

- [ ] **Step 2: Delete on-disk shard so it regenerates**

```bash
rm -f "C:/Users/rampi/Documents/zbot/config/shards/planning_autonomy.md"
```

- [ ] **Step 3: Compile and test**

Run: `cargo check -p gateway-templates`
Run: `cargo test -p gateway-templates`

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(prompts): no polling, one-at-a-time delegation, graceful failure acceptance"
```

---

### Task 6: Final verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`

- [ ] **Step 2: Full test suite**

Run: `cargo test -p gateway-execution -p agent-tools -p gateway-templates -p knowledge-graph`

- [ ] **Step 3: Push**

```bash
git push origin autofill
```
