# Agent Orchestration - Implementation Status

## Vision (ACHIEVED)

Root agent acts as an **orchestrator** with full control over subagents. The root remains "alive" until it explicitly decides to complete, automatically continuing after delegations complete.

## Architecture (IMPLEMENTED)

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
                ┌─────────────────────┐
                │  SessionContinuation│
                │      Ready Event    │
                └──────────┬──────────┘
                           │
                           ▼
                    ┌─────────────────┐
                    │  ROOT CONTINUES │
                    │  - Sees results │
                    │  - Can delegate │
                    │  - Can respond  │
                    └─────────────────┘
```

## Execution Flow (CURRENT)

```
1. User sends message
2. Root agent invoked (new session OR existing session)
3. Root calls delegate tool → spawns subagent
   → register_delegation(session_id) called
4. Root execution completes
   → if pending_delegations > 0: request_continuation(session_id)
5. Subagent(s) execute in parallel
6. Each subagent completes:
   → Callback message added to parent execution
   → complete_delegation(session_id) called
   → Delegation count decremented
7. Last subagent completes AND continuation_needed:
   → SessionContinuationReady event published
8. Continuation handler receives event:
   → invoke_continuation() called
   → New execution created in session
   → Loads full session history (includes callbacks)
   → Root agent invoked
9. Root sees all callbacks, decides:
   → Respond to user (session may complete)
   → Delegate more (go to step 3)
10. Session stays RUNNING until explicit completion
```

## Key Files

### Continuation System
| File | Purpose |
|------|---------|
| `gateway/src/execution/runner.rs:176-253` | `spawn_continuation_handler()` - listens for events |
| `gateway/src/execution/runner.rs:692-831` | `invoke_continuation()` - invokes root with context |
| `gateway/src/execution/lifecycle.rs:168-186` | `request_continuation()` on root completion |
| `gateway/src/execution/delegation/spawn.rs:331-352` | `complete_delegation()` triggers event |

### Session State
| File | Purpose |
|------|---------|
| `services/execution-state/src/service.rs` | StateService with delegation tracking |
| `services/execution-state/src/repository.rs` | DB operations for pending_delegations |
| `gateway/src/database/schema.rs` | Schema with continuation columns |

### Message Loading
| File | Purpose |
|------|---------|
| `gateway/src/database/repository.rs:176-212` | `get_session_root_messages()` |

### Events
| File | Purpose |
|------|---------|
| `gateway/src/events/mod.rs` | `SessionContinuationReady` event |
| `gateway/src/events/broadcast.rs` | Session-scoped subscriptions |
| `gateway/src/websocket/handler.rs` | Forwards delegation events to UI |

## Database Schema (sessions table)

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'running',
    source TEXT NOT NULL DEFAULT 'web',
    root_agent_id TEXT NOT NULL,
    -- ... other fields ...
    pending_delegations INTEGER DEFAULT 0,  -- Count of running subagents
    continuation_needed INTEGER DEFAULT 0   -- Flag: continue after all complete
);
```

## Remaining Issues

### 1. Session Lifecycle (HIGH PRIORITY)
- `/end` and `/new` commands don't properly complete sessions
- `+new` button doesn't mark session as completed
- Sessions stay "running" indefinitely

**Root Cause**: Need to implement explicit session completion trigger from frontend.

### 2. UI Event Visibility (MEDIUM PRIORITY)
- Delegation events sent but not visible in real-time
- User has to slide chat panel in/out to see updates
- WebSocket events are delivered but UI doesn't react

**Root Cause**: React state updates may not be triggering re-renders.

### 3. Future: Control Tools
- `delegation_status` - Query pending/completed delegations
- `cancel_delegation` - Cancel specific subagent
- `wait_delegations` - Block until condition met

## Testing

All tests pass:
- **execution-state**: 82 tests (delegation tracking, continuation flags)
- **gateway lib**: 96 tests (message loading, repository)
- **gateway integration**: 34 tests (API endpoints, sessions)

Key tests:
- `test_delegation_tracking` - Register/complete delegations
- `test_continuation_trigger` - Continuation flag logic
- `test_get_session_root_messages_*` - Session-wide message loading
