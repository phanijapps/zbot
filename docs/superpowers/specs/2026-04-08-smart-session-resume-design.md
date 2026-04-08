# Smart Session Resume — Design Spec

## Problem

When a subagent crashes (LLM 500/429 errors), hitting "Resume" restarts from the root agent. The root re-evaluates intent, re-plans, and re-delegates all subagents — wasting tokens and duplicating completed work.

**Example from `sess-a66ea84c`:**
- planner-agent: completed (26 messages)
- code-agent: completed (18 messages)
- research-agent: crashed (1 message)
- On resume: root re-runs all 16 root messages, re-delegates all 3 agents

## Solution

Resume should detect where the crash happened and retry only the crashed subagent, skipping the root agent entirely.

## Design

### Schema Change

Add `child_session_id TEXT` (nullable) to `agent_executions` table.

- Set during `spawn_delegation()` when the child session is created
- Provides an unambiguous link from a delegation execution to its child session
- Root executions leave it `NULL`

### Resume Flow

When `ExecutionRunner::resume(session_id)` is called:

1. **Find the most recent crashed subagent execution:**
   ```sql
   SELECT * FROM agent_executions
   WHERE session_id = :root_session_id
     AND status = 'crashed'
     AND parent_execution_id IS NOT NULL
   ORDER BY started_at DESC
   LIMIT 1
   ```

2. **If crashed subagent found:**
   - Look up its `child_session_id` to find the child session
   - Update root session status: `crashed` → `running`
   - Update root execution status: `crashed` → `running`
   - Mark the crashed subagent execution as `cancelled`
   - Ensure `pending_delegations` ≥ 1 on the root session (if the crash already called `complete_delegation()`, re-increment; if it didn't, the count is already correct)
   - Create a new child execution for the same agent under the root session (with `child_session_id` pointing to the same child session)
   - Reactivate the child session if it was marked crashed/completed
   - Load the child session's existing message history
   - Build an executor for the agent (agent config from filesystem, same path as normal delegation)
   - Run the executor with the child session's messages
   - Existing delegation completion flow handles the rest:
     - Callback posted to parent session
     - `complete_delegation()` decrements pending count
     - `SessionContinuationReady` fires if all delegations done
     - Root agent gets a continuation turn to process results

3. **If no crashed subagent found (root-level crash):**
   - Fall through to current behavior (reactivate root execution, reload root messages)

### UI Change

Show "Resume" button when session status is `crashed` (currently only shown for `paused`). Sends the same `{ type: "resume", session_id }` WebSocket message — no protocol changes.

### Status Transitions on Subagent Resume

| Entity | Before Resume | After Resume | After Subagent Completes |
|--------|--------------|-------------|------------------------|
| Root session | `crashed` | `running` | `completed` (via existing flow) |
| Root execution | `crashed` | `running` | `completed` (via continuation) |
| Old subagent execution | `crashed` | `cancelled` | `cancelled` |
| New subagent execution | — | `running` | `completed` |
| Child session | `running`/`crashed` | `running` | `completed` |

### What Doesn't Change

- WebSocket protocol (Resume message already has session_id)
- Delegation spawn logic (reused as-is for re-spawn)
- Continuation logic (existing `complete_delegation` → `SessionContinuationReady` flow)
- Root-level crash resume (keeps current behavior)

## Decisions

- **Single subagent retry only:** If multiple subagents crash (parallel delegations), only the most recently started one is retried. Logged as future work if parallel delegation retry becomes needed.
- **Root crashes use current behavior:** No optimization for root-level crashes — root reloads its message history and re-runs.
- **Fresh execution on retry:** Crashed execution is marked `cancelled`, a new execution is created. Cleaner than reactivating a crashed execution.

## Scope

### In Scope
- `child_session_id` column on `agent_executions`
- Smart resume logic in `ExecutionRunner::resume()`
- Status transitions for root session/execution on subagent resume
- UI resume button for crashed sessions

### Out of Scope
- Retry UI showing which subagent crashed
- Parallel multi-subagent retry
- Auto-retry on transient LLM errors (429/500)
- Resume from specific subagent chosen by user

## Files to Modify

| File | Change |
|------|--------|
| `gateway/gateway-database/src/schema.rs` | Add `child_session_id` column |
| `gateway/gateway-database/migrations/` | Migration for new column |
| `execution-state/src/models.rs` | Add field to `AgentExecution` struct |
| `execution-state/src/service.rs` | Query helpers: `get_last_crashed_subagent()`, `resume_session_statuses()` |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Persist `child_session_id` when creating execution |
| `gateway/gateway-execution/src/runner.rs` | Smart resume logic in `resume()` |
| `apps/ui/` | Show Resume button for crashed sessions |
