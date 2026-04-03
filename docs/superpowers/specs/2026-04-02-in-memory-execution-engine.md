# In-Memory Execution Engine — Architecture Spec

## Vision

Replace the DB-mediated conversation model with an in-memory execution engine. The executor stays alive across delegations. Messages live in a vec, persisted asynchronously to JSONL. The DB becomes a lightweight session index. The result: sub-second delegation handoffs instead of 15-30 second continuation cycles.

## Current Architecture (slow)

```
Root executor starts
  → LLM call (messages from DB)
  → Tool call → tool result → append to DB
  → Delegate → executor returns Ok(()) → save to DB
  → Gateway: update execution state in DB
  → Subagent spawns → subagent executor runs
  → Subagent completes → result saved to DB
  → Gateway: complete_delegation in DB
  → Gateway: fire continuation
  → Continuation handler: load 200 messages from DB
  → Build new executor → new LLM call with full history
  → Root decides next step → 2-3 LLM calls → delegate again
  → Repeat...
```

**Cost per delegation round-trip:** 15-30 seconds of pure plumbing
- DB writes: ~5ms each × 10+ writes per delegation
- History load: 200 messages from DB → deserialize → build vec: ~50-200ms
- New executor construction: ~100ms
- Root re-orientation: 2-3 LLM calls to re-read context: ~10-20s
- Total overhead for a 6-step plan: **90-180 seconds of non-productive work**

## Target Architecture (fast)

```
Root executor starts
  → LLM call (messages from in-memory vec)
  → Tool call → tool result → append to vec + async JSONL write
  → Delegate → executor PAUSES (does not exit)
  → Subagent spawns → runs in separate task
  → Subagent completes → result sent via channel
  → Root executor RESUMES → result appended to vec
  → Next LLM call (same vec, same context, no reload)
  → Repeat...
```

**Cost per delegation round-trip:** ~1-2 seconds
- Channel receive: microseconds
- Vec append: nanoseconds
- Next LLM call: immediate (same context window, no re-orientation)
- Total overhead for a 6-step plan: **6-12 seconds**

## Core Changes

### 1. Executor Stays Alive During Delegation

Current: `execute_with_tools_loop` breaks when delegation detected, returns `Ok(())`.

New: When delegation is detected, the executor **pauses** on a channel:
```rust
// In the tool result processing, when delegate action detected:
if let Some(delegate) = &actions.delegate {
    on_event(StreamEvent::ActionDelegate { ... });

    // Send delegation request
    delegation_tx.send(DelegationRequest { ... });

    // PAUSE — wait for subagent result on channel
    let result = delegation_rx.recv().await;

    // Subagent completed — append result to current_messages
    current_messages.push(ChatMessage::user(format!(
        "[Delegation result from {}]\n{}",
        delegate.agent_id, result
    )));

    // Continue the loop — next LLM call sees the result immediately
    continue;
}
```

No executor exit. No continuation handler. No history reload. Root just pauses, gets the result, and continues.

### 2. In-Memory Conversation Store

Replace `ConversationRepository` (SQLite) with:

```rust
/// In-memory conversation store with async JSONL persistence.
pub struct ConversationStore {
    /// Full conversation history — append-only
    messages: Vec<ChatMessage>,
    /// JSONL file for durability
    jsonl_path: PathBuf,
    /// Async writer handle
    writer_tx: mpsc::UnboundedSender<ChatMessage>,
}

impl ConversationStore {
    /// Create new store for a session
    pub fn new(session_id: &str, data_dir: &Path) -> Self {
        let jsonl_path = data_dir.join("conversations").join(format!("{}.jsonl", session_id));
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();

        // Spawn background writer
        tokio::spawn(async move {
            jsonl_writer(jsonl_path.clone(), writer_rx).await;
        });

        Self { messages: Vec::new(), jsonl_path, writer_tx }
    }

    /// Load from existing JSONL (crash recovery / session resume)
    pub fn load(session_id: &str, data_dir: &Path) -> Result<Self, String> { ... }

    /// Append a message — instant (in-memory + async file write)
    pub fn append(&mut self, message: ChatMessage) {
        self.messages.push(message.clone());
        let _ = self.writer_tx.send(message); // async, non-blocking
    }

    /// Get all messages (for LLM context building)
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get compressed view for LLM (applies compaction to a clone)
    pub fn compressed_context(&self, max_tokens: u64) -> Vec<ChatMessage> {
        // Clone and compress — the full vec is never modified
        let mut context = self.messages.clone();
        compress_for_llm(&mut context, max_tokens);
        context
    }
}
```

### 3. Session Index in DB (metadata only)

```sql
-- Simplified schema — no message content in DB
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    status TEXT DEFAULT 'active',
    ward_id TEXT,
    title TEXT,
    agent_id TEXT,
    created_at TEXT,
    updated_at TEXT
);

CREATE TABLE executions (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id),
    agent_id TEXT,
    status TEXT DEFAULT 'running',
    parent_execution_id TEXT,
    created_at TEXT
);
```

No `conversation_messages` table. No `session_messages`. Messages live in JSONL + memory.

### 4. JSONL File Format

```
data/conversations/sess-abc123.jsonl
```

Each line is a JSON object:
```json
{"ts":"2026-04-02T20:15:23Z","role":"system","content":"You are an orchestrator..."}
{"ts":"2026-04-02T20:15:24Z","role":"user","content":"Analyze NVDA..."}
{"ts":"2026-04-02T20:15:30Z","role":"assistant","content":"","tool_calls":[{"id":"call_1","name":"memory","arguments":{...}}]}
{"ts":"2026-04-02T20:15:31Z","role":"tool","content":"{...}","tool_call_id":"call_1"}
{"ts":"2026-04-02T20:16:00Z","role":"assistant","content":"","tool_calls":[{"id":"call_2","name":"delegate_to_agent","arguments":{...}}]}
{"ts":"2026-04-02T20:18:00Z","role":"user","content":"[Delegation result from code-agent]\nFiles created: ..."}
```

Subagent results are appended as user messages to the root's JSONL. Subagents have their own JSONL files for audit.

### 5. Compaction (in-memory only)

The JSONL file is NEVER compacted — it's the full audit trail. Compaction only happens on the in-memory `llm_context` view:

```rust
fn compress_for_llm(messages: &mut Vec<ChatMessage>, max_tokens: u64) {
    let estimated = estimate_total_tokens(messages);
    if estimated < (max_tokens * 70 / 100) as usize {
        return; // Under 70% — no compression needed
    }

    // Phase 1: Compress old assistant messages
    compress_old_assistant_messages(messages, 20);

    // Phase 2: Clear old tool results (preserve file paths)
    let boundary = messages.len().saturating_sub(20);
    for i in 0..boundary {
        if messages[i].role == "tool" {
            let preserved = extract_key_info(&messages[i].content);
            messages[i].content = format!("[cleared — {}]", preserved);
        }
    }

    // Phase 3: Drop if still over budget
    // ... existing drop logic
}
```

### 6. UI Integration

**Active sessions:** UI reads from WebSocket stream (already works). No change.

**Page refresh on active session:** New API endpoint reads from in-memory vec:
```
GET /api/sessions/{id}/messages → returns messages from ConversationStore
```

**Closed/historical sessions:** Read from JSONL file:
```
GET /api/sessions/{id}/messages → reads and parses JSONL file
```

**Session list:** DB query on session metadata table (id, title, ward, status, created_at).

**Tool call display:** UI shows only tool calls for current session from the WebSocket stream. Logs page reads from JSONL or log files.

### 7. Subagent Communication

Current: subagent completes → writes to DB → gateway fires continuation → new executor loads from DB.

New:
```rust
// In root's executor, when delegation is requested:
let (result_tx, result_rx) = tokio::sync::oneshot::channel();

// Send delegation with result channel
delegation_tx.send(DelegationRequest {
    // ... existing fields
    result_channel: Some(result_tx),  // NEW: channel for direct result
});

// Pause executor — wait for result
match tokio::time::timeout(Duration::from_secs(600), result_rx).await {
    Ok(Ok(result)) => {
        // Subagent completed — append result to conversation
        current_messages.push(ChatMessage::user(format!(
            "[Delegation result from {}]\n{}", agent_id, result
        )));
        store.append(current_messages.last().unwrap().clone());
        continue; // Resume loop immediately
    }
    Ok(Err(_)) => {
        // Channel closed — subagent crashed
        current_messages.push(ChatMessage::user(
            "[Delegation failed — subagent crashed]".to_string()
        ));
        continue;
    }
    Err(_) => {
        // Timeout — 10 minutes
        current_messages.push(ChatMessage::user(
            "[Delegation timed out after 10 minutes]".to_string()
        ));
        continue;
    }
}
```

The subagent's spawn handler sends the result back through the oneshot channel. Root never exits, never reloads.

---

## Stretch Goal: Simplified Goal-Oriented Agent

With the in-memory engine, the agent model simplifies dramatically:

### Current complexity (what we can remove)

| Component | Current Purpose | With In-Memory Engine |
|-----------|----------------|----------------------|
| Continuation handler | Resumes root after delegation | **REMOVED** — root stays alive |
| Batch writer (conversation) | Batches DB writes | **REMOVED** — async JSONL append |
| `complete_execution` for delegations | Marks root done, requests continuation | **REMOVED** — root is still running |
| `request_continuation` / `SessionContinuationReady` | Signals root to resume | **REMOVED** — channel-based |
| `get_session_conversation(200)` | Loads history from DB | **REMOVED** — in-memory vec |
| `stopped_for_delegation` flag | Prevents Done event on delegation | **REMOVED** — executor doesn't stop |
| `has_pending_delegations` check | Prevents premature completion | **SIMPLIFIED** — root knows it's waiting |
| Conversation repository | Stores/retrieves messages | **REPLACED** — ConversationStore |

### What the agent loop becomes

```
loop {
    // LLM call with current context
    let response = llm.chat(store.compressed_context(context_window)).await;

    // Process tool calls
    for tool_call in response.tool_calls {
        if tool_call.name == "delegate_to_agent" {
            // Inline delegation — pause and wait
            let result = delegate_and_wait(&tool_call.args).await;
            store.append(result_message);
        } else if tool_call.name == "respond" {
            // Done — send to user
            return Ok(());
        } else {
            // Regular tool — execute immediately
            let result = execute_tool(&tool_call).await;
            store.append(result_message);
        }
    }
}
```

No continuation. No state machine. No DB round-trips. Just a loop that pauses when delegating and resumes when the result arrives.

### Root agent becomes trivially simple

```
1. Recall context
2. Enter ward
3. Read specs/plan.md (if exists)
4. Delegate step by step (each delegation is a pause-and-resume, not exit-and-restart)
5. Synthesize and respond
```

The complexity we built (delegation pause, continuation chaining, pending_delegations counter, race conditions) all goes away because the executor never exits.

---

## Migration Path

### Phase 1: ConversationStore + JSONL (foundation)
- Create `ConversationStore` with in-memory vec + async JSONL writer
- Create `data/conversations/` directory
- Add API endpoint for reading messages from store
- Keep DB conversation writes in parallel (dual-write) for safety

### Phase 2: Executor Inline Delegation (the big change)
- Add `delegation_result_channel` to DelegationRequest
- Modify executor to pause-and-wait instead of exit-and-restart
- Subagent spawn handler sends result through channel
- Remove continuation handler
- Remove `stopped_for_delegation`, `has_pending_delegations` machinery

### Phase 3: Remove DB Conversation Tables
- Stop writing to conversation DB
- Remove dual-write
- Remove `ConversationRepository` dependency from runner/spawn
- Drop conversation tables from schema

### Phase 4: Simplify Session Management
- Remove `request_continuation` / `SessionContinuationReady`
- Remove continuation handler task
- Simplify `complete_execution` (no delegation checks needed)
- Clean up session state machine

---

## Risks

1. **Memory pressure** — 10 concurrent sessions × 500 messages each = ~50MB. Manageable. Add TTL eviction for idle sessions.

2. **Daemon crash** — lose in-memory state. Mitigated by JSONL. On restart: read JSONL, rebuild vec, resume. Messages between last JSONL flush and crash are lost (seconds at most).

3. **Long-running sessions** — JSONL file grows unbounded. Add rotation: when file > 10MB, archive to `{session_id}.1.jsonl` and start fresh. Or just accept large files — disk is cheap.

4. **Subagent timeout** — if subagent hangs, root executor is blocked. The 10-minute timeout handles this, but the executor is consuming a thread. Use `tokio::select!` with a cancellation token.

5. **UI consistency** — during delegation, root is paused. UI needs to show "waiting for code-agent" state. The WebSocket stream already shows DelegationStarted/Completed events — UI just needs to interpret them.

---

## Expected Impact

| Metric | Current | Target |
|--------|---------|--------|
| Delegation round-trip overhead | 15-30s | 1-2s |
| 6-step plan total overhead | 90-180s | 6-12s |
| History load per continuation | 200 msgs from DB | 0 (in-memory) |
| Root re-orientation LLM calls | 2-3 per continuation | 0 (context preserved) |
| Codebase complexity | Continuation handler + state machine + race conditions | Single loop with channel await |
| Conversation DB writes per session | 50-200 | 0 (JSONL only) |
