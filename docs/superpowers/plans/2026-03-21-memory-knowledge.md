# Memory & Knowledge System Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat memory taxonomy with a structured 5-category system, fix the empty knowledge graph (entity dedup), wire graph-enriched recall, remove ward file-based memory, and update distillation to populate the graph.

**Architecture:** Update the memory tool's category validator, fix entity dedup in knowledge graph storage, wire `recall_with_graph()` in the runner, rewrite the distillation prompt for the new taxonomy, remove `.ward_memory.json` from ward and memory tools, update all shards and docs atomically.

**Tech Stack:** Rust, SQLite (memory_facts + knowledge_graph), serde_json

**Spec:** `docs/superpowers/specs/2026-03-21-memory-knowledge-design.md`

---

## File Structure

| File | Change |
|------|--------|
| `runtime/agent-tools/src/tools/memory.rs` | Update valid_categories, remove ward scope |
| `runtime/agent-tools/src/tools/ward.rs` | Remove .ward_memory.json references |
| `services/knowledge-graph/src/types.rs` | Add `File` to EntityType enum |
| `services/knowledge-graph/src/storage.rs` | Entity lookup-before-insert dedup |
| `gateway/gateway-execution/src/distillation.rs` | Rewrite DEFAULT_DISTILLATION_PROMPT, entity dedup in distill() |
| `gateway/gateway-execution/src/runner.rs` | Switch recall() to recall_with_graph(), thread memory_recall into continuation |
| `gateway/gateway-execution/src/recall.rs` | Ward-aware query enrichment |
| `gateway/templates/shards/memory_learning.md` | Update with new taxonomy |

---

## Chunk 1: Memory Taxonomy + Ward Normalization

### Task 1: Update memory tool categories and remove ward scope

**Files:**
- Modify: `runtime/agent-tools/src/tools/memory.rs`

- [ ] **Step 1: Update valid_categories**

At line 515, change:
```rust
let valid_categories = ["preference", "decision", "pattern", "entity", "instruction", "correction"];
```
To:
```rust
let valid_categories = ["user", "pattern", "domain", "instruction", "correction"];
```

- [ ] **Step 2: Remove "ward" from scope enum in parameters_schema**

At lines 237-241, change:
```rust
"enum": ["agent", "shared", "ward"],
```
To:
```rust
"enum": ["agent", "shared"],
```

Update the description too:
```rust
"description": "Memory scope: 'agent' (per-agent), 'shared' (cross-session)"
```

- [ ] **Step 3: Remove the "ward" arm from resolve_memory_path**

At lines 114-125, delete the entire `"ward" =>` match arm.

- [ ] **Step 4: Update tool description**

At lines 202-208, update the description string to remove `ward` scope mention:
```rust
"Persistent memory for storing facts, notes, and context across sessions. \
Actions: get/set/delete/list/search (key-value store), \
save_fact (structured fact with category/key/content/confidence — automatically embedded for semantic search), \
recall (hybrid semantic + keyword search over saved facts). \
Scopes: 'agent' (default), 'shared' (cross-session). \
Shared memory requires a 'file' parameter: user_info, workspace, patterns, or session_summaries."
```

- [ ] **Step 5: Compile and test**

Run: `cargo check -p agent-tools`
Run: `cargo test -p agent-tools`
Expected: Pass (existing tests don't use ward scope or old categories)

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-tools/src/tools/memory.rs
git commit -m "feat(memory): update taxonomy to 5 categories, remove ward scope"
```

---

### Task 2: Remove .ward_memory.json from ward tool

**Files:**
- Modify: `runtime/agent-tools/src/tools/ward.rs`

- [ ] **Step 1: Read the file to find all .ward_memory.json references**

Find: `WARD_MEMORY_FILE` constant (line 14), `load_ward_memory` function (lines 62-72), and all usages.

- [ ] **Step 2: Remove WARD_MEMORY_FILE constant**

Delete line 14:
```rust
const WARD_MEMORY_FILE: &str = ".ward_memory.json";
```

- [ ] **Step 3: Remove load_ward_memory function**

Delete the function at lines 62-72.

- [ ] **Step 4: Update ward actions that use ward memory**

Find all places that call `load_ward_memory` or reference `.ward_memory.json`. Replace ward memory loading with an empty `json!({})` or remove the field from the response if it was returning ward_memory data.

The `ward(action="use")` response currently includes `"ward_memory": self.load_ward_memory(...)`. Change to not include it, or include `"ward_memory": {}`.

- [ ] **Step 5: Remove .ward_memory.json creation in tests**

At line 381, remove: `std::fs::write(ward_dir.join(".ward_memory.json"), "{}").unwrap();`

- [ ] **Step 6: Compile and test**

Run: `cargo check -p agent-tools`
Run: `cargo test -p agent-tools`

- [ ] **Step 7: Commit**

```bash
git add runtime/agent-tools/src/tools/ward.rs
git commit -m "refactor(ward): remove .ward_memory.json, AGENTS.md is ward memory"
```

---

### Task 3: Update memory_learning shard (atomic with taxonomy change)

**Files:**
- Modify: `gateway/templates/shards/memory_learning.md`

- [ ] **Step 1: Rewrite with new taxonomy**

```markdown
MEMORY & LEARNING

Persistent memory across sessions via `memory` tool.

## Categories
Use these categories for `save_fact`:
- `user` — preferences, style, capabilities (permanent)
- `pattern` — how-to knowledge, error workarounds, workflows (reinforced by reuse)
- `domain` — domain knowledge with hierarchical keys: `domain.finance.lmnd.outlook` (decays with time)
- `instruction` — standing orders, workflow rules (permanent)
- `correction` — corrections to agent behavior (permanent)

## Key Format
Use dot-notation hierarchy: `{category}.{domain}.{subdomain}.{topic}`
Examples:
- `user.report_style` = "Professional HTML with charts"
- `pattern.yfinance.multiindex` = "Flatten: [c[0] for c in df.columns]"
- `domain.finance.lmnd.outlook` = "Bullish short-term, RSI 74.9"
- `instruction.coding.tests` = "Always verify code runs before finishing"
- `correction.coding.no_v2` = "Fix the original file, never create _v2"

## Save Immediately
Don't batch — save as you learn:
- `memory(action="save_fact", category="pattern", key="pattern.yfinance.multiindex", content="...", confidence=0.9)`

## Error Patterns
- `pattern.error.powershell_heredoc` = "Use apply_patch, not heredocs"
- `pattern.error.delegation_overflow` = "Keep subagent tasks focused"

## Success Patterns
- `pattern.workflow.stock_analysis` = "data-analyst + yf-data + yf-signals + coding"
```

- [ ] **Step 2: Compile (shard is embedded at compile time)**

Run: `cargo check -p gateway-templates`
Run: `cargo test -p gateway-templates`

- [ ] **Step 3: Delete on-disk shard so it regenerates**

```bash
rm -f ~/Documents/zbot/config/shards/memory_learning.md
```

- [ ] **Step 4: Commit**

```bash
git add gateway/templates/shards/memory_learning.md
git commit -m "feat(memory): update memory_learning shard with new 5-category taxonomy"
```

---

## Chunk 2: Knowledge Graph Fixes

### Task 4: Add File to EntityType enum

**Files:**
- Modify: `services/knowledge-graph/src/types.rs`

- [ ] **Step 1: Add File variant to EntityType**

At line 11-26, add `File` before `Custom`:
```rust
pub enum EntityType {
    Person,
    Organization,
    Location,
    Concept,
    Tool,
    Project,
    File,
    Custom(String),
}
```

- [ ] **Step 2: Update from_str to handle "file"**

In the `from_str` implementation (line 50-60), add:
```rust
"file" => EntityType::File,
```

- [ ] **Step 3: Update as_str / Display to handle File**

Add the reverse mapping:
```rust
EntityType::File => "file",
```

- [ ] **Step 4: Compile and test**

Run: `cargo check -p knowledge-graph`
Run: `cargo test -p knowledge-graph`

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/types.rs
git commit -m "feat(graph): add File entity type"
```

---

### Task 5: Fix entity dedup — lookup before insert

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add find_entity_by_name to storage.rs**

Add a new function near the entity storage functions:

```rust
/// Find an existing entity by agent_id + name (case-insensitive).
pub fn find_entity_by_name(conn: &Connection, agent_id: &str, name: &str) -> GraphResult<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM kg_entities WHERE agent_id = ?1 AND name = ?2 COLLATE NOCASE LIMIT 1"
    ).map_err(GraphError::Database)?;

    let result = stmt.query_row(params![agent_id, name], |row| {
        row.get::<_, String>(0)
    });

    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(GraphError::Database(e)),
    }
}
```

- [ ] **Step 2: Add bump_entity_mention to storage.rs**

```rust
/// Increment mention count and update last_seen for an existing entity.
pub fn bump_entity_mention(conn: &Connection, entity_id: &str) -> GraphResult<()> {
    conn.execute(
        "UPDATE kg_entities SET mention_count = mention_count + 1, last_seen_at = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), entity_id],
    ).map_err(GraphError::Database)?;
    Ok(())
}
```

- [ ] **Step 3: Update distillation.rs entity creation to dedup**

In `distillation.rs`, find the entity creation loop (around line 264-279). Change from creating new Entity every time to lookup-first:

Before:
```rust
let entity = Entity::new(agent_id.to_string(), EntityType::from_str(&ee.entity_type), ee.name.clone());
entity_map.insert(ee.name.clone(), entity.id.clone());
```

After:
```rust
// Dedup: reuse existing entity if found by name
let entity_id = if let Some(ref graph) = self.graph_storage {
    let conn = graph.connection().lock().await;
    match storage::find_entity_by_name(&conn, agent_id, &ee.name) {
        Ok(Some(existing_id)) => {
            storage::bump_entity_mention(&conn, &existing_id).ok();
            tracing::debug!(name = %ee.name, "Reusing existing entity");
            existing_id
        }
        _ => {
            let entity = Entity::new(agent_id.to_string(), EntityType::from_str(&ee.entity_type), ee.name.clone());
            let id = entity.id.clone();
            // store_entity will be called below
            id
        }
    }
} else {
    let entity = Entity::new(agent_id.to_string(), EntityType::from_str(&ee.entity_type), ee.name.clone());
    entity.id.clone()
};
entity_map.insert(ee.name.clone(), entity_id);
```

Note: Read the actual distillation.rs code first to understand how `graph_storage` is accessed and how `store_knowledge` is called. The pattern above is directional — adapt to the actual code structure.

- [ ] **Step 4: Compile and test**

Run: `cargo check --workspace`
Run: `cargo test -p knowledge-graph`

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/storage.rs gateway/gateway-execution/src/distillation.rs
git commit -m "fix(graph): entity dedup — lookup by name before creating new"
```

---

### Task 6: Rewrite distillation prompt

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Replace DEFAULT_DISTILLATION_PROMPT**

At line 480-510, replace the entire constant with the new taxonomy. The prompt should instruct the LLM to:

- Use 5 categories: `user`, `pattern`, `domain`, `instruction`, `correction`
- Use dot-notation keys: `domain.finance.lmnd.outlook`
- Extract entities with types: `person`, `organization`, `project`, `tool`, `concept`, `file`
- Extract relationships: `related_to`, `uses`, `created`, `part_of`, `is_in`, `has_module`, `exports`, `prefers`, `analyzed_by`
- Include ward file summaries as `domain.{subdomain}.data_available` facts
- Max 20 facts, 20 entities, 20 relationships per session
- Confidence: 0.9+ explicit, 0.7-0.9 implied, 0.5-0.7 inferred

- [ ] **Step 2: Delete on-disk distillation prompt so it regenerates**

```bash
rm -f ~/Documents/zbot/config/distillation_prompt.md
```

- [ ] **Step 3: Compile**

Run: `cargo check -p gateway-execution`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): rewrite prompt for new 5-category taxonomy and richer graph extraction"
```

---

## Chunk 3: Recall Improvements

### Task 7: Wire recall_with_graph in runner

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Change recall() to recall_with_graph() at line ~433**

Before:
```rust
match recall.recall(&config.agent_id, &message, 10).await {
    Ok(facts) if !facts.is_empty() => {
        let context = super::recall::format_recalled_facts(&facts);
        history.insert(0, ChatMessage::system(context));
```

After:
```rust
match recall.recall_with_graph(&config.agent_id, &message, 10).await {
    Ok(result) if !result.facts.is_empty() => {
        let context = result.formatted;
        history.insert(0, ChatMessage::system(context));
```

Note: Read `recall.rs` to verify the `RecallResult` struct fields. The `recall_with_graph` method returns a `RecallResult` which has `facts: Vec<ScoredFact>` and `formatted: String` (includes graph context).

- [ ] **Step 2: Thread memory_recall into invoke_continuation**

Check current `invoke_continuation` signature (line ~1084). It already has `memory_repo` and `embedding_client` from the previous threading work. Add `memory_recall: Option<Arc<super::recall::MemoryRecall>>`.

In `spawn_continuation_handler` (line ~258), clone `self.memory_recall` (if it exists as a field) and pass it.

In `invoke_continuation`, after loading history, add recall if history has delegation results:
```rust
if let Some(recall) = &memory_recall {
    // Recall domain-relevant facts for continuation context
    match recall.recall_with_graph(root_agent_id, continuation_message, 5).await {
        Ok(result) if !result.facts.is_empty() => {
            history.insert(0, ChatMessage::system(result.formatted));
            tracing::info!(fact_count = result.facts.len(), "Recalled facts for continuation");
        }
        _ => {}
    }
}
```

- [ ] **Step 3: Compile and test**

Run: `cargo check -p gateway-execution`
Run: `cargo test -p gateway-execution`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat(recall): wire recall_with_graph, add recall at continuation"
```

---

### Task 8: Ward-aware recall query enrichment

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Add optional ward context to recall methods**

Update `recall_with_graph` to accept optional ward name:

```rust
pub async fn recall_with_graph(
    &self,
    agent_id: &str,
    user_message: &str,
    limit: usize,
) -> Result<RecallResult, String>
```

Inside the function, the `user_message` is passed to the hybrid search as the query. If the intent analysis recommended a ward, the caller can prepend the ward/domain context to the message before calling recall. This doesn't require changing the recall API — just the caller in runner.rs.

In runner.rs, where recall is called (line ~433), prepend ward context if available from intent analysis:

```rust
let recall_query = if let Some(ward) = config.ward_hint.as_deref() {
    format!("{} {}", ward, message)
} else {
    message.clone()
};
match recall.recall_with_graph(&config.agent_id, &recall_query, 10).await {
```

Note: Check if `ExecutionConfig` has a `ward_hint` or similar field. If not, the intent analysis ward recommendation can be passed through config, or the ward name from the session can be used.

Alternatively, simpler approach — read the ward_id from the session state:
```rust
let ward_context = setup.ward_id.as_deref().unwrap_or("");
let recall_query = format!("{} {}", ward_context, message);
```

- [ ] **Step 2: Compile and test**

Run: `cargo check -p gateway-execution`

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat(recall): ward-aware recall query enrichment"
```

---

## Chunk 4: Documentation and Cleanup

### Task 9: Update documentation

**Files:**
- Modify: `memory-bank/architecture.md`
- Modify: `memory-bank/decisions.md`

- [ ] **Step 1: Remove .ward_memory.json references from architecture.md**

Search for `.ward_memory.json` and replace with AGENTS.md as ward memory.

- [ ] **Step 2: Update decisions.md**

Update the ward memory decision to reflect AGENTS.md + DB facts.

- [ ] **Step 3: Commit**

```bash
git add memory-bank/
git commit -m "docs: remove .ward_memory.json references, AGENTS.md is ward memory"
```

---

### Task 10: Final verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`

- [ ] **Step 2: Full test suite**

Run: `cargo test -p agent-tools -p knowledge-graph -p gateway-execution -p gateway-templates`

- [ ] **Step 3: Delete stale on-disk files**

```bash
rm -f ~/Documents/zbot/config/distillation_prompt.md
rm -f ~/Documents/zbot/config/shards/memory_learning.md
```

- [ ] **Step 4: Push**

```bash
git push origin autofill
```
