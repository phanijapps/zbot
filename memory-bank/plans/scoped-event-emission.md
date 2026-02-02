# Scoped Event Emission Implementation Plan

## Status: COMPLETE ✅

## Bug Fix (2026-02-02)

**Issue**: Events not reaching UI despite backend publishing correctly.

**Root Cause**: `convert_stream_event()` in `gateway/src/execution/events.rs` was using `session_id` as `execution_id`:
```rust
// BEFORE (broken): execution_id was using session_id value
let execution_id = session_id;  // events had execution_id="sess-xxx"

// AFTER (fixed): proper execution_id parameter
pub fn convert_stream_event(
    event: StreamEvent,
    agent_id: &str,
    conversation_id: &str,
    session_id: &str,
    execution_id: &str,  // NEW parameter
) -> GatewayEvent
```

**Files Changed**:
- `gateway/src/execution/events.rs:12-18` - Added `execution_id` parameter
- `gateway/src/execution/invoke/stream.rs:316-322` - Pass `ctx.execution_id` to convert function

**Why It Failed**: Scope filter checked `event.execution_id` against cached root IDs (`exec-xxx`), but events had `execution_id="sess-xxx"` (session ID format), causing all events to be filtered out.

## Bug Fix #2: Subagent Status Not Updating (2026-02-02)

**Issue**: Subagent showed "running" even after completion.

**Root Cause**: `DelegationStarted` and `DelegationCompleted` had mismatched `child_conversation_id`:
- `DelegationStarted` via `emit_delegation_started()` had `child_conversation_id: None`
- `DelegationCompleted` had `child_conversation_id: Some(conv_id)`
- Frontend uses `childConvId = event.child_conversation_id ?? event.child_execution_id` as lookup key
- Keys didn't match, so status update failed

**Fix**: Updated `emit_delegation_started()` to accept and pass `child_conversation_id`:
```rust
// lifecycle.rs - added parameter
pub async fn emit_delegation_started(
    ...
    child_conversation_id: &str,  // NEW
    ...
) {
    event_bus.publish(GatewayEvent::DelegationStarted {
        ...
        child_conversation_id: Some(child_conversation_id.to_string()),  // Was None
    })
}

// spawn.rs - pass the conversation_id
emit_delegation_started(
    ...
    &child_conversation_id,  // NEW - same value used in DelegationCompleted
    ...
)
```

**Files Changed**:
- `gateway/src/execution/lifecycle.rs:327-350` - Added `child_conversation_id` parameter
- `gateway/src/execution/delegation/spawn.rs:104-113` - Pass `child_conversation_id` to emit function

## Bug Fix #3: Duplicate DelegationStarted Events (2026-02-02)

**Issue**: UI showed 2 subagents when only 1 was spawned.

**Root Cause**: `DelegationStarted` was emitted TWICE:
1. `gateway/src/execution/runner.rs:629` - when handling delegation (proper IDs)
2. `gateway/src/execution/events.rs:92` - when converting `StreamEvent::ActionDelegate` (fabricated IDs)

**Fix**: Changed `events.rs` to return a no-op for `ActionDelegate` since runner.rs handles it:
```rust
// ActionDelegate is handled by the runner/delegation system directly,
// which emits DelegationStarted with proper IDs. Converting here would
// cause duplicate events. Return no-op to let the stream continue.
StreamEvent::ActionDelegate { .. } => GatewayEvent::AgentStarted { ... },
```

**Also Fixed**: Frontend subscription duplication

## Bug Fix #4: Execution Scope Serialization Mismatch (2026-02-02)

**Issue**: Running subagent chat was empty in dashboard - no live events displayed.

**Root Cause**: Serialization format mismatch between frontend and backend for execution-scoped subscriptions:
- Frontend sent: `scope: "execution:exec-456"` (string with colon separator)
- Backend expected: `scope: {"execution": "exec-456"}` (JSON object per serde enum)

The backend's `SubscriptionScope::Execution(String)` variant uses serde's default enum serialization which produces `{"execution": "value"}` format, but the frontend was sending a template literal string `execution:${id}`.

**Fix**: Updated `sendSubscribe()` in frontend to convert execution scope format:
```typescript
// http.ts - convert scope format for backend compatibility
let scopePayload: string | { execution: string } = effectiveScope;
if (typeof effectiveScope === 'string' && effectiveScope.startsWith('execution:')) {
  scopePayload = { execution: effectiveScope.slice('execution:'.length) };
}

this.ws.send(JSON.stringify({
  type: "subscribe",
  conversation_id: conversationId,
  scope: scopePayload,  // Now sends {"execution": "exec-456"} for execution scopes
}));
```

**Files Changed**:
- `apps/ui/src/services/transport/http.ts:890-915` - Convert execution scope to object format

**Why It Worked For Completed Subagents**: Historical messages load via REST API, not WebSocket subscriptions. Only live streaming events were affected by the scope mismatch.

## Bug Fix #5: Scope Not Updated on Re-subscribe (2026-02-02)

**Issue**: Running subagent chat STILL empty even after Bug Fix #4.

**Root Cause**: When re-subscribing with a different scope, the backend returned early without updating the scope:
```rust
// subscribe_with_scope() in subscriptions.rs
if already_subscribed {
    // BUG: Returns early WITHOUT updating scope!
    let current_seq = *state.sequence_numbers.get(&conversation_id).unwrap_or(&0);
    return Ok(SubscribeResult::AlreadySubscribed { current_sequence: current_seq });
}
```

**Scenario**:
1. WebChatPanel subscribes to `sess-xxx` with scope `Session`
2. User opens subagent chat → SessionChatViewer tries to subscribe to same `sess-xxx` with scope `Execution("exec-yyy")`
3. Backend sees "already subscribed", returns early
4. **Old scope (`Session`) continues filtering** - subagent events are filtered out!

**Fix**: Update scope when re-subscribing with a different scope:
```rust
if already_subscribed {
    // Update scope if it changed
    let entry_key = (conversation_id.clone(), client_id.clone());
    if let Some(entry) = state.subscription_entries.get_mut(&entry_key) {
        if entry.scope != scope {
            debug!("Updating subscription scope for {} from {:?} to {:?}",
                conversation_id, entry.scope, scope);
            entry.scope = scope;
            entry.scope_state = scope_state;
        }
    }
    let current_seq = *state.sequence_numbers.get(&conversation_id).unwrap_or(&0);
    return Ok(SubscribeResult::AlreadySubscribed { current_sequence: current_seq });
}
```

**Files Changed**:
- `gateway/src/websocket/subscriptions.rs:360-374` - Update scope on re-subscribe
- Added `test_subscribe_scope_update_on_resubscribe` test

## Bug Fix #6: readOnly Prop Prevented Event Subscription (2026-02-02)

**Issue**: Subagent chat panel showed "No messages" even for completed subagents.

**Root Cause**: The `readOnly` prop in SessionChatViewer was blocking the subscription effect entirely:
```typescript
// BEFORE (broken)
useEffect(() => {
  if (!subscriptionId || readOnly) return;  // readOnly blocks subscription!
  // ... subscription setup
}, [subscriptionId, handleStreamEvent, eventScope, readOnly]);
```

**Scenario**:
1. User clicks "View subagent chat (read-only)" button
2. SessionChatViewer opens with `readOnly={true}`
3. Subscription effect sees `readOnly=true` and returns early
4. No subscription established - no live events received

**Fix**: Remove `readOnly` from subscription condition - it should only affect input UI:
```typescript
// AFTER (fixed)
useEffect(() => {
  if (!subscriptionId) return;  // readOnly removed!

  // Subscribe even in readOnly mode to receive live streaming events
  // readOnly only prevents sending messages, not receiving events
  // ... subscription setup
}, [subscriptionId, handleStreamEvent, eventScope]);  // readOnly removed from deps
```

**Files Changed**:
- `apps/ui/src/components/SessionChatViewer.tsx:207-236` - Remove readOnly from subscription effect

**Why readOnly Should Only Affect Input**:
- `readOnly` means "don't allow user to send messages"
- It should NOT mean "don't receive events"
- Users viewing a subagent chat still want to see live streaming output

---

## Progress (Updated 2026-02-02)

### Phase 1: Backend (Complete)
- ✅ #26: Added SubscriptionScope enum (All, Session, Execution) to messages.rs
- ✅ #27: Added SessionScopeState and updated SubscriptionManager with subscribe_with_scope()
- ✅ #28: Populate cache on subscribe - queries root executions from state_service
- ✅ #29: Implemented should_send_to_scope() filter and route_event_scoped() method
- ✅ #30: Update cache on new root - event router detects AgentStarted for root executions
- ✅ #31: Backend unit tests - 11 scope filtering tests, all 20 subscription tests pass

### Phase 2: Frontend (Complete)
- ✅ #32: Added SubscriptionScope type to types.ts
- ✅ #33: Updated http.ts to send scope in subscribe message
- ✅ #34: Updated WebChatPanel to use session scope
- ✅ #35: Updated SessionChatViewer with execution-scope support

### Key Implementation Details

**Backend File Changes:**
- `gateway/src/websocket/messages.rs`: Added SubscriptionScope enum, updated Subscribe/Subscribed messages
- `gateway/src/websocket/subscriptions.rs`: Added EventMetadata, SessionScopeState, should_send_to_scope(), route_event_scoped()
- `gateway/src/websocket/handler.rs`: Updated Subscribe handling and event router for scoped routing

**Frontend File Changes:**
- `apps/ui/src/services/transport/types.ts`: Added SubscriptionScope type, updated SubscriptionOptions
- `apps/ui/src/services/transport/http.ts`: Updated sendSubscribe, subscribeConversation, resubscribeAll, handleSubscriptionMessage
- `apps/ui/src/features/agent/WebChatPanel.tsx`: Uses scope: "session" for filtered root events
- `apps/ui/src/components/SessionChatViewer.tsx`: Uses scope: "execution:{id}" for detail view, "session" for root view

**How It Works:**
1. Client subscribes with `scope: "session"` (or "all" for backward compatibility)
2. Handler queries root executions via state_service.list_executions()
3. Creates SessionScopeState with cached root IDs
4. Event router uses route_event_scoped() which applies should_send_to_scope() per subscriber
5. On AgentStarted, event router checks if execution is root and updates caches
6. Frontend receives filtered events - no client-side deduplication needed

### Phase 3: Cleanup (Complete)
- ✅ #36: Simplified frontend deduplication (kept as safety net, reduced buffer size)
- ✅ #37: Documented dual-path routing as future optimization opportunity
- ✅ #38: Added E2E test for scoped subscriptions
- ✅ #39: Updated documentation

## Implementation Complete

All phases of scoped event emission are now implemented:

1. **Backend (Rust)**: Server-side scope filtering with SessionScopeState and route_event_scoped()
2. **Frontend (TypeScript)**: SubscriptionScope type and scope parameter in subscribe messages
3. **Cleanup**: Simplified deduplication, documented future optimizations, added E2E tests

---

## Overview

Implement server-side event filtering based on subscription scope to provide clean UI views:
- **Session Scope**: Root execution events + delegation lifecycle markers only
- **Execution Scope**: All events for a specific execution (debug/detail view)

## Data Model (Verified from conversations.db)

```
Session (sess-xxx)
├── Execution (exec-001, parent=NULL, delegation_type=root) ← ROOT
│   └── Messages (multi-turn within execution)
├── Execution (exec-002, parent=exec-001, delegation_type=sequential) ← SUBAGENT
│   └── Messages
└── Execution (exec-003, parent=NULL, delegation_type=root) ← CONTINUATION (new root!)
    └── Messages
```

Key facts:
- Session has MANY root executions (one per user turn + continuations)
- Root = parent_execution_id IS NULL AND delegation_type = 'root'
- Continuation after delegation creates NEW root execution
- Messages linked to execution_id (no conversation_id in schema)

## Caching Strategy

1. **On Subscribe (Session scope)**: Query all root execution IDs for session
2. **On AgentStarted**: If parent=null, add to cache (handles new turns + continuations)
3. **Filter**: Check if event.execution_id is in cached roots OR is delegation lifecycle event

## Task Breakdown

### Phase 1: Backend (Rust)

| Task | Description | File |
|------|-------------|------|
| #26 | Add SubscriptionScope enum | gateway/src/websocket/messages.rs |
| #27 | Add SessionScopeState, update SubscriptionManager | gateway/src/websocket/subscriptions.rs |
| #28 | Populate cache on subscribe | gateway/src/websocket/handler.rs |
| #29 | Implement should_send_to_scope() filter | gateway/src/websocket/handler.rs |
| #30 | Update cache on new root (AgentStarted) | gateway/src/websocket/handler.rs |
| #31 | Backend unit tests | gateway/src/websocket/subscriptions.rs |

### Phase 2: Frontend (TypeScript)

| Task | Description | File |
|------|-------------|------|
| #32 | Add SubscriptionScope type | apps/ui/src/services/transport/types.ts |
| #33 | Send scope in subscribe message | apps/ui/src/services/transport/http.ts |
| #34 | Update WebChatPanel to use session scope | apps/ui/src/features/agent/WebChatPanel.tsx |
| #35 | Add execution-scope detail view | apps/ui/src/components/SessionChatViewer.tsx |

### Phase 3: Cleanup

| Task | Description | File |
|------|-------------|------|
| #36 | Remove frontend deduplication | apps/ui/src/services/transport/http.ts |
| #37 | Remove dual-path routing | gateway/src/websocket/handler.rs |
| #38 | E2E tests | apps/ui/tests/e2e/ |
| #39 | Documentation | memory-bank/architecture.md |

## Filter Logic

```rust
fn should_send_to_scope(
    event: &GatewayEvent,
    scope: &SubscriptionScope,
    cached_roots: &HashSet<String>,
) -> bool {
    match scope {
        SubscriptionScope::All => true,
        SubscriptionScope::Session => {
            match event {
                // High-volume: filter by root cache
                Token { execution_id, .. } |
                Thinking { execution_id, .. } |
                ToolCall { execution_id, .. } |
                ToolResult { execution_id, .. } => cached_roots.contains(execution_id),

                // Delegation lifecycle: always include
                DelegationStarted { .. } |
                DelegationCompleted { .. } => true,

                // Agent lifecycle: check cache
                AgentStarted { execution_id, .. } |
                AgentCompleted { execution_id, .. } => cached_roots.contains(execution_id),

                // Session-level: always include
                _ => true,
            }
        }
        SubscriptionScope::Execution(target_id) => {
            event.execution_id() == Some(target_id)
        }
    }
}
```

## Expected Behavior

| Scenario | Session Scope Shows | Hides |
|----------|---------------------|-------|
| Root tokens/thinking | ✓ | |
| Root tool calls | ✓ | |
| DelegationStarted | ✓ | |
| Subagent tokens | | ✓ |
| Subagent tools | | ✓ |
| DelegationCompleted | ✓ | |
| Nested delegation markers | ✓ | |
| Continuation tokens | ✓ (new root added to cache) | |

## Continuation Flow

1. Root exec-001 delegates → completes
2. Subagent exec-002 works → completes
3. SessionContinuationReady fires
4. NEW root exec-003 created (parent=null)
5. AgentStarted for exec-003 → detected as root → added to cache
6. Continuation tokens shown (exec-003 in cache)

## Dependencies

```
#26 → #27 → #28 ─┬─► #30 → #31 ─────────────────┐
                 └─► #29 ─┘                      │
#26 → #32 → #33 ──────────────────► #34 ────────┼─► #36 → #37 → #38 → #39
                                    ↓           │
                                   #35 ─────────┘
```
