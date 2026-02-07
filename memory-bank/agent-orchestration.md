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
| `gateway/gateway-execution/src/runner.rs` | `spawn_continuation_handler()`, `invoke_continuation()` |
| `gateway/gateway-execution/src/lifecycle.rs` | `request_continuation()` on root completion |
| `gateway/gateway-execution/src/delegation/spawn.rs` | `complete_delegation()` triggers event |

### Session State
| File | Purpose |
|------|---------|
| `services/execution-state/src/service.rs` | StateService with delegation tracking |
| `services/execution-state/src/repository.rs` | DB operations for pending_delegations, ward_id |
| `gateway/gateway-database/src/schema.rs` | Schema with continuation + ward_id columns |

### Message Loading
| File | Purpose |
|------|---------|
| `gateway/gateway-database/src/repository.rs` | `get_session_root_messages()` |

### Events
| File | Purpose |
|------|---------|
| `gateway/gateway-events/src/lib.rs` | `SessionContinuationReady`, `WardChanged` events |
| `gateway/gateway-events/src/broadcast.rs` | Session-scoped subscriptions |
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
    continuation_needed INTEGER DEFAULT 0,  -- Flag: continue after all complete
    ward_id TEXT                            -- Active code ward name
);
```

## Remaining Issues

### 1. Session Lifecycle (RESOLVED)
- `/end` command now properly marks sessions as completed
- `/new` command ends current session AND starts fresh
- Frontend sends `end_session` WebSocket message to backend
- Backend calls `StateService::complete_session()` to mark as completed

**Implementation:**
- WebSocket: `ClientMessage::EndSession`, `ServerMessage::SessionEnded`
- Handler: `gateway/src/websocket/handler.rs` (EndSession handler)
- Runtime: `RuntimeService::end_session()` → `ExecutionRunner::end_session()`
- Frontend: `handleEndSession()` in `WebChatPanel.tsx`

### 2. UI Event Visibility (RESOLVED)
- Delegation events now update UI in real-time
- No need to slide panel to see updates

**Root Cause**: Stale closure - `handleStreamEvent` was captured once when subscription
was set up, causing delegation events to use old state references.

**Fix**: Use `useCallback` with empty deps:
- `handleStreamEvent` defined with `useCallback(fn, [])` - stable reference
- State setters (setMessages, setIsProcessing, etc.) are inherently stable
- Subscription effect depends on `[conversationId, handleStreamEvent]`
- No ref pattern needed - simpler and avoids double-event issues

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
