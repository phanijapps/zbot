# E2E Real-World WebSocket Fix — Change Log

All changes tracked here. Each entry includes: what changed, why, and which files.

## Investigation (read-only)

- **Traced full event flow**: UI → WS invoke → backend handler → event bus → subscription router → WS client
- **Identified root cause**: Backend routes events by `session_id` (sess-xxx). UI initially subscribes by client-minted `conversation_id` (research-xxx). Events never match the first subscription.
- **Mock works because**: Mock gateway sends events directly down the WS with rewritten `conversation_id` matching the UI's subscription — bypasses subscription manager entirely.
- **E2e baseline**: `simple-qa.ui.spec.ts` passes (34.2s). 164/165 research-v2 unit tests pass.

## Change 1: Transport-level debug logging

- **File**: `apps/ui/src/services/transport/http.ts`
- **What**: Add `console.debug` to `handleWebSocketMessage()` to log every incoming WS message type
- **Why**: In real-world testing, user only sees ping/pong. Need visibility into what messages arrive.
- **Approved**: yes
- **Status**: applied

## Change 2: Diagnostic logging in sendMessage + event handler

- **Files**: `apps/ui/src/features/research-v2/useResearchSession.ts`
- **What**: 
  - In `sendMessage()`: log convId and executeAgent result
  - In `makeEventHandler()`: log every event type, session_id, conversation_id, execution_id
- **Why**: Need to see (1) if invoke succeeds, (2) if any events arrive at the handler
- **Approved**: yes
- **Status**: applied

## Change 3 (proposed): Fix subscription to route by session_id after invoke_accepted

- **File**: `apps/ui/src/features/research-v2/useResearchSession.ts`
- **What**: In `makeEventHandler`, when `invoke_accepted` arrives, immediately subscribe to the `session_id` with `scope: "all"` so events routed by `session_id` on the server are received by the client. Currently the UI only subscribes to `session_id` via the R14g effect which is async and may race with early events.
- **Why**: Backend routes ALL events by `session_id` only. The initial `research-uuid` subscription only receives `invoke_accepted` (sent directly). All real events go through the event router which routes by `session_id`.
- **Risk**: LOW — R14g already does this subscription; we'd just do it sooner (in the event handler instead of after a React re-render cycle).
- **Approved**: pending

## Key finding: conversation_id coverage

17/24 ServerMessage variants carry `conversation_id`. The core streaming events (Token, ToolCall, ToolResult, Thinking, AgentStarted, AgentCompleted, TurnComplete, Heartbeat) ALL carry it, so the initial `research-uuid` subscription SHOULD receive them. Events WITHOUT conversation_id (WardChanged, IntentAnalysis*, SessionTitleChanged) can only be received via a `session_id`-keyed subscription.

## Next step: Run real gateway and check diagnostic logs

Before making Change 3, run the real gateway and check browser console for:
1. `[WS] message: connected` → WS connected?
2. `[research-v2] sendMessage: executeAgent result true/false` → invoke succeeded?
3. `[WS] message: invoke_accepted` → server accepted?
4. `[WS] message: agent_started` → any events after invoke?
5. `[research-v2] event: agent_started` → events reaching handler?

If step 3 succeeds but step 4 shows no events → backend execution issue (not UI).
If step 4 shows events but step 5 doesn't → client routing issue (Change 3 needed).
