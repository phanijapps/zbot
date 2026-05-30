# Reflective Memory Phase 1 Completion Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 1 of the reflective memory spec — fix correction_count accuracy, add ward-scoping, inject corrections unconditionally, inject active goals, and include tool calls in session summaries.

**Architecture:** All changes are in two files: `handoff_writer.rs` (write-side accuracy) and `invoke_bootstrap.rs` (read-side structured context). No new structs, no new DB tables. Corrections and goals are read from existing stores already wired in `InvokeBootstrap`.

**Tech Stack:** Rust, Tokio async, zero_stores traits (`MemoryFactStore`, `GoalAccess`), `agent_runtime::ChatMessage`

---

## Files Changed

| File | What changes |
|------|-------------|
| `gateway/gateway-execution/src/sleep/handoff_writer.rs` | Ward filter on `read_handoff_block`; real `correction_count`; tool-call names in LLM prompt |
| `gateway/gateway-execution/src/runner/invoke_bootstrap.rs` | Pass ward to `read_handoff_block`; inject corrections block; inject goals block |

---

## Task 1: Ward-scope `read_handoff_block` + fix real `correction_count`

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/handoff_writer.rs`

### Context

`read_handoff_block` currently ignores the current session's ward, so a "research-ward" session will show a "coding-ward" handoff. And `correction_count` is hardcoded `0`.

The `write` method already receives `agent_id` but names it `_agent_id` (unused). The fix:
1. Use `agent_id` in `write` to count corrections from the fact store.
2. Add `current_ward: Option<&str>` param to `read_handoff_block` and return `None` when wards differ.

- [ ] **Step 1.1: Write failing tests**

Add these two tests inside the `#[cfg(test)]` block in `handoff_writer.rs`, after test 7:

```rust
// ---- Test 8: read_handoff_block returns None when ward differs ----

#[tokio::test]
async fn read_handoff_block_returns_none_when_ward_differs() {
    let store = MockFactStore::new();
    let entry = HandoffEntry {
        summary: "Done something.".to_string(),
        session_id: "sess-1".to_string(),
        completed_at: Utc::now().to_rfc3339(),
        ward_id: "coding-ward".to_string(),
        intent_key: "ctx.sess-1.intent".to_string(),
        goal_count: 0,
        open_task_count: 0,
        correction_count: 0,
        turns: 3,
    };
    store.facts.lock().unwrap().insert(
        "handoff.latest".to_string(),
        serde_json::to_string(&entry).unwrap(),
    );
    let store: Arc<dyn zero_stores::MemoryFactStore> = store;
    // Different ward — should be filtered out
    assert!(
        read_handoff_block(&store, Some("research-ward")).await.is_none(),
        "mismatched ward should return None"
    );
    // Matching ward — should be returned
    assert!(
        read_handoff_block(&store, Some("coding-ward")).await.is_some(),
        "matching ward should return Some"
    );
    // No current ward — accept any (None = new session, don't block orientation)
    assert!(
        read_handoff_block(&store, None).await.is_some(),
        "None current_ward should accept any stored ward"
    );
}

// ---- Test 9: correction_count reflects real fact store count ----

#[tokio::test]
async fn correction_count_reflects_real_count() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MockFactStore::new();
    // Seed 2 correction facts
    store.corrections.lock().unwrap().push("Use write_file not shell".to_string());
    store.corrections.lock().unwrap().push("Never CPU offload".to_string());

    let writer = make_writer(&tmp, MockLlm::ok("Summary."), store.clone());
    writer
        .write_with_messages("sess-c", "agent-root", "test-ward", sample_messages(4))
        .await
        .unwrap();

    let raw = store.get("handoff.latest").unwrap();
    let entry: HandoffEntry = serde_json::from_str(&raw).unwrap();
    assert_eq!(entry.correction_count, 2);
}
```

Also add a `corrections: Arc<Mutex<Vec<String>>>` field to `MockFactStore` and implement `get_facts_by_category` on it to return those items as `MemoryFact` values:

```rust
#[derive(Clone, Default)]
struct MockFactStore {
    facts: Arc<Mutex<HashMap<String, String>>>,
    corrections: Arc<Mutex<Vec<String>>>,
}

impl MockFactStore {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    fn get(&self, key: &str) -> Option<String> {
        self.facts.lock().unwrap().get(key).cloned()
    }
    fn contains(&self, key: &str) -> bool {
        self.facts.lock().unwrap().contains_key(key)
    }
}
```

And add `get_facts_by_category` to the `impl MemoryFactStore for MockFactStore` block:

```rust
async fn get_facts_by_category(
    &self,
    _agent_id: &str,
    category: &str,
    _limit: usize,
) -> Result<Vec<zero_stores_traits::MemoryFact>, String> {
    if category != "correction" {
        return Ok(vec![]);
    }
    let items = self.corrections.lock().unwrap();
    Ok(items
        .iter()
        .enumerate()
        .map(|(i, c)| zero_stores_traits::MemoryFact {
            id: format!("fact-{i}"),
            agent_id: "agent-root".to_string(),
            scope: "global".to_string(),
            ward_id: "__global__".to_string(),
            category: "correction".to_string(),
            key: format!("correction.{i}"),
            content: c.clone(),
            confidence: 1.0,
            source_session_id: None,
            contradicted_by: None,
            created_at: String::new(),
            updated_at: String::new(),
            expires_at: None,
            valid_from: None,
            valid_until: None,
            superseded_by: None,
            pinned: false,
            epistemic_class: None,
            source_episode_id: None,
            source_ref: None,
        })
        .collect())
}
```

- [ ] **Step 1.2: Run tests to verify they fail**

```bash
cd /home/videogamer/projects/agentzero
cargo test -p gateway-execution handoff -- --nocapture 2>&1 | tail -30
```

Expected: compile error (wrong arg count to `read_handoff_block`) or test failures.

- [ ] **Step 1.3: Update `read_handoff_block` signature and add ward filter**

Change the function signature and add the ward check in `gateway/gateway-execution/src/sleep/handoff_writer.rs`:

```rust
pub async fn read_handoff_block(
    fact_store: &Arc<dyn zero_stores::MemoryFactStore>,
    current_ward: Option<&str>,
) -> Option<String> {
    let fact = fact_store
        .get_fact_by_key(HANDOFF_AGENT_SENTINEL, HANDOFF_SCOPE, HANDOFF_WARD, "handoff.latest")
        .await
        .ok()??;
    let entry: HandoffEntry = serde_json::from_str(&fact.content).ok()?;
    let completed_at = entry.completed_at.parse::<DateTime<Utc>>().ok()?;
    if (Utc::now() - completed_at).num_days() > HANDOFF_MAX_AGE_DAYS {
        return None;
    }
    // Ward filter: skip if the new session is a different ward (None = accept any)
    if let Some(cw) = current_ward {
        if !entry.ward_id.is_empty() && entry.ward_id != cw {
            return None;
        }
    }
    let date_str = completed_at.format("%Y-%m-%d").to_string();
    Some(format!(
        "## Last Session  ({date} · ward: {ward} · {turns} turns)\n\
         {summary}\n\n\
         Corrections active: {corrections} · Goals: {goals}\n\
         Full context: memory(action=get_fact, key=handoff.{sid})\n\
         Last intent:  memory(action=get_fact, key={intent_key})",
        date = date_str,
        ward = entry.ward_id,
        turns = entry.turns,
        summary = entry.summary,
        corrections = entry.correction_count,
        goals = entry.goal_count,
        sid = entry.session_id,
        intent_key = entry.intent_key,
    ))
}
```

- [ ] **Step 1.4: Add `agent_id` to `write` and `write_with_messages`, query real correction_count**

In `HandoffWriter`, change `write` to pass `agent_id` (drop the underscore prefix):

```rust
pub async fn write(&self, session_id: &str, agent_id: &str, ward_id: &str) {
    let messages_raw = match self
        .conversation_repo
        .get_session_conversation(session_id, 50)
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(session_id, "handoff: failed to load messages: {e}");
            return;
        }
    };
    let messages = self
        .conversation_repo
        .session_messages_to_chat_format(&messages_raw);
    if let Err(e) = self.write_with_messages(session_id, agent_id, ward_id, messages).await {
        tracing::warn!(session_id, "handoff: write failed: {e}");
    }
}
```

Change `write_with_messages` to accept `agent_id: &str` and query real correction count:

```rust
pub async fn write_with_messages(
    &self,
    session_id: &str,
    agent_id: &str,
    ward_id: &str,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    let turns = messages.iter().filter(|m| m.role == "user").count() as u32;

    let correction_count = self
        .fact_store
        .get_facts_by_category(agent_id, "correction", 200)
        .await
        .unwrap_or_default()
        .len() as u32;

    let input = HandoffInput { messages, ward_id: ward_id.to_string() };
    let summary = self.llm.summarize(&input).await?;
    let entry = HandoffEntry {
        summary,
        session_id: session_id.to_string(),
        completed_at: Utc::now().to_rfc3339(),
        ward_id: ward_id.to_string(),
        intent_key: format!("ctx.{session_id}.intent"),
        goal_count: 0,
        open_task_count: 0,
        correction_count,
        turns,
    };
    self.persist(session_id, &entry).await
}
```

- [ ] **Step 1.5: Fix existing tests that call old signatures**

Tests 1-3 call `write_with_messages("sess-abc", "test-ward", messages)`. Add `"agent-root"` as second arg:

```rust
// Old:
writer.write_with_messages("sess-abc", "test-ward", sample_messages(6)).await
// New:
writer.write_with_messages("sess-abc", "agent-root", "test-ward", sample_messages(6)).await
```

Also fix tests 5-7 which call `read_handoff_block(&store)` — add `None` as second arg:

```rust
// Old:
read_handoff_block(&store).await
// New:
read_handoff_block(&store, None).await
```

- [ ] **Step 1.6: Fix call site in `invoke_bootstrap.rs`**

In `finish_setup` (line ~320), change:

```rust
// Old:
if let Some(block) = crate::sleep::handoff_writer::read_handoff_block(store).await {
// New:
if let Some(block) = crate::sleep::handoff_writer::read_handoff_block(store, ward_id.as_deref()).await {
```

- [ ] **Step 1.7: Run tests**

```bash
cargo test -p gateway-execution handoff -- --nocapture 2>&1 | tail -30
```

Expected: all 9 tests pass.

- [ ] **Step 1.8: Cargo check**

```bash
cargo check --workspace 2>&1 | grep -E "^error" | head -20
```

Expected: no errors.

- [ ] **Step 1.9: Commit**

```bash
git add gateway/gateway-execution/src/sleep/handoff_writer.rs \
        gateway/gateway-execution/src/runner/invoke_bootstrap.rs
git commit -m "fix(handoff): ward-scoping + real correction_count"
```

---

## Task 2: Include tool-call names in LLM summary prompt

**Files:**
- Modify: `gateway/gateway-execution/src/sleep/handoff_writer.rs` (only `LlmHandoffWriter::summarize`)

### Context

The current filter only includes `user`/`assistant` text messages. Tool calls (the most important actions) are invisible. Fix: extract tool call names from assistant messages and include them inline as `[called: memory, shell]` annotations.

- [ ] **Step 2.1: Write a failing test**

Add inside the test module:

```rust
// ---- Test 10: summarize receives tool call annotations ----

#[tokio::test]
async fn summarize_receives_tool_call_annotations() {
    use agent_runtime::{ChatMessage, ToolCall};
    // Build a message with a tool_call
    let mut assistant_msg = ChatMessage::assistant("Let me check memory.".to_string());
    assistant_msg.tool_calls = Some(vec![ToolCall {
        id: "tc-1".to_string(),
        name: "memory".to_string(),
        input: serde_json::json!({"action": "get_fact"}),
    }]);
    let messages = vec![
        ChatMessage::user("What do you know about flux?".to_string()),
        assistant_msg,
    ];

    // CaptureLlm stores the prompt text passed to summarize
    #[derive(Clone, Default)]
    struct CaptureLlm(Arc<Mutex<String>>);
    #[async_trait::async_trait]
    impl HandoffLlm for CaptureLlm {
        async fn summarize(&self, input: &HandoffInput) -> Result<String, String> {
            // We can't test LlmHandoffWriter directly (it needs real provider)
            // so test the formatting helper instead
            let _ = input; // placeholder — see step 2.3
            Ok("ok".to_string())
        }
    }

    // Directly test the conversation-formatting logic
    let text = format_conversation_for_summary(&messages);
    assert!(text.contains("[called: memory]"), "tool call name missing: {text}");
}
```

- [ ] **Step 2.2: Extract `format_conversation_for_summary` helper**

In `handoff_writer.rs`, extract the conversation-building logic from `LlmHandoffWriter::summarize` into a free `pub(super) fn`:

```rust
pub(super) fn format_conversation_for_summary(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant" || m.role == "tool")
        .take(40)
        .map(|m| {
            let text = m.text_content();
            if m.role == "assistant" {
                if let Some(calls) = &m.tool_calls {
                    let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
                    if !names.is_empty() {
                        return format!("assistant [called: {}]: {}", names.join(", "), text);
                    }
                }
            }
            format!("{}: {}", m.role, text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

Then use it in `summarize`:

```rust
let conversation = format_conversation_for_summary(&input.messages);
```

- [ ] **Step 2.3: Run the test**

```bash
cargo test -p gateway-execution summarize_receives_tool_call -- --nocapture 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 2.4: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

- [ ] **Step 2.5: Commit**

```bash
git add gateway/gateway-execution/src/sleep/handoff_writer.rs
git commit -m "feat(handoff): include tool call names in LLM summary prompt"
```

---

## Task 3: Always-inject corrections at session start

**Files:**
- Modify: `gateway/gateway-execution/src/runner/invoke_bootstrap.rs`

### Context

The spec says "Corrections (always active)" — they must appear in context even if recall doesn't surface them. `InvokeBootstrap` already has `memory_store` and `config.agent_id`. We add a corrections block injected BEFORE the handoff so the agent reads: `[handoff, corrections, recall]`.

- [ ] **Step 3.1: Add `format_corrections_block` helper** (in `invoke_bootstrap.rs`)

Add near the top of the file (after the `use` block, before the struct):

```rust
fn format_corrections_block(facts: &[zero_stores_traits::MemoryFact]) -> Option<String> {
    if facts.is_empty() {
        return None;
    }
    let lines: Vec<String> = facts.iter().map(|f| format!("- {}", f.content)).collect();
    Some(format!("## Active Corrections\n{}", lines.join("\n")))
}
```

- [ ] **Step 3.2: Add corrections injection in `finish_setup`**

In `finish_setup`, just BEFORE the handoff injection block (before `if let Some(store) = &self.memory_store { ... read_handoff_block ...}`):

```rust
// Always-active corrections — injected unconditionally so agent never misses hard rules.
// Inserted before handoff so handoff ends up at history[0] (the agent reads it first).
if let Some(store) = &self.memory_store {
    match store
        .get_facts_by_category(&config.agent_id, "correction", 30)
        .await
    {
        Ok(facts) => {
            if let Some(block) = format_corrections_block(&facts) {
                history.insert(0, ChatMessage::system(block));
            }
        }
        Err(e) => {
            tracing::warn!(agent_id = %config.agent_id, "corrections inject failed: {e}");
        }
    }
}
```

- [ ] **Step 3.3: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

- [ ] **Step 3.4: Verify injection order in code**

After your edit, the order in `finish_setup` should be:

```
recall_unified(...)     → history.insert(0, recall_block)
format_corrections(...)  → history.insert(0, corrections_block)   // you just added
read_handoff_block(...)  → history.insert(0, handoff_block)        // existing
```

This means the agent reads: `handoff → corrections → recall`.
Match the spec's desired order.

- [ ] **Step 3.5: Commit**

```bash
git add gateway/gateway-execution/src/runner/invoke_bootstrap.rs
git commit -m "feat(handoff): always-inject active corrections at session start"
```

---

## Task 4: Inject active goals at session start

**Files:**
- Modify: `gateway/gateway-execution/src/runner/invoke_bootstrap.rs`

### Context

`InvokeBootstrap` already holds `goal_adapter: Option<Arc<dyn agent_tools::GoalAccess>>`. `GoalAccess::list_active(agent_id)` returns `Vec<GoalSummary>`. Each `GoalSummary` has `id`, `title`, `description`, `state`.

Insertion order: goals go between corrections and handoff:
```
handoff → goals → corrections → recall
```

So insert goals AFTER corrections inject, BEFORE handoff inject.

- [ ] **Step 4.1: Add `format_goals_block` helper**

In `invoke_bootstrap.rs`, near `format_corrections_block`:

```rust
fn format_goals_block(goals: &[agent_tools::GoalSummary]) -> Option<String> {
    let active: Vec<&agent_tools::GoalSummary> =
        goals.iter().filter(|g| g.state == "active").collect();
    if active.is_empty() {
        return None;
    }
    let lines: Vec<String> = active
        .iter()
        .map(|g| {
            if let Some(desc) = &g.description {
                format!("- {} — {}", g.title, desc)
            } else {
                format!("- {}", g.title)
            }
        })
        .collect();
    Some(format!("## Active Goals\n{}", lines.join("\n")))
}
```

- [ ] **Step 4.2: Add goals injection in `finish_setup`**

AFTER the corrections inject block, BEFORE the handoff inject block:

```rust
// Active goals — injected so agent picks up any in-flight objectives.
if let Some(adapter) = &self.goal_adapter {
    match adapter.list_active(&config.agent_id).await {
        Ok(goals) => {
            if let Some(block) = format_goals_block(&goals) {
                history.insert(0, ChatMessage::system(block));
            }
        }
        Err(e) => {
            tracing::warn!(agent_id = %config.agent_id, "goals inject failed: {e}");
        }
    }
}
```

- [ ] **Step 4.3: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

- [ ] **Step 4.4: Verify final injection order in code**

After your edits, the order in `finish_setup` should be:

```
recall_unified → history[0] = recall_block
corrections    → history[0] = corrections_block   (recall moves to [1])
goals          → history[0] = goals_block         (corrections at [1], recall at [2])
handoff        → history[0] = handoff_block       (goals at [1], corrections at [2], recall at [3])
```

Agent reads: `handoff → goals → corrections → recall`. Matches spec.

- [ ] **Step 4.5: Run clippy**

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error" | head -20
```

Fix any warnings before proceeding.

- [ ] **Step 4.6: Commit**

```bash
git add gateway/gateway-execution/src/runner/invoke_bootstrap.rs
git commit -m "feat(handoff): inject active goals at session start"
```

---

## Final Validation

- [ ] **Full workspace test**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: no failures.

- [ ] **Clippy clean**

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error"
```

Expected: empty.

- [ ] **Fmt check**

```bash
cargo fmt --all --check 2>&1
```

Expected: no output (clean).

---

## Self-Review Against Spec

| Spec requirement (Section 8) | Task |
|------------------------------|------|
| Handoff note on session end | ✅ Done in prev session |
| Direct lookup (not recall) | ✅ Done in prev session |
| Ward-scoping (same ward only) | Task 1 |
| Correction_count accurate | Task 1 |
| Tool calls in summary prompt | Task 2 |
| Corrections always-active | Task 3 |
| Active goals injected | Task 4 |
| `memory(action="consolidate_session")` API | Not in this plan (lower priority) |
| Targeted recall from handoff topics | Not in this plan (Phase 1.5) |
| Tiered recall redesign | Phase 5 (separate plan) |
