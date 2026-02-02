# Session-Based Event Routing

## Status: PLANNING (not implemented)

## Problem Statement

The UI doesn't receive real-time updates for delegation events (subagent completion, callbacks, continuation). User must refresh to see changes.

**Root Cause**: Events are routed by `conversation_id`, but:
- Root agent has `conversation_id: web-xxx`
- Subagents have different `conversation_id: exec-yyy`
- Delegation events use `parent_conversation_id` field
- The subscription model doesn't account for this

**Attempted fixes that failed:**
1. Check `parent_conversation_id` in addition to `conversation_id` - partial fix, still has gaps
2. Ref pattern for React hooks - caused double-character streaming bug

## The Right Model

Think of this like Gmail notifications or mobile push:
- Client subscribes to a **session** (logical unit of work)
- Server pushes **all events** for that session to the client
- Doesn't matter which agent/conversation generated the event

```
┌─────────────────────────────────────────────────────────────┐
│                      SESSION (sess-xxx)                      │
│                                                              │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│   │ Root Agent   │    │  Subagent A  │    │  Subagent B  │  │
│   │ conv: web-x  │    │ conv: exec-a │    │ conv: exec-b │  │
│   └──────────────┘    └──────────────┘    └──────────────┘  │
│                                                              │
│   ALL events from ALL agents go to session subscriber        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   WebSocket     │
                    │   (subscribed   │
                    │   to sess-xxx)  │
                    └─────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   Browser UI    │
                    │   (receives all │
                    │   session events│
                    └─────────────────┘
```

## Current Architecture

```
Frontend (WebChatPanel.tsx)
    │
    ├── Subscribes to: conversation_id (web-xxx)
    │
    └── transport.subscribe(conversationId, callback)
            │
            ▼
HttpTransport (http.ts)
    │
    ├── eventCallbacks: Map<conversationId, Set<callback>>
    │
    └── handleEvent(event):
            extracts event.conversation_id
            routes to matching callbacks
            │
            ▼
WebSocket Connection
    │
    ▼
Gateway WebSocket Handler (handler.rs)
    │
    ├── Subscribes to ALL events (event_bus.subscribe_all())
    │
    └── Forwards ALL events to connected client
            (no filtering by session/conversation)
```

**Issue**: Backend sends everything, frontend filters by conversation_id, but delegation events don't match.

## Proposed Architecture

### Option A: Session-Based Subscription (Clean but Breaking)

```
Frontend
    │
    ├── On first message: get session_id from agent_started event
    ├── Subscribe to: session_id (sess-xxx)
    │
    └── transport.subscribeSession(sessionId, callback)
            │
            ▼
HttpTransport
    │
    ├── sessionCallbacks: Map<sessionId, Set<callback>>
    │
    └── handleEvent(event):
            extracts event.session_id (all events have this)
            routes to matching callbacks
```

**Pros:**
- Clean, correct model
- All events for a session reach the UI
- No special-casing for delegation events

**Cons:**
- Session ID not known until first `agent_started` event
- Need to buffer events or handle subscription timing
- Breaking change to transport API

### Option B: Dual Subscription (Backward Compatible)

Keep conversation subscription, add session subscription:

```
Frontend
    │
    ├── Subscribe to conversation_id for streaming tokens
    ├── Subscribe to session_id for delegation/system events
    │
    └── When agent_started arrives:
            1. Extract session_id
            2. Call transport.subscribeSession(sessionId, delegationCallback)
```

**Pros:**
- Backward compatible
- Clear separation: tokens go to conversation, delegations go to session

**Cons:**
- Two subscriptions to manage
- More complex state management

### Option C: Server-Side Filtering (Minimal Frontend Change)

Have the backend track which sessions each WebSocket client cares about:

```
Backend WebSocket Handler
    │
    ├── Client sends: { type: "subscribe_session", session_id: "sess-xxx" }
    │
    ├── Handler tracks: client -> Set<session_id>
    │
    └── On event: only forward if event.session_id in client's subscriptions
```

**Pros:**
- Minimal frontend change (just send subscribe message)
- Server does the filtering
- Reduces unnecessary traffic

**Cons:**
- Need to implement subscription protocol on backend
- Need to handle subscription lifecycle (unsubscribe, cleanup)

## Recommendation

**Option A (Session-Based Subscription)** is the cleanest long-term solution.

The session_id timing issue can be solved:
1. Frontend generates a temporary session_id hint
2. Or: Backend returns session_id in HTTP response before streaming starts
3. Or: First event received triggers subscription update

## Implementation Plan (When Ready)

### Phase 1: Backend - Add session_id to all events
- [ ] Audit all GatewayEvent variants - ensure session_id is present
- [ ] For events that don't have it, add it

### Phase 2: Frontend - Session subscription
- [ ] Add `subscribeSession(sessionId, callback)` to transport interface
- [ ] Add `sessionCallbacks: Map<string, Set<callback>>` to HttpTransport
- [ ] Modify `handleEvent` to route by session_id
- [ ] Modify WebChatPanel to subscribe by session once known

### Phase 3: Handle subscription timing
- [ ] On `agent_started`, extract session_id and subscribe
- [ ] Buffer any events received before subscription is set up
- [ ] Or: have backend hold events until client confirms subscription

### Phase 4: Cleanup
- [ ] Remove conversation-based subscription for delegation events
- [ ] Update tests
- [ ] Update documentation

## Files Affected

| File | Changes |
|------|---------|
| `apps/ui/src/services/transport/interface.ts` | Add `subscribeSession()` method |
| `apps/ui/src/services/transport/http.ts` | Add session-based event routing |
| `apps/ui/src/features/agent/WebChatPanel.tsx` | Subscribe to session, handle session events |
| `gateway/src/events/mod.rs` | Ensure all events have session_id |
| `gateway/src/websocket/handler.rs` | (Optional) Server-side filtering |

## Questions to Resolve

1. **Subscription timing**: How do we subscribe to a session before we know its ID?
   - Option: Generate session_id on frontend, send with first message
   - Option: Subscribe after first event, accept we might miss some events
   - Option: Backend holds events until subscription confirmed

2. **Multiple sessions**: Can a user have multiple active sessions?
   - If yes, need multi-session subscription support
   - If no, can simplify to "current session" model

3. **Reconnection**: What happens if WebSocket disconnects mid-session?
   - Need to resubscribe on reconnect
   - May need to fetch missed events via HTTP

## Parking This

This is a significant architectural change. Current workarounds:
- User can refresh to see delegation results
- `/end` and `/new` commands work for session management
- Streaming works correctly now (after async race fix)

When ready to implement, start with Phase 1 (audit events) to understand scope.
