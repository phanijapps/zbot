# Persistent Chat — Final Design Spec (v2)

## Problem

The current research mode takes 2-5s before the agent starts responding. For quick conversational use, users want a fast, creative, always-on chat that remembers context across visits without the orchestration overhead.

## Core Model

**The DB is the context window.** Messages exist in the DB only while they're in the active context. When the middleware prunes messages, they are deleted from the DB. The agent's permanent memory lives in `memory_facts` (tagged `scope: "chat"`), not in messages.

```
Messages table (ephemeral)     Memory facts table (permanent)
┌────────────────────┐          ┌──────────────────────────┐
│ Last 30-50 turns   │          │ scope: "chat"            │
│ (live context)     │  prune → │ key: "user prefers X"    │
│ Deleted on prune   │  save  → │ key: "project status"    │
└────────────────────┘          │ key: "correction: don't Y"│
                                └──────────────────────────┘
```

## Design

### Single Permanent Session

- `chatSessionId` stored in `ExecutionSettings` (settings.json)
- Created on first `/chat` visit via `POST /api/chat/init`
- Reused forever — no "New Chat" button
- If session gets corrupted, user clears it from Settings

### Messages Are Ephemeral

- DB holds only the active context window
- Context editing middleware prunes old messages → **deletes them from DB**
- Before pruning, the middleware (or agent) saves important context to `memory_facts` with `scope: "chat"`
- On page load, `GET /api/sessions/:id/messages` returns exactly what the agent sees
- No sync problem — DB = LLM context = UI display

### Chat Memory Facts

Facts saved during chat use a distinct scope so they don't pollute research memory:

```json
{
  "scope": "chat",
  "category": "context",
  "key": "user prefers dark themes",
  "content": "User mentioned they always use dark mode"
}
```

The chat agent's recall searches `scope: "chat"` by default. Research agents search their own scopes. No cross-contamination.

### Agent Behavior

- **Temperature:** 1.0 (creative but tool-call safe, not 2.0)
- **Thinking:** Off by default for speed. Toggle in UI header.
- **No pre-loaded memory:** Agent uses `memory(action="recall", scope="chat")` organically when needed
- **No intent analysis, no resource indexing, no planning pipeline**
- **Multi-tool turns allowed** (single_action_mode disabled)
- **Delegation available** but not forced

### System Prompt

Uses existing lean prompt (SOUL + chat_instructions + OS + chat_protocol + tooling_skills). The `chat_protocol.md` shard adds:

```markdown
## Context Management
- Your messages are ephemeral. Old turns are pruned automatically.
- Use memory(action="save_fact", scope="chat", ...) to persist anything important.
- Use memory(action="recall", scope="chat") when you need past context.
- Don't save everything — only corrections, user preferences, key decisions.
```

### Context Pruning (Middleware-Driven)

Four-layer approach:

| Threshold | Mechanism | Action |
|-----------|-----------|--------|
| 70% | System nudge | Injects "[system] Context at 70%. Save important facts before pruning." |
| 80% | ContextEditingMiddleware | Clears old tool results with placeholders |
| 90% | Hard message deletion | Deletes oldest messages from DB, keeps last N |
| 100% | Truncation | Emergency last resort |

The middleware config for chat:
- `trigger_tokens`: 80% of model's context window
- `keep_tool_results`: 5 (most recent)
- After clearing tool results, if still over budget: delete oldest user/assistant message pairs from DB

### UI

Inherits from current FastChat but with changes:

| Feature | Behavior |
|---------|----------|
| Messages | Load from DB on mount — shows exactly what agent sees |
| Thinking | Inline collapsible blocks, toggle in header (brain icon) |
| Tool calls | Inline compact blocks (same as current) |
| Delegation | Shown as compact indicator (v2: expand to full lifecycle) |
| File pills | v2 — defer for now |
| New button | **Removed** — single permanent session |
| Attachments | Same drag-drop + file picker as current |

### API Endpoints

**New:**
- `POST /api/chat/init` — creates chat session if none exists, returns `{ sessionId, conversationId }`. Idempotent (returns existing if already created).
- `GET /api/sessions/:id/messages?limit=100` — returns messages for a session (for history loading on page mount)

**Modified:**
- `GET /api/settings` — returns `chatSessionId` in execution settings

### Data Flow

```
Page mount → GET /api/settings → chatSessionId
  ├─ If null → POST /api/chat/init → creates session → returns IDs
  └─ If exists → GET /api/sessions/:id/messages → load history → render

User sends message → executeAgent(root, convId, text, sessionId, "fast")
  → Runner skips intent analysis (is_fast_mode)
  → Loads messages from DB (= context window)
  → Middleware checks token budget
    ├─ Under 70%: proceed normally
    ├─ 70-80%: inject nudge system message
    ├─ 80-90%: clear old tool results (middleware)
    └─ 90%+: delete old messages from DB
  → LLM call with pruned messages
  → Stream response to UI
  → On turn complete: messages persisted to DB
```

## Phasing

### Phase 1 (Build Now)
- `chatSessionId` in settings.json
- `POST /api/chat/init` endpoint
- `GET /api/sessions/:id/messages` endpoint
- Wire context editing middleware for chat mode (enable with chat-appropriate thresholds)
- Message deletion on prune (not just hide)
- Chat-scoped memory facts (`scope: "chat"`)
- Temperature 1.0
- Remove "New" button from FastChat
- Inline thinking display (collapsible blocks with header toggle)
- Load history from DB on page mount

### Phase 2 (Defer)
- `compress_context` explicit agent tool
- File pills in chat
- Compact delegation blocks with expand
- Context pressure visualization
- "Clear context" button (soft reset without new session)

## Files to Modify

### Phase 1 Backend
| File | Change |
|------|--------|
| `gateway/gateway-services/src/settings.rs` | Add `chat_session_id: Option<String>` to ExecutionSettings |
| `gateway/src/http/chat.rs` | New: `POST /api/chat/init` endpoint |
| `gateway/src/http/mod.rs` | Register chat routes + messages endpoint |
| `gateway/gateway-execution/src/runner.rs` | Wire middleware for chat, message deletion on prune |
| `gateway/gateway-execution/src/invoke/executor.rs` | Set temperature=1.0, enable context_editing for fast mode |
| `gateway/templates/shards/chat_protocol.md` | Update with context management instructions |
| `services/execution-state/src/repository.rs` | Add `delete_messages_before(session_id, message_id)` method |

### Phase 1 Frontend
| File | Change |
|------|--------|
| `apps/ui/src/features/chat/fast-chat-hooks.ts` | Settings-based session, history loading on mount, thinking event handling |
| `apps/ui/src/features/chat/FastChat.tsx` | Remove New button, add thinking toggle, load history |
| `apps/ui/src/features/chat/ThinkingBlock.tsx` | New: collapsible thinking/reasoning display |
| `apps/ui/src/styles/components.css` | ThinkingBlock styles |

## Decisions Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Session storage | settings.json | Survives browser clear, shared across tabs, canonical config store |
| Message persistence | Ephemeral (deleted on prune) | DB = context window. No sync problem. Memory facts are the permanent record. |
| Chat facts scope | `scope: "chat"` | Prevents pollution of research agent memory |
| Temperature | 1.0 | Creative but tool-call safe. 2.0 breaks JSON in tool calls. |
| Context pruning | Middleware-driven (v1), agent tool (v2) | Middleware already exists and works. Agent tool is nice-to-have. |
| History display | DB = what you see | No "load more", no scroll-back. DB IS the context window. |
| Thinking display | Inline collapsible, header toggle | Power users want it, casual users don't. Toggle respects both. |
| `compress_context` tool | Phase 2 | Middleware handles v1. Tool adds explicit agent control later. |
