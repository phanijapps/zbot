# Chat Session Defects + Intent Analysis Restoration

**Date:** 2026-03-30
**Status:** Approved
**Scope:** 5 issues — 4 defects in chat session display, 1 intent analysis restoration

---

## Issues

| ID | Type | Title |
|----|------|-------|
| 1 | Defect | Session title not showing up on chat page |
| 2 | Defect | Thinking animation shows when loading previous chat from history |
| 3 | Defect | Final response not showing for resumed chats |
| 4 | Defect | Final response not showing for new chat sessions |
| 5 | Investigate | Intent analysis not running; sidebar should surface intent |

---

## Issue 1: Session Title Not Showing

### Root Cause
Title depends entirely on the agent calling `set_session_title` tool. If it doesn't call it (or calls it late), the title stays empty/"New Session". For resumed sessions, if the title was never set during execution, it's empty in the DB too.

### Fix
**Fallback title generation (frontend only):**
- In `mission-hooks.ts`, after `agent_started` fires, start a 10-second timer
- If `sessionTitle` is still empty when it expires, auto-generate a title from the first user message (truncate to ~50 chars, add "..." if needed)
- Persist the fallback title to the backend via the existing transport call

### Files Changed
- `apps/ui/src/features/chat/mission-hooks.ts` — add fallback timer in `agent_started` handler

---

## Issue 2: Thinking Animation on Resumed Chats

### Root Cause
`ExecutionNarrative.tsx` shows the thinking indicator based solely on the last block type (user message, incomplete tool). It never checks the session `status`. When loading a completed session where the last block is a user message or tool without output, the thinking animation renders despite the session being done.

### Fix
1. Pass `status` prop to `ExecutionNarrative` from `MissionControl.tsx`
2. Guard the thinking indicator: only show when `status === "running"`. Suppress for `"completed"`, `"idle"`, or `"error"`.

### Files Changed
- `apps/ui/src/features/chat/ExecutionNarrative.tsx` — accept `status` prop, add guard
- `apps/ui/src/features/chat/MissionControl.tsx` — pass `status` to `ExecutionNarrative`

---

## Issues 3 + 4: Final Response Not Showing

### Root Cause — Two Bugs

**Bug A (New chats):** The `agent_completed` handler in `mission-hooks.ts` only finalizes existing streaming blocks (`isStreaming: false`). It never creates a response block. If `turn_complete` is missed or arrives with empty `final_message`, and no tokens were streamed into a response block, the user sees nothing after execution ends.

**Bug B (Resumed chats):** The backend accumulates the final response and stores it in the conversation messages table (`role: "assistant"`), but never writes it as an `ExecutionLog` entry. When the UI loads a past session, it only reads execution logs — so there's no log to reconstruct a response block from.

### Fix — Backend

1. **Add `Response` variant** to `LogCategory` enum in `services/api-logs/src/types.rs`
2. **Emit ExecutionLog** in `runner.rs` after accumulating the final response, with `category: Response` and the accumulated content — alongside the existing `session_message()` call

### Fix — Frontend (New Chats)

3. In `mission-hooks.ts`, the `agent_completed` handler gets a safety net: if no response block exists in `blocks` when `agent_completed` fires, AND the event carries a `result` field (which `GatewayEvent::AgentCompleted` does), create a response block from it.

### Fix — Frontend (Resumed Chats)

4. In the session-loading logic in `mission-hooks.ts`, add a handler for `log.category === "response"` that creates a `type: "response"` narrative block with the log's message content.

### Files Changed
- `services/api-logs/src/types.rs` — add `Response` to `LogCategory`
- `gateway/gateway-execution/src/runner.rs` — emit response ExecutionLog after accumulation
- `apps/ui/src/features/chat/mission-hooks.ts` — safety net in `agent_completed` + response log handler in session loading

---

## Issue 5: Intent Analysis Restoration + Sidebar

### Root Cause
`analyze_intent()` was intentionally removed from the execution pipeline (commit `d681791`) in favor of a `first_turn_protocol` shard. This lost execution graph capability, semantic resource matching, hidden intent discovery, and ward recommendations. The shard just tells the agent to "recall, title, plan, execute" — no structured analysis.

### Fix — Re-activate with Transparency

**Backend:**

1. **Re-wire `analyze_intent()`** in `runner.rs::create_executor()` for root agent first turns. Restore the import, un-underscore `_user_message`, call `analyze_intent()` after `index_resources()`. Re-enable `inject_intent_context()` to enrich the system prompt.

2. **New `GatewayEvent::IntentAnalysisComplete`** in `gateway-events/src/lib.rs`:
   - `session_id: String`
   - `execution_id: String`
   - `primary_intent: String`
   - `hidden_intents: Vec<String>`
   - `recommended_skills: Vec<String>`
   - `recommended_agents: Vec<String>`
   - `ward_recommendation: Value`
   - `execution_strategy: Value` (includes approach + optional graph)

3. **Emit the event** in `runner.rs` right after `analyze_intent()` succeeds — before the executor starts.

4. **Log it** as an `ExecutionLog` with a new `Intent` log category so resumed sessions can reconstruct the sidebar section.

5. **Keep `first_turn_protocol` shard** — it complements intent analysis. The shard handles agent-driven behaviors (recall, title, plan). Intent analysis handles the structured orchestration layer (graph, ward setup, skill/agent matching).

**Frontend:**

6. **New state** in `mission-hooks.ts`:
   ```typescript
   const [intentAnalysis, setIntentAnalysis] = useState<IntentAnalysis | null>(null);
   ```

7. **Event handler** for `intent_analysis_complete` — populate the state from event data.

8. **Session loading** — handle `log.category === "intent"` to restore intent analysis state for resumed chats.

9. **New sidebar section in `IntelligenceFeed.tsx`** — progressive disclosure:
   - **Collapsed:** Primary intent badge + execution strategy (simple/tracked/graph)
   - **Expanded:** Hidden intents list, recommended skills, recommended agents, ward recommendation with reason, execution graph (node list or mermaid)
   - Positioned as **first section** in the sidebar (above Active Ward)

### Files Changed
- `gateway/gateway-execution/src/runner.rs` — re-wire analyze_intent, emit event + log
- `gateway/gateway-events/src/lib.rs` — add `IntentAnalysisComplete` variant
- `services/api-logs/src/types.rs` — add `Intent` to `LogCategory`
- `apps/ui/src/features/chat/mission-hooks.ts` — new state, event handler, session loading
- `apps/ui/src/features/chat/IntelligenceFeed.tsx` — new Intent Analysis section with progressive disclosure

---

## New Artifacts

| Artifact | Type | Location |
|----------|------|----------|
| `Response` | LogCategory variant | `services/api-logs/src/types.rs` |
| `Intent` | LogCategory variant | `services/api-logs/src/types.rs` |
| `IntentAnalysisComplete` | GatewayEvent variant | `gateway/gateway-events/src/lib.rs` |

---

## Build Order

1. Backend log categories (`Response` + `Intent`)
2. Backend response logging (fixes issues 3+4 for future sessions)
3. Frontend issues 1+2 (quick wins, no backend dependency)
4. Frontend issues 3+4 (response block safety net + session loading)
5. Backend intent analysis restoration + event emission
6. Frontend intent analysis sidebar

---

## Non-Goals

- No changes to the `first_turn_protocol` shard (it complements, not conflicts)
- No graph execution engine (intent analysis produces the graph; agent orchestration is separate)
- No changes to ward setup automation (was deleted in refactor; out of scope)
- No changes to spec file generation (was deleted in refactor; out of scope)
