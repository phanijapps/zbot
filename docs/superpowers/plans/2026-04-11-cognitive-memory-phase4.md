# Cognitive Memory Phase 4 — Procedural Memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture successful multi-step action sequences as reusable procedures — agents learn HOW to do things, not just WHAT they know.

**Architecture:** A `ProcedureRepository` stores procedures with embeddings for semantic search. Distillation extracts procedures from successful sessions. Intent analysis recalls matching procedures and includes them as context for the LLM planner.

**Tech Stack:** Rust (gateway-execution, gateway-database), SQLite, LLM-based extraction via existing distillation patterns.

**Spec:** `docs/superpowers/specs/2026-04-11-cognitive-memory-system-design.md` — Section 8

**Branch:** `feature/sentient` (continuing from Phase 3)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| MODIFY | `gateway/gateway-database/src/schema.rs` | Migration v20: procedures table |
| CREATE | `gateway/gateway-database/src/procedure_repository.rs` | CRUD + embedding search for procedures |
| MODIFY | `gateway/gateway-database/src/lib.rs` | Export ProcedureRepository |
| MODIFY | `gateway/gateway-execution/src/distillation.rs` | Extract procedures from successful sessions |
| MODIFY | `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Recall procedures during intent analysis |
| MODIFY | `gateway/gateway-execution/src/recall.rs` | Add procedure search method |
| MODIFY | `gateway/src/state.rs` | Wire ProcedureRepository |

---

### Task 1: DB Migration v20 + ProcedureRepository

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`
- Create: `gateway/gateway-database/src/procedure_repository.rs`
- Modify: `gateway/gateway-database/src/lib.rs`

- [ ] **Step 1: Add migration v20**

In `schema.rs`, increment SCHEMA_VERSION to 20. Add migration block:

```sql
CREATE TABLE IF NOT EXISTS procedures (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    ward_id TEXT DEFAULT '__global__',
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    trigger_pattern TEXT,
    steps TEXT NOT NULL,
    parameters TEXT,
    success_count INTEGER DEFAULT 1,
    failure_count INTEGER DEFAULT 0,
    avg_duration_ms INTEGER,
    avg_token_cost INTEGER,
    last_used TEXT,
    embedding BLOB,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_procedures_agent ON procedures(agent_id);
CREATE INDEX IF NOT EXISTS idx_procedures_ward ON procedures(ward_id);
```

Also add to fresh database init section. Update schema version test.

- [ ] **Step 2: Create procedure_repository.rs**

Create `gateway/gateway-database/src/procedure_repository.rs` with:

- `Procedure` struct (all table fields, Debug, Clone)
- `ProcedureRepository` struct with `Arc<DatabaseManager>`
- Methods:
  - `upsert_procedure(procedure)` — INSERT ON CONFLICT update
  - `search_by_similarity(embedding, agent_id, ward_id, limit)` — vector cosine search
  - `get_procedure(id)` — single lookup
  - `increment_success(id, duration_ms, token_cost)` — bump success_count, update avgs
  - `increment_failure(id)` — bump failure_count
  - `list_procedures(agent_id, ward_id)` — list for agent/ward
- Follow exact pattern from `wiki_repository.rs` (constructor, `with_connection`, embedding serialization)
- Include `cosine_similarity()` helper (or import from wiki_repository if accessible)
- Tests: upsert + get, search by similarity, increment success, increment failure, list

- [ ] **Step 3: Export from lib.rs**

Add to `gateway/gateway-database/src/lib.rs`:
```rust
pub mod procedure_repository;
pub use procedure_repository::{Procedure, ProcedureRepository};
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-database -- procedure_repository`

- [ ] **Step 5: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-database -- -D warnings`

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-database/
git commit -m "feat(db): migration v20 + ProcedureRepository with CRUD, vector search, and tests"
```

---

### Task 2: Procedure Extraction in Distillation

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Read the distillation flow**

Read `distillation.rs` thoroughly. Understand:
- The `DistillationResponse` struct and how it's deserialized
- The `extract_all()` function or equivalent that calls the LLM
- Where facts are processed after extraction
- Where the wiki compilation was added (Phase 3)

- [ ] **Step 2: Add ExtractedProcedure to the distillation response**

Extend the `DistillationResponse` struct:

```rust
#[derive(Debug, Deserialize)]
pub struct ExtractedProcedure {
    pub name: String,
    pub description: String,
    pub steps: Vec<ProcedureStep>,
    pub parameters: Option<Vec<String>>,
    pub trigger_pattern: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProcedureStep {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
```

Add `procedure: Option<ExtractedProcedure>` to `DistillationResponse` (with `#[serde(default)]` so existing responses without it don't fail).

- [ ] **Step 3: Extend the distillation LLM prompt**

Find where the distillation prompt is built. Append a procedure extraction section:

```
## Procedure Extraction (optional)

If this session followed a reusable multi-step approach, extract it:
{
  "procedure": {
    "name": "short_snake_case_name",
    "description": "what this procedure does",
    "steps": [
      {"action": "ward|delegate|shell|respond", "agent": "agent-id", "task_template": "...", "note": "..."}
    ],
    "parameters": ["param1"],
    "trigger_pattern": "when to use this procedure"
  }
}

If the session was too simple or unique to be reusable, set "procedure": null.
```

- [ ] **Step 4: Add procedure upsert after extraction**

After facts are processed, add procedure processing:

```rust
// Extract and store procedure if present
if let Some(ref procedure) = response.procedure {
    if let Some(ref procedure_repo) = self.procedure_repo {
        let steps_json = serde_json::to_string(&procedure.steps).unwrap_or_default();
        let params_json = procedure.parameters.as_ref()
            .map(|p| serde_json::to_string(p).unwrap_or_default());
        
        let proc = gateway_database::Procedure {
            id: format!("proc-{}", uuid::Uuid::new_v4()),
            agent_id: agent_id.to_string(),
            ward_id: ward_id.clone().unwrap_or_else(|| "__global__".to_string()),
            name: procedure.name.clone(),
            description: procedure.description.clone(),
            trigger_pattern: procedure.trigger_pattern.clone(),
            steps: steps_json,
            parameters: params_json,
            success_count: 1,
            failure_count: 0,
            avg_duration_ms: None,
            avg_token_cost: None,
            last_used: Some(chrono::Utc::now().to_rfc3339()),
            embedding: None, // Will be set below
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        
        // Embed procedure description for similarity search
        if let Some(ref ec) = self.embedding_client {
            if let Ok(emb) = ec.embed(&procedure.description).await {
                // Check for similar existing procedure
                if let Ok(similar) = procedure_repo.search_by_similarity(&emb, agent_id, None, 1) {
                    if let Some((existing, score)) = similar.first() {
                        if *score > 0.85 {
                            // Merge: increment success count of existing
                            let _ = procedure_repo.increment_success(&existing.id, 0, 0);
                            tracing::info!(name = %procedure.name, "Merged with existing procedure");
                        } else {
                            let mut proc = proc;
                            proc.embedding = Some(emb);
                            let _ = procedure_repo.upsert_procedure(&proc);
                            tracing::info!(name = %procedure.name, "Stored new procedure");
                        }
                    } else {
                        let mut proc = proc;
                        proc.embedding = Some(emb);
                        let _ = procedure_repo.upsert_procedure(&proc);
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 5: Add procedure_repo to SessionDistiller**

Add `pub procedure_repo: Option<Arc<ProcedureRepository>>` to `SessionDistiller` struct. Add a setter method `set_procedure_repo()`.

- [ ] **Step 6: Verify compilation**

Run: `cargo check --workspace`

- [ ] **Step 7: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`

- [ ] **Step 8: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): extract procedures from successful sessions"
```

---

### Task 3: Wire ProcedureRepository + Procedure Recall in Intent Analysis

**Files:**
- Modify: `gateway/src/state.rs`
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

- [ ] **Step 1: Wire ProcedureRepository in state.rs**

In `gateway/src/state.rs`:
1. Add import: `use gateway_database::ProcedureRepository;`
2. Create repo: `let procedure_repo = Arc::new(ProcedureRepository::new(db_manager.clone()));`
3. Wire into distiller: `distiller_inner.set_procedure_repo(procedure_repo.clone());`
4. Wire into MemoryRecall (if needed for intent analysis recall)

- [ ] **Step 2: Add procedure search to MemoryRecall**

In `gateway/gateway-execution/src/recall.rs`, add:

```rust
pub procedure_repo: Option<Arc<ProcedureRepository>>,
```

Add setter `set_procedure_repo()`.

Add a new method:

```rust
/// Search for procedures matching a query by embedding similarity.
pub async fn recall_procedures(
    &self,
    query: &str,
    agent_id: &str,
    ward_id: Option<&str>,
    limit: usize,
) -> Result<Vec<(Procedure, f64)>, String> {
    let procedure_repo = match &self.procedure_repo {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    
    let embedding = self.embedding_client
        .embed(query)
        .await
        .map_err(|e| format!("Failed to embed query: {e}"))?;
    
    procedure_repo.search_by_similarity(&embedding, agent_id, ward_id, limit)
}
```

- [ ] **Step 3: Add procedure recall in intent analysis**

In `gateway/gateway-execution/src/middleware/intent_analysis.rs`, find where `recall.recall_for_intent()` is called (~line 347). After it, add procedure recall:

```rust
// Recall matching procedures
let mut procedure_context = String::new();
if let Some(recall) = memory_recall {
    if let Ok(procedures) = recall.recall_procedures(user_message, "root", None, 3).await {
        for (proc, score) in &procedures {
            if *score > 0.7 && proc.success_count >= 2 {
                let success_rate = proc.success_count as f64 
                    / (proc.success_count + proc.failure_count).max(1) as f64;
                procedure_context.push_str(&format!(
                    "\n## Proven Procedure: {}\n{}\nSteps: {}\nSuccess rate: {:.0}% ({} uses)\n",
                    proc.name,
                    proc.description,
                    proc.steps,
                    success_rate * 100.0,
                    proc.success_count,
                ));
                break; // Only include top match
            }
        }
    }
}
```

Then include `procedure_context` in the prompt (append to `memory_context` or create a combined context):

```rust
let full_context = if procedure_context.is_empty() {
    memory_context
} else {
    format!("{}\n{}", memory_context, procedure_context)
};
```

- [ ] **Step 4: Wire procedure_repo in state.rs for MemoryRecall**

Add `recall.set_procedure_repo(procedure_repo.clone());` in state.rs after the MemoryRecall is created.

- [ ] **Step 5: Verify compilation**

Run: `cargo check --workspace`

- [ ] **Step 6: Quality checks**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 7: Commit**

```bash
git add gateway/src/state.rs gateway/gateway-execution/src/recall.rs gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat(intent): recall proven procedures during intent analysis"
```

---

### Task 4: Final Checks

- [ ] **Step 1: Format and lint**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace --lib --bins --tests`

- [ ] **Step 3: Push**

```bash
git push
```
