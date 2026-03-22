# Execution Discipline Design

## Problem

Root agent fires 6 delegations simultaneously, polls with execution_graph(status) wasting iterations, re-delegates entire plans when steps fail, and crashes with 0/6 steps done. Orphaned subagents keep running after root crashes, burning API credits. Subagents create 15-step plans for simple tasks.

Evidence from sess-ce764111: 16 delegations for a 6-step task. Root crashed at 29 iterations with plan: 0/6 done. 3 subagents still running at crash. 5 subagents crashed with "stuck in loop."

## Design

### 1. Sequential Delegation Per Session

**Current:** Root fires 6 `delegate_to_agent` calls. The delegation handler spawns them concurrently via semaphore (max 3). Root then polls `execution_graph(status)` burning iterations.

**Fix:** The delegation handler processes requests **one at a time per session**. When a delegation request arrives:
1. Check if there's an active (running) delegation for this session
2. If yes: queue the request, log it, process when the current one completes
3. If no: spawn immediately

The global semaphore drops from 3 to **2** (limits total concurrent delegations across all sessions).

**Implementation:**
- `runner.rs` `spawn_delegation_handler`: maintain a `HashMap<String, VecDeque<DelegationRequest>>` keyed by session_id
- When a delegation completes (success or failure), check the queue for that session and spawn the next one
- The existing `Semaphore::new(3)` changes to `Semaphore::new(2)`

**Effect:** Root fires 6 delegate calls → first one spawns immediately, other 5 queue → first completes → continuation fires → root sees result → decides whether to continue → next delegation spawns. Sequential by default. No polling needed.

### 2. Kill Orphans on Root Completion/Crash

**Current:** When root crashes with pending_delegations: 3, those subagents keep running indefinitely.

**Fix:** When a root execution completes or crashes, cancel all in-flight delegations for that session.

**Implementation:**
- `runner.rs`: after root execution completes/crashes (both Ok and Err branches in the spawn task), call a new function `cancel_session_delegations(session_id)`
- `cancel_session_delegations`: iterate `DelegationRegistry`, find all entries for this session, call `handle.request_stop()` on each
- Subagent executors already check `handle.is_stop_requested()` on each iteration — they'll stop on the next check
- Mark stopped child executions as "cancelled" (not "crashed")
- Also drain the per-session delegation queue (from change 1) — discard queued requests

**New execution status:** Add `cancelled` alongside `completed` and `crashed`. Cancelled means "parent stopped us, not an error."

### 3. Graceful Failure Acceptance

**Current:** Root re-creates entire plan when delegations fail. Never marks steps as failed. Enters re-plan/re-delegate loop until it crashes.

**Fix (prompt):** Planning autonomy shard update:
```
When a delegation fails:
1. Read the structured crash report. Note what was accomplished.
2. Retry the FAILED STEP once with a simpler task.
3. If the retry also fails, mark the step FAILED and move to the next step.
4. NEVER re-create the plan. Update step statuses on the existing plan.
5. If more than half your steps have failed, call respond() with partial results.
6. Include in your response: what succeeded, what failed, and why.
```

**Fix (code):** The `update_plan` tool detects plan replacement and warns:
- If a plan already exists AND the new plan has all steps set to "pending" (a full reset), return a warning:
  `"Warning: You are replacing an existing plan. Update step statuses instead of creating a new plan. If you need to re-plan, mark completed steps as completed in the new plan."`
- Don't block — just warn. The LLM can still override, but the warning nudges correct behavior.

**Implementation:** In `runtime/agent-tools/src/tools/execution/plan.rs` (or wherever `update_plan` is implemented), check:
```rust
if existing_plan_has_completed_steps && new_plan_all_pending {
    // Return warning alongside the update
}
```

### 4. Subagent Plan Cap

**Current:** Subagents create 9-15 step plans for simple delegated tasks.

**Fix:** The `update_plan` tool limits plan size based on context. If the executor is a subagent (has delegation context / is not root):
- Max 5 steps in a plan
- If more than 5 submitted, truncate to 5 and warn:
  `"Plan truncated to 5 steps. You are a specialist — keep tasks focused."`

**Implementation:** The executor context has a flag or state that indicates whether it's a root or delegated execution. The `update_plan` tool reads this flag. If delegated AND steps > 5, truncate.

**How to detect subagent:** The executor config already has `initial_state` which includes `hook_context` with delegation info. Or simpler: add a `is_delegated: bool` field to `ExecutorConfig`, set to `true` in `spawn.rs` when building the delegated executor.

### 5. No Polling

**Current:** Root calls `execution_graph(status)` repeatedly to check delegation progress, wasting iterations.

**Fix (prompt):** Remove any guidance about checking execution graph status. The planning shard should say:
```
After delegating a step, the system will automatically resume you when the delegation completes.
Do NOT poll or check status. Your next turn will include the delegation result.
```

**Fix (code):** Not needed — the continuation mechanism already handles this. The polling is LLM behavior triggered by prompt patterns. Removing the prompt guidance and adding explicit "do not poll" should stop it.

## File Changes

| File | Change |
|---|---|
| `gateway/gateway-execution/src/runner.rs` | Sequential delegation queue per session, semaphore 3→2, cancel_session_delegations on root finish |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Support cancellation status |
| `gateway/gateway-execution/src/delegation/context.rs` | Queue data structure |
| `runtime/agent-tools/src/tools/execution/plan.rs` | Plan replacement warning, subagent plan cap |
| `runtime/agent-runtime/src/executor.rs` | `is_delegated` flag in config |
| `gateway/templates/shards/planning_autonomy.md` | Graceful failure protocol, no polling guidance |
| `services/execution-state/src/service.rs` | Add "cancelled" status if not exists |

## Testing

| Test | What it verifies |
|---|---|
| Sequential delegation | Second delegation for same session queues until first completes |
| Cross-session parallelism | Delegations from different sessions can run concurrently (up to semaphore limit) |
| Orphan cancellation | Root crash → all child handles get stop signal |
| Queue drain on cancel | Queued delegations discarded when root finishes |
| Plan replacement warning | Replacing plan with all-pending triggers warning |
| Plan update works normally | Updating step statuses on existing plan — no warning |
| Subagent plan cap | Delegated executor with 10-step plan → truncated to 5 with warning |
| Root plan not capped | Root executor with 10-step plan → no truncation |

## What This Does NOT Change

- Subagents can still create plans (up to 5 steps)
- Root can still call `delegate_to_agent` multiple times (they queue)
- `execution_graph` tool still works (for complex DAG workflows)
- Throttle per provider still works
- Auto-create specialist agents still works
- Structured crash reports still work
