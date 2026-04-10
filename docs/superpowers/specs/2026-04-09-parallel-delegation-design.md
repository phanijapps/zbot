# Parallel Delegation — Design Spec

## Problem

All subagent delegations within a session run sequentially — one at a time — even when tasks are independent. The per-session delegation handler queues each request and waits for the previous to complete before spawning the next. This means 3 independent 30-second tasks take 90 seconds instead of 30.

The `DelegationType::Parallel` enum variant exists in the data model but the handler ignores it, treating all delegations as sequential.

## Solution

Honor the `Parallel` delegation type. When the root agent sets `parallel: true` on the `delegate_to_agent` tool call, the delegation handler skips the per-session queue and sends the task directly to the global semaphore. The root agent decides when tasks are safe to parallelize.

## Design

### Tool Change: `delegate_to_agent`

Add `parallel: bool` parameter (default `false`) to the delegate tool schema:

```json
{
  "name": "delegate_to_agent",
  "parameters": {
    "agent_id": "code-agent",
    "task": "Implement auth middleware",
    "parallel": true
  }
}
```

When `parallel: true`:
- `DelegationType::Parallel` is set on the execution (instead of `Sequential`)
- The delegation handler spawns immediately without per-session queuing

When `parallel: false` (default):
- Current behavior preserved — per-session sequential queue

### Delegation Handler Change

In `runner.rs` delegation handler loop (lines 369-437), change the queuing decision:

**Current:**
```rust
if active_sessions.contains(&session_id) {
    queued.entry(session_id).or_default().push_back(request);
} else {
    active_sessions.insert(session_id.clone());
    spawn_with_notification(request, ...);
}
```

**New:**
```rust
if request.parallel {
    // Parallel: skip per-session queue, go straight to global semaphore
    spawn_with_notification(request, ...);
} else if active_sessions.contains(&session_id) {
    // Sequential: queue behind active delegation
    queued.entry(session_id).or_default().push_back(request);
} else {
    // Sequential: no active delegation, spawn immediately
    active_sessions.insert(session_id.clone());
    spawn_with_notification(request, ...);
}
```

Parallel delegations don't register in `active_sessions` and don't block sequential delegations from the same session. They only compete for the global semaphore (`max_parallel_agents`).

### DelegationRequest Change

Add `parallel: bool` field to `DelegationRequest` struct:

```rust
pub struct DelegationRequest {
    // ... existing fields ...
    pub parallel: bool,
}
```

### stream.rs Change

In `handle_delegation()`, pass the parallel flag from the tool call arguments to the `DelegationRequest` and set `DelegationType::Parallel` or `Sequential` accordingly.

### Root Agent System Prompt Guidance

Add to orchestrator instructions:

```
When delegating to agents, set parallel: true when:
- Tasks are independent (no shared files or state)
- Tasks don't need results from each other
- Tasks work in different wards or on different files

Keep parallel: false (default) when:
- Tasks must run in order (plan before code)
- Tasks share the same ward or files
- A later task depends on an earlier task's output
```

### Callback Handling (Already Works)

The existing continuation mechanism handles parallel delegations correctly:
- `pending_delegations` counter is incremented per delegation (in `handle_delegation`)
- Each completion decrements the counter via `complete_delegation()`
- When counter reaches 0 and `continuation_needed` is set, `SessionContinuationReady` fires
- Root gets a continuation turn to process all results

No changes needed to the continuation flow.

### Global Semaphore Still Enforced

Even with `parallel: true`, the global `delegation_semaphore` (capacity = `max_parallel_agents`) controls total concurrency. If `max_parallel_agents = 2` and 4 parallel delegations are requested, 2 run immediately and 2 block on the semaphore — but they block on the GLOBAL semaphore, not the per-session queue.

### Rate Limiter Interaction

If parallel subagents share a provider with `concurrent_requests: 1`, one will block on the rate limiter. This is correct behavior — the rate limiter protects the provider API. Parallel delegation reduces wall clock time for compute-bound tasks (different providers) and I/O-bound tasks (file operations), even if LLM calls are serialized by the rate limiter.

## Status Visibility

Parallel delegations that are waiting on the global semaphore show as `queued` in the database (same as today). The Dashboard already renders these with a Clock icon.

Potential enhancement (not required): Emit a `DelegationQueued` event when a delegation is created but waiting on the semaphore, so the UI can show it in real-time without polling.

## Scope

### In Scope
- Add `parallel` parameter to `delegate_to_agent` tool
- Add `parallel` field to `DelegationRequest`
- Modify delegation handler to skip per-session queue for parallel delegations
- Set `DelegationType::Parallel` on parallel executions
- Add orchestrator prompt guidance for when to use parallel

### Out of Scope
- Auto-detection of parallelizable tasks (intent analysis deciding)
- `DelegationQueued` WebSocket event (future enhancement)
- Rate limiter awareness of parallel agents
- Ward-level file locking for conflict prevention
- UI changes (queued status already visible)

## Files to Modify

| File | Change |
|------|--------|
| `runtime/agent-runtime/src/tools/delegate.rs` | Add `parallel` parameter to tool schema and action |
| `gateway/gateway-execution/src/invoke/stream.rs` | Pass parallel flag to DelegationRequest, set DelegationType |
| `gateway/gateway-execution/src/delegation/mod.rs` | Add `parallel: bool` to DelegationRequest |
| `gateway/gateway-execution/src/runner.rs:369-437` | Skip per-session queue when parallel=true |
| `gateway/gateway-templates/src/` or config shards | Add parallel delegation guidance to orchestrator prompt |

## Risks

- **File conflicts:** Two parallel code-agents editing the same file = corruption. Mitigated by prompt guidance ("only parallelize independent tasks"). Future: ward-level locking.
- **Rate limiter bottleneck:** Parallel agents sharing a provider with low concurrent_requests get serialized at the rate limiter. This is by design — the rate limiter protects the API.
- **Token waste:** If parallel tasks turn out to be dependent and one fails, the other may do wasted work. Sequential avoids this by letting root react to each result.
