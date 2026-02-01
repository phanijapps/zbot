# Agent Orchestration Design

## Vision

Root agent acts as an **orchestrator** with full control over subagents, tools, MCPs, and context. The root remains "alive" until it explicitly decides to complete, not when the executor runs out of tool calls.

## Current Problem

```
Root delegates → Executor has no tool calls → Root marked complete → Subagent callback → Nobody listening
```

Root completes prematurely because:
1. Delegation is fire-and-forget (async)
2. Executor auto-completes when no pending tool calls
3. No mechanism to "wait" for delegation results

## Target Architecture

```
                    ┌─────────────────┐
                    │   ROOT AGENT    │
                    │  (Orchestrator) │
                    └────────┬────────┘
                             │
           ┌─────────────────┼─────────────────┐
           │                 │                 │
           ▼                 ▼                 ▼
    ┌──────────┐      ┌──────────┐      ┌──────────┐
    │ Subagent │      │ Subagent │      │ Subagent │
    │    A     │      │    B     │      │    C     │
    └────┬─────┘      └────┬─────┘      └────┬─────┘
         │                 │                 │
         ▼                 ▼                 ▼
      Callback          Callback          Callback
         │                 │                 │
         └─────────────────┼─────────────────┘
                           │
                           ▼
                    ┌─────────────────┐
                    │  ROOT EVALUATES │
                    │  - Got A & B    │
                    │  - Good enough  │
                    │  - Kill C       │
                    │  - Respond      │
                    └─────────────────┘
```

## Root Agent Capabilities

### Delegation Control
- Spawn N parallel subagents
- Query delegation status (who's running, who's done, results)
- Cancel specific subagent or all pending
- Wait for: all, any, specific, or N completions

### Decision Making
- Evaluate after each callback (streaming results)
- Decide: wait more, kill remaining, delegate more, respond
- Access full context: tools, MCPs, subagent outputs

### Completion
- Agent-driven, not executor-driven
- Root explicitly signals "done" after satisfaction
- Premature completion prevented by design

## Required Components

### 1. Delegation Registry (per execution)
```rust
struct ExecutionDelegations {
    execution_id: String,
    pending: HashMap<String, DelegationInfo>,
    completed: HashMap<String, DelegationResult>,
}

struct DelegationInfo {
    subagent_id: String,
    task: String,
    started_at: DateTime,
    execution_id: String,  // subagent's execution ID
}

struct DelegationResult {
    subagent_id: String,
    result: Option<String>,
    error: Option<String>,
    completed_at: DateTime,
}
```

### 2. Callback Injection
- Subagent completion injects message into root's conversation
- Root executor receives message and continues processing
- Message format includes subagent ID, task, result

### 3. Executor Pause/Resume
- Executor can enter "waiting for delegations" state
- Not completed, not running - paused
- Resumes when callback arrives or timeout

### 4. Control Tools
```
delegate           - Spawn subagent (existing, enhanced)
delegation_status  - Query pending/completed delegations
cancel_delegation  - Cancel specific or all subagents
wait_delegations   - Block until condition met (all/any/N)
```

## Execution Flow

### Current (Broken)
1. Root receives user message
2. Root calls delegate tool → returns immediately
3. Executor sees no tool calls → completes
4. Root execution = COMPLETED
5. Subagent runs, completes, sends callback
6. Callback has nowhere to go

### Target (Orchestrator)
1. Root receives user message
2. Root calls delegate tool → spawns subagent, returns delegation ID
3. Root calls wait_delegations or continues working
4. Executor pauses if waiting for delegations
5. Subagent completes → callback injected as message
6. Executor resumes → root evaluates callback
7. Root decides: more work, more delegations, or respond
8. Root calls respond → execution completes

## Implementation Phases

### Phase 1: Modularize runner.rs
- Split 1800+ lines into focused modules
- Clear separation of concerns
- Prepare for orchestration changes

### Phase 2: Delegation Infrastructure
- Per-execution delegation tracking
- Callback injection mechanism
- Executor pause/resume state

### Phase 3: Control Tools
- delegation_status tool
- cancel_delegation tool
- wait_delegations tool
- Enhanced delegate tool

### Phase 4: Agent Instructions
- Update agent prompts for orchestration
- Examples of parallel delegation patterns
- Best practices for subagent management

## Open Questions

1. **Timeout handling**: What if subagent hangs? Default timeout? Agent-specified?
2. **Nested orchestration**: Can subagent also be an orchestrator?
3. **Resource limits**: Max parallel subagents? Token budget distribution?
4. **Error propagation**: How does subagent failure affect root?
5. **State persistence**: Checkpoint/resume for long orchestrations?

## Related Files

- `gateway/src/execution/runner.rs` - Main execution logic (needs modularization)
- `gateway/src/execution/delegation.rs` - Delegation context/registry
- `runtime/agent-runtime/src/executor.rs` - Agent executor loop
- `runtime/agent-runtime/src/tools/delegate.rs` - Delegate tool
