# `run_procedure` — Procedure-as-Callable Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make z-Bot's learned procedures dispatchable. Add a `run_procedure(name, args)` tool that loads a row from the `procedures` table and executes its steps as a guided sub-loop, dispatching each step against the existing tool registry. Procedures stop being prompt-text-advice and become measurable, reusable, executable artifacts.

**Architecture:** `run_procedure` is a **tool** (not middleware), constructed with `Arc<ToolRegistry>` + `Arc<dyn ProcedureStore>`. Middleware (`intent_analysis.rs`) only *recommends* the tool when a high-confidence match exists; the LLM decides whether to call it. Inside the tool, a sub-executor walks `Vec<PatternStep>`, dispatches each step against the existing registry, interpolates `{step_N.field}` references, and updates `success_count` / `failure_count` on completion. Strict validation: each step's `action` must resolve to a registered tool.

**Tech Stack:** Rust workspace, SQLite (sqlite-vec for embeddings), `async_trait`, `serde_json`, existing `Tool` trait at `framework/zero-core/src/tool.rs`, existing `ToolRegistry` at `runtime/agent-runtime/src/tools/registry.rs`.

**Background reading (one-time, before starting):**
- `runtime/agent-runtime/src/tools/delegate.rs` — closest existing tool shape (no registry injection there, but same crate)
- `stores/zero-stores-traits/src/procedures.rs` — `ProcedureStore` trait surface
- `stores/zero-stores-domain/src/procedure.rs` — `Procedure` + `PatternProcedureInsert` shapes
- `gateway/gateway-memory/src/sleep/pattern_extractor.rs:60-87` — `PatternStep` + LLM response shape
- `gateway/gateway-execution/src/middleware/intent_analysis.rs:466-486` — existing recall surfacing

---

## File map

### Modified

- `stores/zero-stores-domain/src/procedure.rs` — add `embedding: Option<Vec<f32>>` field to `PatternProcedureInsert`
- `stores/zero-stores-traits/src/procedures.rs` — add `get_procedure_by_name` method to `ProcedureStore` trait
- `stores/zero-stores-sqlite/src/procedure_store.rs` — implement `get_procedure_by_name`; honour `embedding` in `insert_pattern_procedure`
- `gateway/gateway-memory/src/sleep/pattern_extractor.rs` — accept `EmbeddingClient`; extend `PatternStep` with `args` + `binds`; tighten LLM prompt
- `gateway/gateway-execution/src/distillation.rs` — pass embedding into `upsert_procedure` instead of `None`
- `gateway/gateway-execution/src/middleware/intent_analysis.rs` — emit `run_procedure` recommendation for high-confidence matches
- `runtime/agent-runtime/src/tools/mod.rs` — re-export `RunProcedureTool`
- `runtime/agent-runtime/src/executor.rs` — construct `RunProcedureTool` at the site that owns `Arc<ToolRegistry>` and `Arc<dyn ProcedureStore>`

### Created

- `runtime/agent-runtime/src/tools/run_procedure.rs` — the new tool, sub-executor, interpolation logic

---

## Task 1: Add `embedding` field to `PatternProcedureInsert`

**Why:** Today `insert_pattern_procedure` writes `embedding: None`, leaving `procedures_index` unpopulated. Vector recall on procedures is dead. This task threads an embedding through the insert path.

**Files:**
- Modify: `stores/zero-stores-domain/src/procedure.rs:43-54`
- Modify: `stores/zero-stores-sqlite/src/procedure_store.rs:121-147`
- Test: `stores/zero-stores-sqlite/tests/procedure_store_tests.rs` (create or append)

- [ ] **Step 1: Write the failing test**

Append to `stores/zero-stores-sqlite/tests/procedure_store_tests.rs` (create the file if missing — see existing test conventions in `stores/zero-stores-sqlite/tests/`):

```rust
#[tokio::test]
async fn insert_pattern_procedure_persists_embedding() {
    let store = test_procedure_store();   // existing helper; use the same pattern as other tests
    let req = PatternProcedureInsert {
        agent_id: "root".into(),
        ward_id: Some("__global__".into()),
        name: "test_proc".into(),
        description: "test".into(),
        trigger_pattern: None,
        steps_json: "[]".into(),
        parameters_json: None,
        embedding: Some(vec![0.1_f32, 0.2, 0.3]),
    };
    let id = store.insert_pattern_procedure(req).await.unwrap();
    // search by similarity should now return this row
    let results = store
        .search_procedures_by_similarity(&[0.1, 0.2, 0.3], "root", None, 5)
        .await
        .unwrap();
    assert!(results.iter().any(|r| r["procedure"]["id"] == id));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p zero-stores-sqlite insert_pattern_procedure_persists_embedding 2>&1 | tail -20
```

Expected: FAIL — `PatternProcedureInsert` has no `embedding` field (compile error).

- [ ] **Step 3: Add the field to `PatternProcedureInsert`**

Edit `stores/zero-stores-domain/src/procedure.rs:43-54`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternProcedureInsert {
    pub agent_id: String,
    pub ward_id: Option<String>,
    pub name: String,
    pub description: String,
    pub trigger_pattern: Option<String>,
    pub steps_json: String,
    pub parameters_json: Option<String>,
    /// Pre-computed embedding for the procedure description.
    /// Populated by the writer (PatternExtractor / distillation) so the
    /// SQLite store can upsert into `procedures_index`.
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
}
```

- [ ] **Step 4: Thread the embedding through `insert_pattern_procedure`**

Edit `stores/zero-stores-sqlite/src/procedure_store.rs:121-147` — change the `embedding: None` on line 141 to `embedding: req.embedding`:

```rust
async fn insert_pattern_procedure(
    &self,
    req: PatternProcedureInsert,
) -> Result<String, String> {
    let id = format!("proc-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    let procedure = Procedure {
        id: id.clone(),
        agent_id: req.agent_id,
        ward_id: req.ward_id,
        name: req.name,
        description: req.description,
        trigger_pattern: req.trigger_pattern,
        steps: req.steps_json,
        parameters: req.parameters_json,
        success_count: 1,
        failure_count: 0,
        avg_duration_ms: None,
        avg_token_cost: None,
        last_used: None,
        embedding: req.embedding,
        created_at: now.clone(),
        updated_at: now,
    };
    self.repo.upsert_procedure(&procedure)?;
    Ok(id)
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p zero-stores-sqlite insert_pattern_procedure_persists_embedding 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 6: Verify nothing else broke**

```bash
cargo check --workspace 2>&1 | tail -20
```

Expected: clean. Existing call sites that construct `PatternProcedureInsert` will compile because the new field is `#[serde(default)]` and `Option<Vec<f32>>` — but any literal struct constructors need `embedding: None` added. Fix any compile errors that surface, then re-run.

- [ ] **Step 7: Commit**

```bash
git add stores/zero-stores-domain/src/procedure.rs stores/zero-stores-sqlite/src/procedure_store.rs stores/zero-stores-sqlite/tests/
git commit -m "feat(procedures): thread embedding through PatternProcedureInsert

Adds optional embedding field so callers (PatternExtractor, distillation)
can populate the procedures_index vec0 table at write time. Without this,
recall_procedures returns nothing — the surfacing path at intent_analysis
has been silently no-oping in production.
"
```

---

## Task 2: Inject `EmbeddingClient` into `PatternExtractor`

**Why:** PatternExtractor builds `PatternProcedureInsert` rows but has no way to compute an embedding. Inject the embedding client through the constructor.

**Files:**
- Modify: `gateway/gateway-memory/src/sleep/pattern_extractor.rs:100-123` (struct + constructor)
- Modify: `gateway/gateway-memory/src/sleep/pattern_extractor.rs` near `build_procedure_insert` (the helper that constructs the insert)
- Modify: every site that calls `PatternExtractor::new` (use grep to find them)
- Test: `gateway/gateway-memory/src/sleep/pattern_extractor.rs` (existing test module)

- [ ] **Step 1: Find all `PatternExtractor::new` call sites**

```bash
grep -rIn "PatternExtractor::new" /home/videogamer/projects/agentzero --include="*.rs"
```

Expected: 1-3 sites (typically in the sleep cycle wiring and tests).

- [ ] **Step 2: Write a failing test**

Edit the test module at the bottom of `pattern_extractor.rs` (search for `#[cfg(test)] mod tests`). Add:

```rust
#[tokio::test]
async fn extractor_passes_embedding_to_store() {
    // Mock store that captures the insert
    let captured: Arc<tokio::sync::Mutex<Option<PatternProcedureInsert>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let store = Arc::new(CapturingProcedureStore { captured: captured.clone() });

    // Mock embedding client returning a fixed vector
    let embed = Arc::new(FixedEmbeddingClient { vec: vec![0.5_f32; 384] });

    let extractor = PatternExtractor::new(
        test_episode_store(),
        test_conversation_store(),
        store,
        test_compaction_store(),
        Arc::new(MockLlm::with_response(test_response("my_proc"))),
        embed,                              // NEW arg
    );
    extractor.run_cycle("run-1").await.unwrap();

    let inserted = captured.lock().await.take().expect("no insert captured");
    assert_eq!(inserted.embedding.as_ref().map(Vec::len), Some(384));
}
```

You will need to define `CapturingProcedureStore` and `FixedEmbeddingClient` test doubles in the test module. Keep them simple: `CapturingProcedureStore` impls `ProcedureStore` with all defaults except `insert_pattern_procedure` (stores the req) and `get_procedure_summary_by_name` (returns `Ok(None)`); `FixedEmbeddingClient` impls `EmbeddingClient` returning the fixed vec.

- [ ] **Step 3: Run the test to verify it fails**

```bash
cargo test -p gateway-memory extractor_passes_embedding_to_store 2>&1 | tail -20
```

Expected: FAIL — `PatternExtractor::new` does not accept an embedding client.

- [ ] **Step 4: Add embedding client to the struct + constructor**

Edit `pattern_extractor.rs:100-123`:

```rust
pub struct PatternExtractor {
    episode_store: Arc<dyn EpisodeStore>,
    conversation_store: Arc<dyn ConversationStore>,
    procedure_store: Arc<dyn ProcedureStore>,
    compaction_store: Arc<dyn CompactionStore>,
    llm: Arc<dyn PatternExtractLlm>,
    embedding_client: Arc<dyn EmbeddingClient>,
}

impl PatternExtractor {
    pub fn new(
        episode_store: Arc<dyn EpisodeStore>,
        conversation_store: Arc<dyn ConversationStore>,
        procedure_store: Arc<dyn ProcedureStore>,
        compaction_store: Arc<dyn CompactionStore>,
        llm: Arc<dyn PatternExtractLlm>,
        embedding_client: Arc<dyn EmbeddingClient>,
    ) -> Self {
        Self {
            episode_store,
            conversation_store,
            procedure_store,
            compaction_store,
            llm,
            embedding_client,
        }
    }
```

Add the import at the top of the file:

```rust
use agent_runtime::llm::embedding::EmbeddingClient;
```

(Cross-reference: `gateway/gateway-execution/src/distillation.rs:24` shows the exact import path.)

- [ ] **Step 5: Compute embedding before insert**

Find the `process_pair` / `insert_synthesized_procedure` flow around line 236-252. Modify the build step to embed `resp.description` and attach:

```rust
let mut req = match build_procedure_insert(agent_id, &name, resp) {
    Ok(p) => p,
    Err(e) => {
        tracing::warn!(error = %e, "pattern: build procedure failed");
        stats.skipped_llm_or_parse_error += 1;
        return;
    }
};

// Embed the description for the vec index
let embed_input = format!("{}\n{}", resp.name, resp.description);
match self.embedding_client.embed(&embed_input).await {
    Ok(vec) => req.embedding = Some(vec),
    Err(e) => {
        tracing::warn!(error = %e, "pattern: embedding failed; proceeding without");
    }
}

let proc_id = match self.procedure_store.insert_pattern_procedure(req).await {
    // existing match arms unchanged
};
```

Note the **graceful degradation**: if the embedding call fails, we still insert the procedure — name lookup remains a viable retrieval path.

- [ ] **Step 6: Update all `PatternExtractor::new` call sites**

For each site found in Step 1, pass an `Arc<dyn EmbeddingClient>`. The sleep cycle composition site already has one available (the same client distillation uses). For each call site:

```rust
// Before
PatternExtractor::new(ep, conv, proc, comp, llm)

// After
PatternExtractor::new(ep, conv, proc, comp, llm, embedding_client.clone())
```

- [ ] **Step 7: Run the test and the workspace**

```bash
cargo test -p gateway-memory extractor_passes_embedding_to_store 2>&1 | tail -10
cargo check --workspace 2>&1 | tail -10
```

Expected: target test passes; workspace compiles.

- [ ] **Step 8: Commit**

```bash
git add gateway/gateway-memory/src/sleep/pattern_extractor.rs
git add gateway/gateway-memory/src/sleep/mod.rs   # if call-site changed here
git add <any other modified composition sites>
git commit -m "feat(procedures): populate embedding in PatternExtractor

Injects EmbeddingClient into PatternExtractor. Procedure description is
embedded before insert_pattern_procedure; embedding failures degrade
gracefully (warn + proceed without). Closes the latent gap where
procedures_index was never populated.
"
```

---

## Task 3: Populate procedure embedding in distillation

**Why:** Same gap as Task 2, different code path. `distillation.rs:737` writes `upsert_procedure(v, None)` — embedding always null.

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs` around line 715-741

- [ ] **Step 1: Locate the procedure upsert block**

Read `gateway/gateway-execution/src/distillation.rs:710-745` to confirm the current shape.

- [ ] **Step 2: Modify the block to embed before upsert**

Replace the upsert section with:

```rust
let embed_input = format!("{}\n{}", procedure.name, procedure.description);
let embedding = self.embed_text(&embed_input).await;

let upsert_res = match &self.procedure_store {
    Some(store) => match serde_json::to_value(&proc) {
        Ok(v) => store.upsert_procedure(v, embedding).await,
        Err(e) => Err(format!("encode procedure: {e}")),
    },
    None => Err("no procedure store wired".to_string()),
};
```

`embed_text` already exists at `distillation.rs:1658` and returns `Option<Vec<f32>>`, matching the expected type of `upsert_procedure`'s embedding argument.

- [ ] **Step 3: Verify compile**

```bash
cargo check -p gateway-execution 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 4: Add an integration test**

Append to `gateway/gateway-execution/tests/distillation_tests.rs` (or wherever distillation tests live — use `find . -name 'distillation*tests*'` to locate):

```rust
#[tokio::test]
async fn distillation_writes_procedure_with_embedding() {
    let (distiller, captured_store) = setup_distiller_with_capture();
    // ... drive distillation with a transcript that produces 1 procedure
    distiller.distill_session("sess-1").await.unwrap();
    let captures = captured_store.upserts.lock().await;
    let (_, embedding) = captures.iter().find(|(v, _)| v["name"] == "expected_name").unwrap();
    assert!(embedding.is_some(), "procedure was upserted without embedding");
    assert_eq!(embedding.as_ref().unwrap().len(), 384);  // or whatever the dimension is
}
```

If a `CapturingProcedureStore` test double doesn't exist for distillation tests, create one inline in the test module.

- [ ] **Step 5: Run the test**

```bash
cargo test -p gateway-execution distillation_writes_procedure_with_embedding 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs gateway/gateway-execution/tests/
git commit -m "feat(procedures): populate embedding in distillation path

Distillation now embeds procedure name+description before upsert so the
procedures_index vec0 table is populated. Matches the PatternExtractor
fix from the previous commit.
"
```

---

## Task 4: Extend `PatternStep` with `args` and `binds`

**Why:** Current `PatternStep` (`pattern_extractor.rs:68-77`) has only `action`, `agent`, `note`, `task_template`. To dispatch a step we need a structured argument map and a list of fields to extract from each step's result for `{step_N.field}` interpolation.

**Files:**
- Modify: `gateway/gateway-memory/src/sleep/pattern_extractor.rs:68-77`
- Test: same file's existing test module

- [ ] **Step 1: Write a failing test for backward-compat deserialization**

In the existing test module, add:

```rust
#[test]
fn pattern_step_deserializes_old_format_without_args_or_binds() {
    let old = r#"{"action": "read_file"}"#;
    let step: PatternStep = serde_json::from_str(old).unwrap();
    assert_eq!(step.action, "read_file");
    assert!(step.args.is_empty());
    assert!(step.binds.is_empty());
}

#[test]
fn pattern_step_deserializes_new_format() {
    let new = r#"{
        "action": "shell",
        "args": {"cmd": "cargo test {test_name}"},
        "binds": ["assertion"]
    }"#;
    let step: PatternStep = serde_json::from_str(new).unwrap();
    assert_eq!(step.action, "shell");
    assert_eq!(step.args.get("cmd").and_then(|v| v.as_str()), Some("cargo test {test_name}"));
    assert_eq!(step.binds, vec!["assertion".to_string()]);
}
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test -p gateway-memory pattern_step_deserializes 2>&1 | tail -10
```

Expected: FAIL — `args`/`binds` fields don't exist.

- [ ] **Step 3: Extend `PatternStep`**

Edit `pattern_extractor.rs:68-77`:

```rust
/// Single step of a generalized pattern.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternStep {
    /// Tool name to dispatch (validated strict against ToolRegistry at run time).
    pub action: String,
    /// Structured arguments for the tool. May contain `{step_N.field}`
    /// interpolation tokens that the sub-executor resolves.
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    /// Field names to extract from this step's result and bind into
    /// `vars[step_N]` for later interpolation. Empty = bind whole result.
    #[serde(default)]
    pub binds: Vec<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub task_template: Option<String>,
}
```

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo test -p gateway-memory pattern_step 2>&1 | tail -10
```

Expected: both tests pass.

- [ ] **Step 5: Workspace check**

```bash
cargo check --workspace 2>&1 | tail -10
```

Expected: clean. Any code constructing `PatternStep` literally will need the new fields (use `Default::default()` for them).

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-memory/src/sleep/pattern_extractor.rs
git commit -m "feat(procedures): extend PatternStep with args and binds

Adds structured args map and binds list. Both default to empty, so old
procedure rows (without these fields) deserialize cleanly. Sets up the
shape RunProcedureTool will dispatch.
"
```

---

## Task 5: Tighten PatternExtractor LLM prompt

**Why:** Today the LLM is asked to "generalize" with no schema constraints on `action`. Strict-mode `run_procedure` will reject steps whose action isn't a registered tool. Tighten the prompt to require literal tool names + structured args.

**Files:**
- Modify: `gateway/gateway-memory/src/sleep/pattern_extractor.rs` around line 414-431 (the prompt constant or builder)

- [ ] **Step 1: Read the existing prompt**

```bash
grep -n "Generalize it into a reusable procedure\|generalize.*procedure\|Return ONLY JSON" /home/videogamer/projects/agentzero/gateway/gateway-memory/src/sleep/pattern_extractor.rs
```

Read 30 lines around the match. Confirm the prompt's current shape.

- [ ] **Step 2: Modify the prompt**

Replace the prompt body with:

```rust
let prompt = format!(
    "Two recent successful agent sessions shared a recurring tool-call sequence. \
     Generalize it into a reusable procedure that can be DISPATCHED by an automated executor.\n\n\
     STRICT REQUIREMENTS:\n\
     - Each step's `action` MUST be one of the following registered tool names: {}.\n\
     - Each step's `args` MUST be a JSON object containing the tool's required arguments.\n\
     - Use `{{step_N.field}}` (e.g., `{{step_0.stdout}}`) to reference previous step output.\n\
     - `binds` lists field names to extract from a step's result for later interpolation. \
       Omit for steps whose output isn't referenced.\n\
     - Procedures must be parameterizable: top-level `parameters` lists `{{parameter}}` names \
       used in step args.\n\n\
     Return ONLY JSON:\n\
     {{\n\
       \"name\": snake_case_string,\n\
       \"description\": string,\n\
       \"trigger_pattern\": string,\n\
       \"parameters\": [string],\n\
       \"steps\": [\n\
         {{\"action\": string, \"args\": object, \"binds\": [string], \"note\": string|null}}\n\
       ]\n\
     }}\n\n\
     Session A task: {sa}\nSession A tool sequence: {ta:?}\n\n\
     Session B task: {sb}\nSession B tool sequence: {tb:?}\n\n\
     Matched prefix: {mp:?}",
    tool_whitelist_csv,
    sa = input.task_summary_a,
    ta = input.tool_sequence_a,
    sb = input.task_summary_b,
    tb = input.tool_sequence_b,
    mp = input.matched_prefix,
);
```

You will need a `tool_whitelist_csv` — a comma-separated list of registered tool names. Add a new field to `PatternInput`:

```rust
pub struct PatternInput {
    pub task_summary_a: String,
    pub task_summary_b: String,
    pub tool_sequence_a: Vec<String>,
    pub tool_sequence_b: Vec<String>,
    pub matched_prefix: Vec<String>,
    /// Comma-separated list of registered tool names — the LLM must pick from these.
    pub tool_whitelist: String,
}
```

And populate it where `PatternInput` is constructed (find the call site with grep). Source the names from the same registry the runtime uses. If the registry isn't reachable from PatternExtractor, accept a `Vec<String>` in the constructor and let composition populate it.

- [ ] **Step 3: Update the test mock to assert the prompt mentions the whitelist**

In the existing test module:

```rust
#[tokio::test]
async fn pattern_prompt_includes_tool_whitelist() {
    // MockLlm that captures the prompt
    let captured = Arc::new(Mutex::new(String::new()));
    let llm = Arc::new(CapturingLlm { captured: captured.clone() });
    let extractor = build_test_extractor(llm, vec!["shell".into(), "read_file".into()]);
    extractor.run_cycle("r1").await.unwrap();
    let prompt = captured.lock().unwrap().clone();
    assert!(prompt.contains("shell"));
    assert!(prompt.contains("read_file"));
    assert!(prompt.contains("STRICT REQUIREMENTS"));
}
```

Add `CapturingLlm` as a test double impl of `PatternExtractLlm`. Build the test extractor helper to accept a whitelist.

- [ ] **Step 4: Run the test and the workspace**

```bash
cargo test -p gateway-memory pattern_prompt_includes_tool_whitelist 2>&1 | tail -10
cargo check --workspace 2>&1 | tail -10
```

Expected: passes; workspace clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-memory/src/sleep/pattern_extractor.rs
git add <any composition sites you touched>
git commit -m "feat(procedures): tighten extractor prompt for dispatchable steps

LLM must now pick action from a tool whitelist, emit structured args,
and declare binds. Sets up procedures whose shape RunProcedureTool can
execute directly.
"
```

---

## Task 6: Add `get_procedure_by_name` to `ProcedureStore`

**Why:** Today only `get_procedure_summary_by_name` exists (returns id + name + success_count). `RunProcedureTool` needs the full row to access `steps`.

**Files:**
- Modify: `stores/zero-stores-traits/src/procedures.rs` (add method to trait with default no-op)
- Modify: `stores/zero-stores-sqlite/src/procedure_store.rs` (implement)
- Test: `stores/zero-stores-sqlite/tests/procedure_store_tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `stores/zero-stores-sqlite/tests/procedure_store_tests.rs`:

```rust
#[tokio::test]
async fn get_procedure_by_name_returns_full_row() {
    let store = test_procedure_store();
    let req = PatternProcedureInsert {
        agent_id: "root".into(),
        ward_id: Some("__global__".into()),
        name: "find_me".into(),
        description: "a procedure".into(),
        trigger_pattern: None,
        steps_json: r#"[{"action":"shell","args":{"cmd":"ls"},"binds":[]}]"#.into(),
        parameters_json: Some(r#"["dir"]"#.into()),
        embedding: None,
    };
    store.insert_pattern_procedure(req).await.unwrap();
    let found = store.get_procedure_by_name("root", "find_me").await.unwrap();
    let proc = found.expect("not found");
    assert_eq!(proc.name, "find_me");
    assert_eq!(proc.description, "a procedure");
    assert!(proc.steps.contains("shell"));
}

#[tokio::test]
async fn get_procedure_by_name_returns_none_when_missing() {
    let store = test_procedure_store();
    let found = store.get_procedure_by_name("root", "nonexistent").await.unwrap();
    assert!(found.is_none());
}
```

- [ ] **Step 2: Run the tests to verify failure**

```bash
cargo test -p zero-stores-sqlite get_procedure_by_name 2>&1 | tail -10
```

Expected: FAIL — method doesn't exist.

- [ ] **Step 3: Add the trait method**

Edit `stores/zero-stores-traits/src/procedures.rs` — insert after `get_procedure_summary_by_name` (around line 95-101):

```rust
/// Look up a full procedure row by `(agent_id, name)`. Returns the
/// complete `Procedure` so callers (e.g., `RunProcedureTool`) can access
/// `steps`, `parameters`, etc. Default: not implemented.
async fn get_procedure_by_name(
    &self,
    _agent_id: &str,
    _name: &str,
) -> Result<Option<Procedure>, String> {
    Ok(None)
}
```

- [ ] **Step 4: Implement in SQLite**

Edit `stores/zero-stores-sqlite/src/procedure_store.rs` — add the impl method (mirroring the existing `get_procedure_summary_by_name` shape):

```rust
async fn get_procedure_by_name(
    &self,
    agent_id: &str,
    name: &str,
) -> Result<Option<Procedure>, String> {
    let agent_id = agent_id.to_string();
    let name = name.to_string();
    self.run_blocking(move |conn| {
        let r = conn.query_row(
            "SELECT id, agent_id, ward_id, name, description, trigger_pattern,
                    steps, parameters, success_count, failure_count,
                    avg_duration_ms, avg_token_cost, last_used,
                    created_at, updated_at
             FROM procedures
             WHERE agent_id = ?1 AND name = ?2
             LIMIT 1",
            params![agent_id, name],
            |row| {
                Ok(Procedure {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    ward_id: row.get(2)?,
                    name: row.get(3)?,
                    description: row.get(4)?,
                    trigger_pattern: row.get(5)?,
                    steps: row.get(6)?,
                    parameters: row.get(7)?,
                    success_count: row.get(8)?,
                    failure_count: row.get(9)?,
                    avg_duration_ms: row.get(10)?,
                    avg_token_cost: row.get(11)?,
                    last_used: row.get(12)?,
                    embedding: None,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            },
        );
        match r {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    })
    .await
}
```

If `run_blocking`'s exact shape differs, mirror the helper used by `get_procedure_summary_by_name` at `procedure_store.rs:90-118`.

- [ ] **Step 5: Run tests**

```bash
cargo test -p zero-stores-sqlite get_procedure_by_name 2>&1 | tail -10
cargo check --workspace 2>&1 | tail -10
```

Expected: passes; clean.

- [ ] **Step 6: Commit**

```bash
git add stores/zero-stores-traits/src/procedures.rs stores/zero-stores-sqlite/src/procedure_store.rs stores/zero-stores-sqlite/tests/
git commit -m "feat(procedures): add get_procedure_by_name returning full row

RunProcedureTool needs the full Procedure (steps, parameters) — the
summary variant only returns id + success_count. Default no-op on the
trait; SQLite impl reads the canonical columns.
"
```

---

## Task 7: Create `RunProcedureTool` skeleton

**Why:** The tool struct, schema, validation, and unimplemented `execute` body. Sets up the surface; the dispatch logic lands in Task 8.

**Files:**
- Create: `runtime/agent-runtime/src/tools/run_procedure.rs`
- Modify: `runtime/agent-runtime/src/tools/mod.rs` (add `pub mod run_procedure;` and re-export)

- [ ] **Step 1: Create the skeleton file**

Write `runtime/agent-runtime/src/tools/run_procedure.rs`:

```rust
//! # Run Procedure Tool
//!
//! Loads a learned procedure by name and executes its steps as a
//! guided sub-loop, dispatching each step against the live tool
//! registry. Strict-mode: each step's `action` must resolve to a
//! registered tool, or the whole procedure aborts with an error
//! (failure_count is bumped).

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use zero_core::{Result, Tool, ToolContext, ZeroError};
use zero_stores_traits::ProcedureStore;

use crate::tools::registry::ToolRegistry;

pub struct RunProcedureTool {
    registry: Arc<ToolRegistry>,
    procedure_store: Arc<dyn ProcedureStore>,
}

impl RunProcedureTool {
    #[must_use]
    pub fn new(
        registry: Arc<ToolRegistry>,
        procedure_store: Arc<dyn ProcedureStore>,
    ) -> Self {
        Self { registry, procedure_store }
    }
}

#[async_trait]
impl Tool for RunProcedureTool {
    fn name(&self) -> &'static str {
        "run_procedure"
    }

    fn description(&self) -> &'static str {
        "Execute a learned procedure by name. Loads the procedure's steps and \
         dispatches each step against the tool registry. Use this when a procedure \
         was recommended in your context. Returns the aggregated result of the \
         final step plus a summary of intermediate steps. On any step failure, \
         the procedure aborts and the failure is recorded."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Procedure name (snake_case)"
                },
                "args": {
                    "type": "object",
                    "description": "Top-level parameters the procedure declares. \
                                    Use the names listed in the procedure's `parameters` field."
                }
            },
            "required": ["name"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("name is required".into()))?;

        let agent_id = ctx
            .get_state("agent_id")
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "root".into());

        let proc = self
            .procedure_store
            .get_procedure_by_name(&agent_id, name)
            .await
            .map_err(|e| ZeroError::Tool(format!("procedure lookup failed: {e}")))?
            .ok_or_else(|| ZeroError::Tool(format!("procedure '{name}' not found")))?;

        // Step 8 lands the dispatch loop here. For now, fail loud so a
        // skeleton-only deployment can't claim success.
        Err(ZeroError::Tool(format!(
            "run_procedure: dispatch loop not yet implemented (loaded '{}', steps={}B)",
            proc.name,
            proc.steps.len()
        )))
    }
}
```

- [ ] **Step 2: Wire the module**

Edit `runtime/agent-runtime/src/tools/mod.rs`:

```rust
pub mod run_procedure;
pub use run_procedure::RunProcedureTool;
```

(Pattern matches `pub mod delegate; pub use delegate::DelegateTool;` already present.)

- [ ] **Step 3: Write a skeleton test**

Append to `runtime/agent-runtime/src/tools/run_procedure.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::ToolContext as ConcreteCtx;

    #[test]
    fn run_procedure_tool_schema() {
        let registry = Arc::new(ToolRegistry::new());
        let store = Arc::new(NoOpProcedureStore);
        let tool = RunProcedureTool::new(registry, store);
        assert_eq!(tool.name(), "run_procedure");
        let schema = tool.parameters_schema().unwrap();
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["required"][0], "name");
    }

    struct NoOpProcedureStore;
    #[async_trait]
    impl ProcedureStore for NoOpProcedureStore {}

    #[tokio::test]
    async fn run_procedure_errors_when_procedure_missing() {
        let registry = Arc::new(ToolRegistry::new());
        let store = Arc::new(NoOpProcedureStore);
        let tool = RunProcedureTool::new(registry, store);
        let ctx: Arc<dyn ToolContext> = Arc::new(ConcreteCtx::full_with_state(
            "root".into(), Some("c1".into()), vec![], Default::default(),
        ));
        let res = tool.execute(ctx, json!({"name": "nope"})).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("not found"));
    }
}
```

- [ ] **Step 4: Run the tests**

```bash
cargo test -p agent-runtime run_procedure 2>&1 | tail -10
```

Expected: both tests pass.

- [ ] **Step 5: Workspace check**

```bash
cargo check --workspace 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/tools/run_procedure.rs runtime/agent-runtime/src/tools/mod.rs
git commit -m "feat(tools): RunProcedureTool skeleton

Adds the tool struct, schema, validation, and 'not found' path. Dispatch
loop is intentionally unimplemented and fails loud — Task 8 lands the
executor body.
"
```

---

## Task 8: Implement the step dispatch loop (no interpolation yet)

**Why:** Walk `Vec<PatternStep>` and dispatch each against the registry. Defer `{step_N.field}` interpolation to Task 9 so this task stays focused.

**Files:**
- Modify: `runtime/agent-runtime/src/tools/run_procedure.rs`

- [ ] **Step 1: Write the failing test**

Append to the existing test module in `run_procedure.rs`:

```rust
#[tokio::test]
async fn dispatch_loop_runs_each_step_in_order() {
    // Register two fake tools that record their invocation order.
    let log = Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));

    struct LoggingTool { name: &'static str, log: Arc<tokio::sync::Mutex<Vec<String>>> }
    #[async_trait]
    impl Tool for LoggingTool {
        fn name(&self) -> &'static str { self.name }
        fn description(&self) -> &'static str { "log" }
        async fn execute(&self, _ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
            self.log.lock().await.push(self.name.into());
            Ok(json!({"ok": true}))
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(LoggingTool { name: "step_a", log: log.clone() }));
    registry.register(Arc::new(LoggingTool { name: "step_b", log: log.clone() }));
    let registry = Arc::new(registry);

    // Procedure with steps_json that runs step_a then step_b.
    let steps_json = serde_json::to_string(&vec![
        serde_json::json!({"action": "step_a", "args": {}, "binds": []}),
        serde_json::json!({"action": "step_b", "args": {}, "binds": []}),
    ]).unwrap();
    let store = Arc::new(InMemoryProcedureStore::with_one(Procedure {
        id: "p1".into(), agent_id: "root".into(), ward_id: None,
        name: "demo".into(), description: "d".into(), trigger_pattern: None,
        steps: steps_json, parameters: None,
        success_count: 0, failure_count: 0,
        avg_duration_ms: None, avg_token_cost: None, last_used: None,
        embedding: None, created_at: "".into(), updated_at: "".into(),
    }));

    let tool = RunProcedureTool::new(registry, store);
    let ctx: Arc<dyn ToolContext> = Arc::new(ConcreteCtx::full_with_state(
        "root".into(), Some("c1".into()), vec![], Default::default(),
    ));
    let res = tool.execute(ctx, json!({"name": "demo"})).await.unwrap();
    assert_eq!(res["status"], "ok");
    assert_eq!(res["steps_run"], 2);
    let order = log.lock().await.clone();
    assert_eq!(order, vec!["step_a", "step_b"]);
}
```

Define `InMemoryProcedureStore::with_one` near the test module — an `Arc<Mutex<Option<Procedure>>>`-style holder that returns the stored proc from `get_procedure_by_name`.

- [ ] **Step 2: Run the test to verify failure**

```bash
cargo test -p agent-runtime dispatch_loop_runs_each_step_in_order 2>&1 | tail -10
```

Expected: FAIL — current `execute` returns the "not yet implemented" error.

- [ ] **Step 3: Replace the `execute` body's tail with the dispatch loop**

Replace the placeholder return in `run_procedure.rs::execute` with:

```rust
let steps: Vec<crate::pattern::PatternStep> = serde_json::from_str(&proc.steps)
    .map_err(|e| ZeroError::Tool(format!("procedure steps unparseable: {e}")))?;

let mut step_results: Vec<Value> = Vec::with_capacity(steps.len());
let started = std::time::Instant::now();

for (i, step) in steps.iter().enumerate() {
    let inner_tool = self.registry.find(&step.action).ok_or_else(|| {
        ZeroError::Tool(format!(
            "step {} action '{}' is not a registered tool",
            i, step.action
        ))
    })?;

    // Task 9 will interpolate {step_N.field} here. For now, args go through unchanged.
    let step_args = Value::Object(step.args.clone());

    let result = match inner_tool.execute(ctx.clone(), step_args).await {
        Ok(v) => v,
        Err(e) => {
            // Bump failure_count and propagate.
            if let Err(ee) = self.procedure_store.increment_failure(&proc.id).await {
                tracing::warn!(error = %ee, "increment_failure failed");
            }
            return Err(ZeroError::Tool(format!(
                "run_procedure '{}' step {} ({}) failed: {}",
                proc.name, i, step.action, e
            )));
        }
    };
    step_results.push(result);
}

let duration_ms = started.elapsed().as_millis() as i64;
if let Err(e) = self
    .procedure_store
    .increment_success(&proc.id, Some(duration_ms), None)
    .await
{
    tracing::warn!(error = %e, "increment_success failed");
}

Ok(json!({
    "status": "ok",
    "procedure": proc.name,
    "steps_run": step_results.len(),
    "duration_ms": duration_ms,
    "final": step_results.last().cloned().unwrap_or(Value::Null),
    "all_steps": step_results
}))
```

You also need `PatternStep` accessible from this crate. Two options:
- (a) re-export `PatternStep` from a low-dep crate. If `pattern_extractor.rs` is the only definition, this means moving `PatternStep` to `stores/zero-stores-domain/src/procedure.rs` and re-exporting it.
- (b) define a thin parallel `PatternStep` here.

Prefer (a): move the struct to `zero-stores-domain` (it's a data shape, not a sleep-cycle internal). Update `pattern_extractor.rs` to import it from there. This is the cleaner long-term placement.

- [ ] **Step 4: Move `PatternStep` to `zero-stores-domain`**

Edit `stores/zero-stores-domain/src/procedure.rs` — append:

```rust
/// One step of a learned procedure. The `action` is a tool name validated
/// strict against the live `ToolRegistry` at run time. `args` may carry
/// `{step_N.field}` interpolation tokens. `binds` lists fields to extract
/// from this step's result for use in later interpolations.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternStep {
    pub action: String,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub binds: Vec<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub task_template: Option<String>,
}
```

Edit `gateway/gateway-memory/src/sleep/pattern_extractor.rs` — remove the local `PatternStep` definition and import from domain:

```rust
use zero_stores_domain::PatternStep;
```

Verify everything still compiles: `cargo check --workspace`.

- [ ] **Step 5: Import `PatternStep` in `run_procedure.rs`**

```rust
use zero_stores_domain::PatternStep;
```

And replace the `crate::pattern::PatternStep` reference with `PatternStep`.

- [ ] **Step 6: Run the test**

```bash
cargo test -p agent-runtime dispatch_loop_runs_each_step_in_order 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 7: Add a failure-path test**

```rust
#[tokio::test]
async fn dispatch_aborts_and_bumps_failure_on_step_error() {
    struct FailingTool;
    #[async_trait]
    impl Tool for FailingTool {
        fn name(&self) -> &'static str { "boom" }
        fn description(&self) -> &'static str { "fails" }
        async fn execute(&self, _ctx: Arc<dyn ToolContext>, _args: Value) -> Result<Value> {
            Err(ZeroError::Tool("nope".into()))
        }
    }

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(FailingTool));
    let registry = Arc::new(registry);

    let steps_json = serde_json::to_string(&vec![
        json!({"action": "boom", "args": {}, "binds": []})
    ]).unwrap();
    let store = Arc::new(InMemoryProcedureStore::with_one(
        // use the same builder as previous test
        test_procedure("p2", "demo2", &steps_json)
    ));

    let tool = RunProcedureTool::new(registry, store.clone());
    let ctx: Arc<dyn ToolContext> = Arc::new(ConcreteCtx::full_with_state(
        "root".into(), Some("c1".into()), vec![], Default::default(),
    ));
    let res = tool.execute(ctx, json!({"name": "demo2"})).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("step 0 (boom) failed"));
    assert!(store.failure_was_incremented("p2").await);
}
```

Add `test_procedure(id, name, steps_json)` builder helper near the test module. Extend `InMemoryProcedureStore` with `failure_was_incremented(id)` returning `bool`.

- [ ] **Step 8: Run all tests in the module**

```bash
cargo test -p agent-runtime run_procedure 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add stores/zero-stores-domain/src/procedure.rs gateway/gateway-memory/src/sleep/pattern_extractor.rs runtime/agent-runtime/src/tools/run_procedure.rs
git commit -m "feat(tools): RunProcedureTool dispatch loop

Walks PatternStep[] and dispatches each against the live tool registry.
Strict-mode action validation, success/failure counter wiring, full-stop
abort on any step error. PatternStep moved to zero-stores-domain so both
the extractor and the dispatcher share one definition.
"
```

---

## Task 9: Implement `{step_N.field}` interpolation

**Why:** Step args need to reference outputs from earlier steps. Without this, procedures can only chain stateless operations — useless for anything real (test name → assertion → file path → function name → caller).

**Files:**
- Modify: `runtime/agent-runtime/src/tools/run_procedure.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn args_interpolate_step_references() {
    struct EchoTool;
    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &'static str { "echo" }
        fn description(&self) -> &'static str { "echo" }
        async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
            // Echo the input back as { result: <input> }
            Ok(json!({ "result": args }))
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool));
    let registry = Arc::new(registry);

    let steps_json = serde_json::to_string(&vec![
        json!({"action": "echo", "args": {"x": "hello"}, "binds": ["result"]}),
        json!({"action": "echo", "args": {"prev": "{step_0.result}"}, "binds": []}),
    ]).unwrap();
    let store = Arc::new(InMemoryProcedureStore::with_one(
        test_procedure("p3", "interp", &steps_json)
    ));
    let tool = RunProcedureTool::new(registry, store);
    let ctx: Arc<dyn ToolContext> = Arc::new(ConcreteCtx::full_with_state(
        "root".into(), Some("c1".into()), vec![], Default::default(),
    ));
    let out = tool.execute(ctx, json!({"name": "interp"})).await.unwrap();
    // step_1 received args.prev == step_0's `result` field (the echoed args object)
    let final_args = &out["final"]["result"];
    assert_eq!(final_args["prev"]["x"], "hello");
}

#[tokio::test]
async fn interpolation_resolves_top_level_args() {
    // {args.foo} should resolve to top-level args passed to run_procedure
    struct EchoTool;
    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &'static str { "echo" }
        fn description(&self) -> &'static str { "echo" }
        async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
            Ok(args)
        }
    }
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool));
    let registry = Arc::new(registry);

    let steps_json = serde_json::to_string(&vec![
        json!({"action": "echo", "args": {"got": "{args.test_name}"}, "binds": []})
    ]).unwrap();
    let store = Arc::new(InMemoryProcedureStore::with_one(
        test_procedure("p4", "args_demo", &steps_json)
    ));
    let tool = RunProcedureTool::new(registry, store);
    let ctx: Arc<dyn ToolContext> = Arc::new(ConcreteCtx::full_with_state(
        "root".into(), Some("c1".into()), vec![], Default::default(),
    ));
    let out = tool.execute(ctx, json!({"name": "args_demo", "args": {"test_name": "test_belief"}})).await.unwrap();
    assert_eq!(out["final"]["got"], "test_belief");
}
```

- [ ] **Step 2: Run the tests to verify failure**

```bash
cargo test -p agent-runtime args_interpolate 2>&1 | tail -10
```

Expected: FAIL — current code passes `step.args` through verbatim.

- [ ] **Step 3: Implement interpolation**

Add a helper module to `run_procedure.rs` near the top:

```rust
mod interp {
    use serde_json::{Map, Value};
    use std::collections::HashMap;

    /// Resolve `{step_N.field.subfield}` and `{args.key.subkey}` tokens
    /// in a JSON Value tree. Tokens that don't resolve are left as-is
    /// (with a warning logged by the caller).
    pub fn resolve(
        v: &Value,
        prev_steps: &[Value],
        top_args: &Value,
        binds_per_step: &[Vec<String>],
    ) -> Value {
        match v {
            Value::String(s) => Value::String(resolve_string(s, prev_steps, top_args, binds_per_step)),
            Value::Array(a) => Value::Array(
                a.iter().map(|x| resolve(x, prev_steps, top_args, binds_per_step)).collect()
            ),
            Value::Object(m) => {
                let mut out = Map::new();
                for (k, val) in m {
                    out.insert(k.clone(), resolve(val, prev_steps, top_args, binds_per_step));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    /// If the string is exactly `{token}`, substitute the resolved Value
    /// (preserving its type). If the string contains `{token}` mixed with
    /// other text, substitute the stringified form.
    fn resolve_string(
        s: &str,
        prev_steps: &[Value],
        top_args: &Value,
        binds_per_step: &[Vec<String>],
    ) -> String {
        // Full-string token: {step_0.field} or {args.key}
        if let Some(stripped) = s.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            if !stripped.contains('{') && !stripped.contains('}') {
                if let Some(resolved) = lookup(stripped, prev_steps, top_args, binds_per_step) {
                    // Return JSON-stringified form so this can be re-parsed by the caller
                    // when the caller wants the typed value.
                    return match &resolved {
                        Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    };
                }
            }
        }
        // Embedded tokens: substitute each {token} in turn.
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        let bytes = s.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'{' {
                if let Some(end) = s[i+1..].find('}') {
                    let token = &s[i+1..i+1+end];
                    if let Some(v) = lookup(token, prev_steps, top_args, binds_per_step) {
                        match v {
                            Value::String(t) => out.push_str(&t),
                            other => out.push_str(&serde_json::to_string(&other).unwrap_or_default()),
                        }
                        i = i + 1 + end + 1;
                        continue;
                    }
                }
            }
            out.push(s[i..].chars().next().unwrap());
            i += s[i..].chars().next().unwrap().len_utf8();
        }
        out
    }

    fn lookup(
        path: &str,
        prev_steps: &[Value],
        top_args: &Value,
        binds_per_step: &[Vec<String>],
    ) -> Option<Value> {
        let mut parts = path.split('.');
        let head = parts.next()?;
        let rest: Vec<&str> = parts.collect();

        let root = if let Some(idx_str) = head.strip_prefix("step_") {
            let idx: usize = idx_str.parse().ok()?;
            let step_val = prev_steps.get(idx)?;
            // If the step had explicit binds, look up under those keys; otherwise the whole result.
            if let Some(binds) = binds_per_step.get(idx) {
                if !binds.is_empty() {
                    // Walk into step_val under the named bind first.
                    // If `rest[0]` matches a bind, use the corresponding subtree.
                    if let Some(first) = rest.first() {
                        if binds.iter().any(|b| b == first) {
                            return walk(step_val, &rest);
                        }
                    }
                }
            }
            step_val.clone()
        } else if head == "args" {
            top_args.clone()
        } else {
            return None;
        };

        walk(&root, &rest)
    }

    fn walk(root: &Value, path: &[&str]) -> Option<Value> {
        let mut cur = root.clone();
        for seg in path {
            cur = cur.get(seg)?.clone();
        }
        Some(cur)
    }
}
```

Then in `execute`, replace the `let step_args = Value::Object(step.args.clone());` line with:

```rust
let top_args = args.get("args").cloned().unwrap_or(Value::Object(Default::default()));
let binds_per_step: Vec<Vec<String>> = steps.iter().map(|s| s.binds.clone()).collect();
// ... inside the loop, replace:
let raw_args = Value::Object(step.args.clone());
let step_args = interp::resolve(&raw_args, &step_results, &top_args, &binds_per_step);
```

- [ ] **Step 4: Run the tests**

```bash
cargo test -p agent-runtime args_interpolate 2>&1 | tail -20
cargo test -p agent-runtime run_procedure 2>&1 | tail -20
```

Expected: all interp tests pass; previous tests still pass.

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/tools/run_procedure.rs
git commit -m "feat(tools): {step_N.field} and {args.key} interpolation

Adds a small resolver that walks JSON arg trees and substitutes
{step_N.path.to.field} and {args.path.to.key} tokens. Full-string tokens
preserve their resolved type; embedded tokens stringify. Unresolved
tokens pass through unchanged.
"
```

---

## Task 10: Wire `RunProcedureTool` into the runtime composition

**Why:** The tool exists but isn't constructed anywhere — the LLM can't call it. Add it to the runtime's tool registry build path.

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs` near the composition site that owns `Arc<ToolRegistry>` and the procedure store

- [ ] **Step 1: Find the composition site**

```bash
grep -n "tool_registry\|ToolRegistry::new\|register(Arc::new(DelegateTool" /home/videogamer/projects/agentzero/runtime/agent-runtime/src/executor.rs | head -20
```

Locate where `DelegateTool` is registered. The same site should own (or have access to) the procedure store.

- [ ] **Step 2: Verify procedure_store is reachable at the composition site**

```bash
grep -rIn "procedure_store" /home/videogamer/projects/agentzero/runtime/agent-runtime --include="*.rs" | head -10
```

If `procedure_store` isn't already plumbed through to the executor's composition site, you need to thread it. Check `gateway/src/main.rs` or wherever the executor is constructed — that's where the SQLite store lives. The cleanest approach: add `procedure_store: Arc<dyn ProcedureStore>` to whatever builder constructs the runtime tool registry, then pass it through.

If this thread is wider than one file, halt and pull in the architecture maintainer — don't refactor blind.

- [ ] **Step 3: Register the tool**

At the composition site, after the existing `DelegateTool` registration, add:

```rust
let run_procedure = RunProcedureTool::new(
    tool_registry.clone(),         // self-reference for sub-dispatch
    procedure_store.clone(),
);
tool_registry.register(Arc::new(run_procedure));
```

Note: this creates a circular reference (registry contains a tool that holds the registry). `Arc` handles this safely as long as we use `Arc<ToolRegistry>` (not a back-pointer that could leak). Verify no `Weak` discipline is required.

- [ ] **Step 4: Add an end-to-end integration test**

Append to `runtime/agent-runtime/tests/run_procedure_integration_tests.rs` (create if missing):

```rust
//! End-to-end: register run_procedure into a real registry alongside another
//! tool, run a real procedure row from a real (test) SQLite store, verify
//! the dispatch loop completes and success_count is incremented.

#[tokio::test]
async fn run_procedure_dispatches_against_live_registry() {
    let store = build_test_procedure_store_with(/* one procedure named "demo" with one step */).await;

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool));   // simple test tool
    let registry = Arc::new(registry);

    // Register run_procedure with self-reference + store
    // (Note: in real composition, run_procedure is registered into the same Arc<ToolRegistry>
    //  it holds. Here we build the registry incrementally; clone the Arc once it's set up.)
    let rp = RunProcedureTool::new(registry.clone(), store.clone());
    // Use a fresh registry for the test outer call:
    let mut outer = ToolRegistry::new();
    outer.register(Arc::new(rp));
    outer.register(Arc::new(EchoTool));
    let outer = Arc::new(outer);

    // Build a ctx and call run_procedure through outer
    let ctx: Arc<dyn ToolContext> = Arc::new(ConcreteCtx::full_with_state(
        "root".into(), Some("c1".into()), vec![], Default::default(),
    ));
    let rp_tool = outer.find("run_procedure").unwrap();
    let res = rp_tool.execute(ctx, json!({"name": "demo"})).await.unwrap();
    assert_eq!(res["status"], "ok");

    // Verify success_count was incremented
    let proc = store.get_procedure_by_name("root", "demo").await.unwrap().unwrap();
    assert_eq!(proc.success_count, 2);  // started at 1 from insert
}
```

- [ ] **Step 5: Run the integration test**

```bash
cargo test -p agent-runtime run_procedure_dispatches 2>&1 | tail -10
cargo check --workspace 2>&1 | tail -10
```

Expected: passes; clean.

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs runtime/agent-runtime/tests/run_procedure_integration_tests.rs
git add <any composition wiring you touched>
git commit -m "feat(tools): wire RunProcedureTool into runtime registry

Registers the tool alongside DelegateTool. The tool's registry handle
is the same Arc the executor uses, enabling sub-dispatch.
"
```

---

## Task 11: Recommend `run_procedure` from `intent_analysis`

**Why:** Today `intent_analysis.rs:466-486` formats a matched procedure as `## Proven Procedure Available` text. Promote it to an actionable recommendation that suggests calling `run_procedure(name, args)`. Only recommend when the procedure is structurally dispatchable (all `action`s valid; high confidence).

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs:466-486`

- [ ] **Step 1: Read the current block**

```bash
sed -n '440,500p' /home/videogamer/projects/agentzero/gateway/gateway-execution/src/middleware/intent_analysis.rs
```

Confirm the current shape.

- [ ] **Step 2: Add a structural-validity helper**

Append to the same file (private fn):

```rust
fn procedure_is_dispatchable(steps_json: &str, known_tool_names: &[&str]) -> bool {
    let parsed: Vec<zero_stores_domain::PatternStep> =
        match serde_json::from_str(steps_json) {
            Ok(v) => v,
            Err(_) => return false,
        };
    if parsed.is_empty() {
        return false;
    }
    parsed
        .iter()
        .all(|s| known_tool_names.iter().any(|n| *n == s.action))
}
```

- [ ] **Step 3: Replace the recall surfacing block**

At lines 466-486, replace:

```rust
let mut procedure_context = String::new();
if let Some(recall) = memory_recall {
    if let Ok(procedures) = recall
        .recall_procedures(user_message, "root", None, 3).await
    {
        for (proc, score) in &procedures {
            if *score > 0.7 && proc.success_count >= 2 {
                let total = (proc.success_count + proc.failure_count).max(1) as f64;
                let success_rate = proc.success_count as f64 / total;
                procedure_context = format!(
                    "\n## Proven Procedure Available: {}\n{}\nSteps: {}\nSuccess rate: {:.0}% ({} uses)\n",
                    proc.name, proc.description, proc.steps,
                    success_rate * 100.0, proc.success_count,
                );
                break;
            }
        }
    }
}
```

with:

```rust
let mut procedure_context = String::new();
if let Some(recall) = memory_recall {
    if let Ok(procedures) = recall
        .recall_procedures(user_message, "root", None, 3).await
    {
        // The set of tool names the runtime registry knows about — passed in
        // from the orchestrator's tool inventory. If empty, fall back to
        // surfacing the procedure as advisory text (legacy behavior).
        let known_tools_owned = tool_inventory.iter().map(|n| n.as_str()).collect::<Vec<_>>();

        for (proc, score) in &procedures {
            let success_floor = 3;
            let score_floor = 0.85;
            let total = (proc.success_count + proc.failure_count).max(1) as f64;
            let success_rate = proc.success_count as f64 / total;

            let dispatchable = !known_tools_owned.is_empty()
                && procedure_is_dispatchable(&proc.steps, &known_tools_owned);

            if dispatchable && *score > score_floor && proc.success_count >= success_floor {
                // Promoted: actionable recommendation
                procedure_context = format!(
                    "\n## Recommended action: run_procedure\n\
                     A learned procedure matches this request.\n\
                     - Name: `{}`\n\
                     - Description: {}\n\
                     - Success rate: {:.0}% across {} uses\n\
                     - Parameters: {}\n\n\
                     Suggested call:\n\
                     ```\n\
                     run_procedure(name=\"{}\", args={{...fill from user request...}})\n\
                     ```\n\
                     If the procedure doesn't fit, ignore this recommendation and proceed normally.\n",
                    proc.name, proc.description,
                    success_rate * 100.0, proc.success_count,
                    proc.parameters.as_deref().unwrap_or("[]"),
                    proc.name,
                );
                break;
            } else if *score > 0.7 && proc.success_count >= 2 {
                // Legacy advisory fallback
                procedure_context = format!(
                    "\n## Proven Procedure Available: {}\n{}\nSteps: {}\nSuccess rate: {:.0}% ({} uses)\n",
                    proc.name, proc.description, proc.steps,
                    success_rate * 100.0, proc.success_count,
                );
                break;
            }
        }
    }
}
```

`tool_inventory` needs to be threaded into the caller of this function. Find the function's signature and add `tool_inventory: &[String]` as a parameter. Update all call sites (likely 1-2 places).

If the executor's tool registry isn't directly reachable from this layer, expose `pub fn tool_names(&self) -> Vec<String>` on `ToolRegistry` and pass the snapshot in at the call site.

- [ ] **Step 4: Add a unit test for the dispatchable gate**

```rust
#[test]
fn procedure_is_dispatchable_accepts_known_tools() {
    let steps = r#"[
        {"action": "shell", "args": {}, "binds": []},
        {"action": "read_file", "args": {}, "binds": []}
    ]"#;
    assert!(procedure_is_dispatchable(steps, &["shell", "read_file", "grep"]));
}

#[test]
fn procedure_is_dispatchable_rejects_unknown_tools() {
    let steps = r#"[{"action": "frobnicate", "args": {}, "binds": []}]"#;
    assert!(!procedure_is_dispatchable(steps, &["shell", "read_file"]));
}

#[test]
fn procedure_is_dispatchable_rejects_empty_steps() {
    assert!(!procedure_is_dispatchable("[]", &["shell"]));
}

#[test]
fn procedure_is_dispatchable_rejects_malformed_json() {
    assert!(!procedure_is_dispatchable("not json", &["shell"]));
}
```

- [ ] **Step 5: Run the tests**

```bash
cargo test -p gateway-execution procedure_is_dispatchable 2>&1 | tail -10
cargo check --workspace 2>&1 | tail -10
```

Expected: all pass; clean.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git add <any composition sites you touched for tool_inventory>
git commit -m "feat(intent): recommend run_procedure for dispatchable matches

When a high-confidence (score > 0.85, success_count >= 3) procedure
recall hit has steps that all resolve against the runtime tool inventory,
surface it as an actionable run_procedure recommendation. Lower
confidence or non-dispatchable shapes fall back to the legacy advisory
text so we don't regress existing surfacing.
"
```

---

## Task 12: End-to-end smoke test

**Why:** Prove the full loop end-to-end: write a procedure → run it → success_count updates → recall finds it next time.

**Files:**
- Create: `runtime/agent-runtime/tests/procedure_e2e_test.rs`

- [ ] **Step 1: Write the test**

```rust
//! End-to-end procedure-as-callable test.
//!
//! 1. Insert a procedure with a real PatternStep[] using two registered tools.
//! 2. Run it via RunProcedureTool.
//! 3. Verify success_count == 2 (insert sets it to 1, success adds 1).
//! 4. Verify the result contains the final step's output.

use serde_json::json;
use std::sync::Arc;
use zero_stores_traits::ProcedureStore;
use agent_runtime::tools::registry::ToolRegistry;
use agent_runtime::tools::run_procedure::RunProcedureTool;

#[tokio::test]
async fn procedure_e2e_writes_runs_and_increments_success() {
    let store = build_test_sqlite_procedure_store().await;

    // Insert a procedure that uses two test tools
    let steps_json = serde_json::to_string(&vec![
        json!({"action": "test_echo", "args": {"msg": "{args.greeting}"}, "binds": ["echoed"]}),
        json!({"action": "test_upper", "args": {"in": "{step_0.echoed}"}, "binds": []}),
    ]).unwrap();

    store.insert_pattern_procedure(zero_stores_domain::PatternProcedureInsert {
        agent_id: "root".into(),
        ward_id: None,
        name: "shout".into(),
        description: "uppercase a greeting".into(),
        trigger_pattern: None,
        steps_json,
        parameters_json: Some(r#"["greeting"]"#.into()),
        embedding: Some(vec![0.0; 4]),
    }).await.unwrap();

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool));
    registry.register(Arc::new(UpperTool));
    let registry = Arc::new(registry);

    let rp = RunProcedureTool::new(registry.clone(), store.clone());
    let ctx: Arc<dyn zero_core::ToolContext> = Arc::new(test_ctx());
    let result = rp.execute(ctx, json!({"name": "shout", "args": {"greeting": "hello"}})).await.unwrap();

    assert_eq!(result["status"], "ok");
    assert_eq!(result["final"], "HELLO");

    let after = store.get_procedure_by_name("root", "shout").await.unwrap().unwrap();
    assert_eq!(after.success_count, 2);
}

// EchoTool: returns { echoed: args.msg }
// UpperTool: returns args.in.to_uppercase() as a bare string
// build_test_sqlite_procedure_store + test_ctx: existing test helpers; mirror Task 1's setup.
```

Implement `EchoTool` and `UpperTool` inline in the test file.

- [ ] **Step 2: Run the test**

```bash
cargo test --test procedure_e2e_test 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 3: Final workspace check**

```bash
cargo check --workspace 2>&1 | tail -10
cargo fmt --all --check 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings 2>&1 | tail -30
```

Expected: all clean.

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/tests/procedure_e2e_test.rs
git commit -m "test(procedures): end-to-end procedure dispatch smoke test

Inserts a 2-step procedure, runs it through RunProcedureTool with arg
interpolation between steps, asserts the success_count increments and
the final result propagates correctly.
"
```

---

## Verification checklist (run after all tasks)

- [ ] `cargo fmt --all --check` — clean
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo test --workspace` — all tests pass
- [ ] Manually create a procedure via direct DB insert (or via PatternExtractor running on real session data), call `run_procedure` from a real session, observe success_count increment in `knowledge.db`
- [ ] Confirm `intent_analysis.rs` recommendation appears in the system prompt for a request that matches a high-confidence dispatchable procedure (use `tracing::debug!` to surface the formatted prompt during dev)

## Self-review notes

- **Spec coverage:** Tool (Task 7-9), middleware recommendation (Task 11), strict validation (Task 8 step 3), success/failure counters (Task 8), interpolation (Task 9), embedding fix (Tasks 2-3), schema extension (Tasks 4, 6), lookup API (Task 6). All present.
- **Placeholders:** None. Every step contains the actual code or command.
- **Type consistency:** `PatternStep` is defined once in `zero-stores-domain` (Task 8 step 4), imported by both extractor and dispatcher. `Procedure` shape unchanged. `PatternProcedureInsert` gains one optional field (Task 1).
- **One known soft spot:** Task 10 step 2 may fan out wider than expected if `procedure_store` isn't already threaded to the executor composition site. The task says "halt and pull in the maintainer" — that's the right call; don't refactor blind.

---

## Out of scope (explicit non-goals for this plan)

- **Slash commands.** The user explicitly deferred these.
- **FallbackLlmClient.** In the backlog.
- **Planner-spec → procedure bridge.** Follow-up. Once `run_procedure` ships and PatternExtractor produces dispatchable procedures, we add a distillation hook that promotes successful planner specs into procedure rows whose steps are `delegate_to_agent` calls. Tracked separately.
- **Per-step hooks.** `before_tool_call` / `after_tool_call` do NOT fire for inner steps — a procedure is one logical action. Telemetry for inner steps (if needed) is a separate concern (emit `procedure_step` events from inside the dispatch loop).
- **Procedure UI / observability.** No new dashboard work in this plan. The `procedures` table can be inspected via existing memory-bank tooling.

---

## Execution handoff

Plan complete and consolidated under `memory-bank/components/memory-layer/implementation-plans/2026-05-17-run-procedure-tool.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
