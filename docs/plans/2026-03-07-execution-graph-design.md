# ExecutionGraphTool — Design Document

## Goal

Give the orchestrator agent the ability to dynamically build and execute workflow DAGs at runtime. This enables complex autonomous behaviors like deep research, report generation, and multi-step analysis — all without creating agents on disk.

## Architecture

The ExecutionGraphTool is a single tool with multiple actions (create, execute_next, status, add_node) that stores graph state in session state (`app:execution_graph`). It works WITH the existing delegation system — it doesn't replace `delegate_to_agent`, it orchestrates it. The agent builds a graph, then iteratively calls `execute_next` which tells it which nodes are ready. The agent then delegates those nodes using the existing `delegate_to_agent` tool.

## Key Design Decisions

1. **Session state, not persistent storage** — Graphs live in `ctx.set_state()`, same as `update_plan`. They're ephemeral per-session, which is correct for workflow orchestration.

2. **Agent-driven execution loop** — The tool doesn't auto-dispatch delegations. It tells the agent which nodes are ready, and the agent calls `delegate_to_agent` for each. This keeps the agent in control and avoids bypassing the existing delegation pipeline.

3. **Callback-driven completion** — When a delegation completes and the continuation fires, the agent calls `execute_next` again with completed node IDs. The tool evaluates conditions and returns the next wave of ready nodes.

4. **Condition evaluation** — Supports simple operators (contains, equals, gt, lt, regex) evaluated in Rust, plus `llm_eval` which returns a prompt for the agent to evaluate itself.

## Data Model

### Graph
```rust
struct ExecutionGraph {
    id: String,
    nodes: HashMap<String, GraphNode>,
    status: GraphStatus,  // pending, running, completed, failed
    created_at: String,
}
```

### Node
```rust
struct GraphNode {
    id: String,
    agent: String,           // agent-id to delegate to
    task: String,            // task description
    depends_on: Vec<String>, // upstream node IDs
    depend_mode: DependMode, // all, any_completed, any_one
    when: Option<Condition>, // conditional execution
    inputs: HashMap<String, InputRef>, // result routing
    retry: Option<RetryPolicy>,
    timeout_seconds: Option<u64>,
    on_timeout: TimeoutAction, // skip, fail
    status: NodeStatus,      // pending, ready, running, completed, skipped, failed
    result: Option<String>,  // stored when completed
    error: Option<String>,   // stored when failed
    attempts: u32,
}
```

### Condition
```rust
struct Condition {
    ref_node: String,        // upstream node to check
    field: String,           // "result" or "status"
    operator: ConditionOp,   // contains, not_contains, equals, gt, lt, regex, llm_eval
    value: String,           // comparison value
}
```

### InputRef (result routing)
```rust
struct InputRef {
    from: String,  // upstream node ID
    field: String, // "result"
}
```

## Tool Actions

### `create` — Build a new graph
**Input:** Array of node definitions
**Output:** Graph ID + initial ready nodes

### `execute_next` — Advance the graph
**Input:** Graph ID + optional list of completed node IDs with results
**Output:** List of ready nodes to dispatch (or graph completion status)

### `status` — Check graph state
**Input:** Graph ID
**Output:** All node statuses, overall progress

### `add_node` — Inject nodes mid-execution
**Input:** Graph ID + new node definition
**Output:** Updated graph state

## Execution Flow

```
Agent receives task "Deep research on X"
  → Agent calls execution_graph(action: "create", nodes: [...])
  → Tool returns: ready nodes [A, B]
  → Agent calls delegate_to_agent for A and B
  → Delegations complete, continuation fires
  → Agent calls execution_graph(action: "execute_next", completed: [{id: "A", result: "..."}, {id: "B", result: "..."}])
  → Tool evaluates conditions, returns ready nodes [C] (D skipped due to condition)
  → Agent delegates C
  → Continuation fires
  → Agent calls execute_next with C's result
  → Tool returns: graph completed, final results
  → Agent synthesizes and responds
```

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `runtime/agent-tools/src/tools/execution/graph.rs` | CREATE | ExecutionGraphTool (~500 lines) |
| `runtime/agent-tools/src/tools/execution/mod.rs` | MODIFY | Add `pub mod graph;` and re-export |
| `runtime/agent-tools/src/tools/mod.rs` | MODIFY | Add to core_tools, re-export |
| `gateway/templates/shards/tooling_skills.md` | MODIFY | Document the new tool for agents |

## What NOT to Change

- Delegation system (delegate_to_agent, DelegationRegistry, callbacks)
- Continuation mechanism (SessionContinuationReady)
- Session state infrastructure
- Any existing tool behavior
