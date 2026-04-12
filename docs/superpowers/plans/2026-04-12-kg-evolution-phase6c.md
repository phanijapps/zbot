# Knowledge Graph Evolution — Phase 6c Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Make recall scoring class-aware — archival facts (historical records) never decay with age, current state decays when superseded, conventions and procedural facts use confidence-based scoring only.

**Architecture:** Three changes working together:
1. `MemoryFact` struct gains `epistemic_class`, `source_episode_id`, `source_ref` fields (DB columns already exist from 6a)
2. Distillation prompt asks LLM to classify each fact; `ExtractedFact` captures it; conversion propagates it
3. `recall.rs` scoring branches on class when applying supersession penalty

**Spec:** `docs/superpowers/specs/2026-04-12-knowledge-graph-evolution-design.md` — Section 3

**Branch:** `feature/sentient` (continues from Phase 6b)

---

## Task 1: MemoryFact Struct + Row Mapping + SELECT Queries

**Files:** `gateway/gateway-database/src/memory_repository.rs`

- [ ] **Step 1: Add 3 fields to MemoryFact struct**

After the existing `pinned: bool,` field:

```rust
/// Epistemic classification governing lifecycle behavior:
/// - `archival` — historical records, never decay
/// - `current` — volatile observed state, decays when superseded
/// - `convention` — rules/preferences, stable until explicitly replaced
/// - `procedural` — learned patterns, evolve via success counts
/// Defaults to "current" when not specified.
#[serde(default)]
pub epistemic_class: Option<String>,

/// FK to kg_episodes.id — the extraction event that produced this fact
#[serde(default)]
pub source_episode_id: Option<String>,

/// Human-readable pointer to source (e.g., "hindu_mahasabha.pdf:page_42")
#[serde(default)]
pub source_ref: Option<String>,
```

- [ ] **Step 2: Update `row_to_memory_fact`**

Add after the `pinned` mapping (the existing function uses numeric column indices):

```rust
epistemic_class: row.get(20).ok().flatten(),
source_episode_id: row.get(21).ok().flatten(),
source_ref: row.get(22).ok().flatten(),
```

- [ ] **Step 3: Update ALL SELECT queries**

Find every SELECT on `memory_facts` (there are several — `get_fact_by_key`, `list_memory_facts`, `search_memory_facts_*`, `get_corrections_for_agent`, etc.). Append to the column list:

```sql
, epistemic_class, source_episode_id, source_ref
```

Ensure the column order matches `row_to_memory_fact`'s index expectations (20, 21, 22).

- [ ] **Step 4: Update INSERT statement**

Find `upsert_memory_fact` (or wherever INSERT INTO memory_facts happens). Add the 3 columns + bind params.

- [ ] **Step 5: Update test helpers**

Any `MemoryFact { ... }` literal in tests will fail to compile because of the new fields. Add `epistemic_class: None, source_episode_id: None, source_ref: None` (or use `..Default::default()` if that's available).

- [ ] **Step 6: Verify compilation**

Run: `cargo check --package gateway-database`
Expected: Clean.

Run: `cargo test --package gateway-database`
Expected: All pass.

Run: `cargo fmt --all && cargo clippy --package gateway-database -- -D warnings`
Expected: Clean.

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-database/src/memory_repository.rs
git commit -m "feat(db): add epistemic_class, source_episode_id, source_ref to MemoryFact struct"
```

---

## Task 2: Propagate epistemic_class Through Distillation

**Files:** `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add field to ExtractedFact struct**

```rust
#[derive(Debug, Clone, Deserialize)]
struct ExtractedFact {
    category: String,
    key: String,
    content: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
    /// Optional epistemic classification (archival|current|convention|procedural).
    /// Defaults to "current" when omitted.
    #[serde(default)]
    epistemic_class: Option<String>,
}
```

- [ ] **Step 2: Update distillation prompt**

In `DEFAULT_DISTILLATION_PROMPT`, in the fact schema shown in the response template, update:

```
{"category": "...", "key": "...", "content": "...", "confidence": 0.0-1.0,
 "epistemic_class": "archival|current|convention|procedural"}
```

Add a new section in the prompt after "## Fact Categories":

```
## Epistemic Classification (REQUIRED per fact)

Every fact has a lifecycle class that determines how it ages:

- `archival` — Historical record of what happened or was stated in a primary source.
  NEVER DECAYS. Examples: birthdates, historical events, quotes from documents.
  Choose this when the fact describes something that happened in the past and
  won't change (only be corrected if it was wrong).

- `current` — Observed state at a point in time that can change.
  DECAYS when superseded. Examples: stock prices, API states, "current X".

- `convention` — Standing rules, preferences, standing orders.
  STABLE, replaced only on explicit policy change. Examples: user preferences,
  coding standards.

- `procedural` — Reusable action sequences reinforced by outcomes.
  EVOLVES via success/failure counts.

Default when unsure: `archival` if the fact comes from a document/book/URL,
otherwise `current`.
```

- [ ] **Step 3: Update ExtractedFact → MemoryFact conversion**

Find where `MemoryFact { ... }` is constructed from `ExtractedFact`. Add:

```rust
epistemic_class: ef.epistemic_class
    .clone()
    .or_else(|| Some("current".to_string())),
source_episode_id: None,  // Populated by ward_artifact_indexer; distillation uses LLM so no episode
source_ref: None,
```

Note: for facts produced by distillation (LLM), `source_episode_id` stays None — the session itself is the provenance via `session_id`. For facts produced by the ward artifact indexer, that code already populates properties with `_source_episode_id` / `_source_ref` on entities; facts don't go through there.

- [ ] **Step 4: Update the few-shot example in the prompt**

In the Savarkar example, add `"epistemic_class": "archival"` to the example fact:

```json
{"category": "domain", "key": "hindu_mahasabha.savarkar.presidency",
 "content": "...",
 "confidence": 0.95,
 "epistemic_class": "archival"}
```

- [ ] **Step 5: Verify + commit**

Run: `cargo check --package gateway-execution && cargo test --package gateway-execution`
Expected: All pass.

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat(distillation): extract epistemic_class per fact + propagate to MemoryFact"
```

---

## Task 3: Class-Aware Recall Scoring

**Files:** `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Locate the supersession penalty**

Find the scoring block that does `sf.score *= 0.3` when `valid_until.is_some()`. Typically near the end of score aggregation.

- [ ] **Step 2: Replace with class-aware branching**

```rust
// Step 9: Class-aware scoring for superseded or age-sensitive facts.
//
// Archival facts (historical records from primary sources) NEVER DECAY with age.
// Only corrected archival facts get a mild penalty — and they remain retrievable
// for provenance queries ("what did we previously believe?").
//
// Current state facts decay sharply when superseded.
// Convention and procedural facts are confidence-based; no temporal penalty.
for sf in &mut results {
    let class = sf.fact.epistemic_class.as_deref().unwrap_or("current");
    match class {
        "archival" => {
            // Only apply a mild penalty when a corrected replacement exists.
            // An archival fact from 1937 retrieved in 2026 is as relevant as ever.
            if sf.fact.valid_until.is_some() {
                sf.score *= 0.3;
            }
        }
        "current" => {
            // Strong decay for superseded volatile state.
            if sf.fact.valid_until.is_some() {
                sf.score *= 0.1;
            }
        }
        "convention" | "procedural" => {
            // Confidence-based; no temporal decay applies here.
        }
        _ => {
            // Unknown class — treat as current (conservative).
            if sf.fact.valid_until.is_some() {
                sf.score *= 0.3;
            }
        }
    }
}
```

- [ ] **Step 3: Add unit tests for class-aware scoring**

In the existing test module (or create one), add tests that build `MemoryFact`s with different classes, run scoring, and assert the expected penalties:

```rust
#[test]
fn archival_superseded_gets_mild_penalty() {
    let mut sf = make_scored_fact("archival", Some("2026-01-01"), 1.0);
    apply_class_aware_penalty(&mut sf);
    assert!((sf.score - 0.3).abs() < 1e-6);
}

#[test]
fn current_superseded_gets_strong_penalty() {
    let mut sf = make_scored_fact("current", Some("2026-01-01"), 1.0);
    apply_class_aware_penalty(&mut sf);
    assert!((sf.score - 0.1).abs() < 1e-6);
}

#[test]
fn archival_not_superseded_no_penalty() {
    let mut sf = make_scored_fact("archival", None, 1.0);
    apply_class_aware_penalty(&mut sf);
    assert!((sf.score - 1.0).abs() < 1e-6);
}

#[test]
fn convention_never_decays() {
    let mut sf = make_scored_fact("convention", Some("2026-01-01"), 1.0);
    apply_class_aware_penalty(&mut sf);
    assert!((sf.score - 1.0).abs() < 1e-6);
}

#[test]
fn procedural_never_decays() {
    let mut sf = make_scored_fact("procedural", Some("2026-01-01"), 1.0);
    apply_class_aware_penalty(&mut sf);
    assert!((sf.score - 1.0).abs() < 1e-6);
}

#[test]
fn unknown_class_treated_as_current() {
    let mut sf = make_scored_fact("mystery", Some("2026-01-01"), 1.0);
    apply_class_aware_penalty(&mut sf);
    assert!((sf.score - 0.3).abs() < 1e-6);
}
```

For testability, extract the penalty logic into a helper:

```rust
fn apply_class_aware_penalty(sf: &mut ScoredFact) {
    let class = sf.fact.epistemic_class.as_deref().unwrap_or("current");
    match class {
        "archival" => { if sf.fact.valid_until.is_some() { sf.score *= 0.3; } }
        "current" => { if sf.fact.valid_until.is_some() { sf.score *= 0.1; } }
        "convention" | "procedural" => {}
        _ => { if sf.fact.valid_until.is_some() { sf.score *= 0.3; } }
    }
}
```

Use this helper in the main scoring loop.

- [ ] **Step 4: Verify + commit**

Run: `cargo test --package gateway-execution -- recall`
Expected: 6 new tests pass, existing tests still green.

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`
Expected: Clean.

```bash
git add gateway/gateway-execution/src/recall.rs
git commit -m "feat(recall): class-aware scoring — archival never decays, current decays hard"
```

---

## Task 4: Final Checks

- [ ] **Step 1: Full test run**

Run: `cargo test --workspace --lib --bins --tests`
Expected: All pass.

- [ ] **Step 2: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: Clean.

- [ ] **Step 3: Cognitive complexity audit**

Run: `cargo clippy --package gateway-database --package gateway-execution --lib --tests -- -W clippy::cognitive_complexity 2>&1 | grep cognitive`
Expected: no flags on new Phase 6c code (all functions < 15).

- [ ] **Step 4: Push**

```bash
git push
```
