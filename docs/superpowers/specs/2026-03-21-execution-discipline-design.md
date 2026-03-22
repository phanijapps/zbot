# Execution Discipline Design

## Problem

Root agent fires 6 delegations simultaneously, polls with execution_graph(status) wasting iterations, re-delegates entire plans when steps fail, and crashes with 0/6 steps done. Orphaned subagents keep running after root crashes, burning API credits. Subagents create 15-step plans for simple tasks.

Evidence from sess-ce764111: 16 delegations for a 6-step task. Root crashed at 29 iterations with plan: 0/6 done. 3 subagents still running at crash. 5 subagents crashed with "stuck in loop."

## Design

### 1. Sequential Delegation Enforcement

**Current:** Root fires 6 `delegate_to_agent` calls in one turn. The delegation handler spawns them concurrently. Continuation only fires when ALL `pending_delegations` reach 0, so root can't make mid-flow decisions.

**Fix (two layers):**

**Layer A — Tool-level warning:** The `delegate_to_agent` tool checks if there are already active delegations for this session. If yes, it still processes the request (to avoid breaking existing flows) but returns a warning in the response:
```
"Warning: You already have N active delegation(s). Delegate one step at a time and wait for the result. The system will resume you automatically when each delegation completes."
```

**Layer B — Handler-level sequential queue:** The delegation handler maintains a per-session queue (`HashMap<String, VecDeque<DelegationRequest>>`). When a request arrives:
1. If no active delegation for this session: spawn immediately
2. If active delegation exists: queue the request
3. When a delegation completes (success or failure): pop next from queue, spawn it
4. Each queued request completion decrements `pending_delegations`

This means if root fires 6, they run one at a time. Each completion triggers the next, and `pending_delegations` decrements correctly. Continuation fires after the last one completes.

**Note:** This does NOT enable mid-flow decisions (root doesn't resume between queued delegations). But it prevents resource exhaustion and API rate limits. Mid-flow decisions come from the prompt fix (root should delegate one at a time).

**Global semaphore:** `Semaphore::new(3)` → `Semaphore::new(2)`.

### 2. Kill Orphans on Root Completion/Crash

**Current:** When root crashes with pending_delegations: 3, subagents keep running indefinitely.

**Fix:** When root execution completes or crashes, call `cancel_session_delegations(session_id)`:

1. Add `get_by_session_id()` method to `DelegationRegistry` — iterates entries, filters by `ctx.session_id`, returns list of `(execution_id, DelegationContext)` pairs
2. For each active delegation: find the corresponding `ExecutionHandle` in the handles map (keyed by conversation_id — use `ctx.parent_conversation_id` to reconstruct the child conversation_id pattern, OR store `child_conversation_id` in the context)
3. Call `handle.stop()` (NOT `request_stop()` — the actual method is `stop()`) on each child handle
4. Mark each child execution as `ExecutionStatus::Cancelled` (already exists in the enum — no schema migration needed)
5. Drain the per-session delegation queue — for each discarded request:
   - Mark `request.child_execution_id` as `Cancelled` in the DB
   - Call `state_service.complete_delegation(session_id)` to decrement `pending_delegations`
6. Call `state_service.complete_session(session_id)` to finalize

**Key-mismatch note:** The `DelegationRegistry` is keyed by `execution_id`, the handles map by `conversation_id`. To bridge: store `child_conversation_id` in the `DelegationContext` (it's already constructed in spawn.rs — just needs to be saved to the context).

### 3. Graceful Failure Acceptance

**Current:** Root re-creates entire plan when delegations fail. Enters re-plan/re-delegate loop.

**Fix (prompt):** Replace "Self-Healing on Failure" section in `planning_autonomy.md`:
```
When a delegation fails:
1. Read the structured crash report. Note what was accomplished.
2. Retry the FAILED STEP once with a simpler task description.
3. If the retry also fails, mark the step "failed" and move to the next step.
4. NEVER re-create the plan. Update step statuses on the existing plan.
5. If more than half your steps have failed, call respond() with partial results.
6. Include in your response: what succeeded, what failed, and why.

After delegating a step, the system will automatically resume you when it completes.
Do NOT poll with execution_graph(status). Do NOT use Start-Sleep. Just wait.
```

**Fix (code) — Plan replacement warning:** In the `update_plan` tool:
- If a plan already exists with completed/failed steps AND the new plan has all steps "pending" (full reset), return warning:
  `"Warning: You are replacing an existing plan. Update step statuses instead."`
- Non-blocking — just a warning in the response.

**Fix (code) — Add "failed" status:** The `update_plan` schema currently allows `["pending", "in_progress", "completed"]`. Add `"failed"` to the enum so the LLM can mark steps as failed per the prompt guidance.

### 4. Subagent Plan Cap

**Current:** Subagents create 9-15 step plans for simple tasks.

**Fix:** The `update_plan` tool checks if the executor is a delegated subagent. If yes, cap plans at 5 steps. If more submitted, truncate and warn:
`"Plan truncated to 5 steps. You are a specialist — keep tasks focused."`

**Detection:** Add `is_delegated: bool` to `ExecutorConfig` (in `runtime/agent-runtime/src/executor.rs`). Set to `true` in `spawn.rs` when building the delegated executor config — inject as initial_state `"app:is_delegated"`. The `update_plan` tool reads `ctx.get_state("app:is_delegated")`.

### 5. No Polling (prompt only)

**Current:** Root calls `execution_graph(status)` and `Start-Sleep` repeatedly, wasting iterations.

**Fix:** Prompt change only (Section 3 above covers this). The continuation mechanism already handles automatic resumption. No code change needed.

## File Changes

| File | Change |
|---|---|
| `gateway/gateway-execution/src/runner.rs` | Per-session delegation queue, semaphore 3→2, cancel_session_delegations on root finish, store child_conversation_id in context |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Pass child_conversation_id to DelegationContext |
| `gateway/gateway-execution/src/delegation/context.rs` | Add child_conversation_id to DelegationContext, queue drain with execution cancellation |
| `gateway/gateway-execution/src/delegation/registry.rs` | Add get_by_session_id() method |
| `runtime/agent-tools/src/tools/execution/update_plan.rs` | Plan replacement warning, "failed" status, subagent plan cap |
| `runtime/agent-runtime/src/executor.rs` | Add is_delegated to ExecutorConfig |
| `gateway/gateway-execution/src/invoke/executor.rs` | Set is_delegated in initial_state for delegated executors |
| `gateway/templates/shards/planning_autonomy.md` | Graceful failure protocol, no polling, one-at-a-time delegation |
| `runtime/agent-tools/src/tools/agent.rs` | Warning when pending_delegations > 0 |

## Testing

| Test | What it verifies |
|---|---|
| Sequential queue | Second delegation for same session queues until first completes |
| Cross-session parallelism | Different sessions can delegate concurrently (up to semaphore 2) |
| Orphan cancellation | Root crash → all child handles get stop() signal |
| Queue drain on cancel | Queued delegations marked cancelled, pending_delegations decremented |
| Plan replacement warning | Replacing completed plan with all-pending triggers warning |
| Plan update normal | Updating step statuses on existing plan — no warning |
| Failed status | update_plan with step status "failed" accepted |
| Subagent plan cap | Delegated executor with 10-step plan → truncated to 5 |
| Root plan not capped | Root executor with 10-step plan → no truncation |
| Delegate warning | delegate_to_agent with active delegation returns warning |
