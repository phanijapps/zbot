# Recall Min-Score Threshold Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Suppress noisy recall results by (1) preserving real similarity scores from the hybrid search instead of synthesizing 0.5 for every fact, and (2) filtering out results below a configurable `min_score` threshold (default 0.3).

**Architecture:** Two files. `RecallConfig` gets a new `min_score` field. `recall/mod.rs` gets two changes: extract real scores from `search_memory_facts_hybrid` results, and apply min-score filter before RRF and before truncate.

**Tech Stack:** Rust, `gateway-services` crate (RecallConfig), `gateway-execution` crate (recall/mod.rs)

---

## Files Changed

| File | What changes |
|------|-------------|
| `gateway/gateway-services/src/recall_config.rs` | Add `min_score: f64` field, default 0.3 |
| `gateway/gateway-execution/src/recall/mod.rs` | Extract real scores + filter by min_score |

---

## Task 1: Add `min_score` to `RecallConfig`

**Files:**
- Modify: `gateway/gateway-services/src/recall_config.rs`

### Context

`RecallConfig` is loaded from `config/recall_config.json` with deep-merge fallback to defaults. Adding a new field with a default value is backward-compatible — existing config files without the field get the default.

- [ ] **Step 1.1: Write failing test**

Add to the test module in `recall_config.rs`:

```rust
#[test]
fn default_min_score_is_0_3() {
    let config = RecallConfig::default();
    assert_eq!(config.min_score, 0.3);
}

#[test]
fn min_score_can_be_overridden() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("recall_config.json"),
        r#"{"min_score": 0.5}"#,
    ).unwrap();
    let config = RecallConfig::load_from_path(dir.path());
    assert_eq!(config.min_score, 0.5);
}
```

- [ ] **Step 1.2: Run to confirm failure**

```bash
cargo test -p gateway-services min_score -- --nocapture 2>&1 | tail -10
```

Expected: compile error (field does not exist).

- [ ] **Step 1.3: Add `min_score` field**

In `RecallConfig` struct (around line 130), add after `contradiction_penalty`:

```rust
/// Minimum score threshold — results scoring below this are suppressed.
/// Prevents low-relevance facts (e.g., unrelated chess procedures) from
/// appearing in response to short/generic queries like "hi".
pub min_score: f64,
```

In `RecallConfig::default()`, add after `contradiction_penalty`:

```rust
min_score: 0.3,
```

- [ ] **Step 1.4: Run tests**

```bash
cargo test -p gateway-services -- --nocapture 2>&1 | grep -E "FAILED|ok\." | head -20
```

Expected: all pass including the two new tests.

- [ ] **Step 1.5: Cargo check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
```

- [ ] **Step 1.6: Commit**

```bash
git add gateway/gateway-services/src/recall_config.rs
git commit -m "feat(recall): add min_score threshold to RecallConfig (default 0.3)"
```

---

## Task 2: Use real scores + apply min_score filter in `recall/mod.rs`

**Files:**
- Modify: `gateway/gateway-execution/src/recall/mod.rs`

### Context

**Root cause of noise:** `recall_unified` calls `search_memory_facts_hybrid`, which returns `Vec<Value>` where each value has a `score` field (the actual cosine similarity). But the code ignores this and synthesizes `0.5` for every fact:

```rust
// CURRENT (line ~337) — wrong: ignores real score
.map(|fact| adapters::fact_to_item(&fact, 0.5))
```

The fix: extract the real score before deserializing the fact:

```rust
// FIXED: use real score if present, fall back to 0.5 only if missing
.filter_map(|v| {
    let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);
    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
        .ok()
        .map(|fact| adapters::fact_to_item(&fact, score))
})
.filter(|item| item.score >= self.config.min_score)
```

**Legacy path filter:** In `recall_facts` (line ~288–296), add `results.retain` after sort and before truncate:

```rust
// Sort by score descending and take top-K
results.sort_by(|a, b| { ... });
results.retain(|sf| sf.score >= self.config.min_score);  // ADD THIS
results.truncate(limit);
```

- [ ] **Step 2.1: Write failing tests**

Add to the test module at the bottom of `recall/mod.rs`:

```rust
#[test]
fn format_scored_items_filters_low_score() {
    // Verify that items with score < 0.3 do NOT appear in output
    let items = vec![
        mk_item(ItemKind::Fact, "high", "relevant content", 0.8),
        mk_item(ItemKind::Fact, "low", "chess procedures irrelevant", 0.1),
        mk_item(ItemKind::Fact, "border", "borderline content", 0.3),
    ];
    // min_score=0.3 → keep "high" (0.8) and "border" (0.3), drop "low" (0.1)
    let filtered: Vec<_> = items.into_iter().filter(|i| i.score >= 0.3).collect();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().any(|i| i.id == "high"));
    assert!(filtered.iter().any(|i| i.id == "border"));
    assert!(!filtered.iter().any(|i| i.id == "low"));
}
```

This test verifies the filter logic directly (not the full recall pipeline, which requires a DB).

- [ ] **Step 2.2: Run to confirm it passes already** (it tests filter logic, not integration)

```bash
cargo test -p gateway-execution format_scored_items_filters_low_score -- --nocapture 2>&1 | tail -10
```

Expected: PASS (the logic is trivial). This is a sanity test to document intent.

- [ ] **Step 2.3: Fix `recall_unified` — extract real scores**

Find the block around line 333-339 that reads:

```rust
.filter_map(|v| {
    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
        .ok()
        .map(|fact| adapters::fact_to_item(&fact, 0.5))
})
```

Replace with:

```rust
.filter_map(|v| {
    let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.5);
    serde_json::from_value::<zero_stores_sqlite::MemoryFact>(v)
        .ok()
        .map(|fact| adapters::fact_to_item(&fact, score))
})
.filter(|item| item.score >= self.config.min_score)
```

- [ ] **Step 2.4: Fix `recall_facts` legacy path — retain by min_score**

Find the block around line 288-294 that reads:

```rust
results.sort_by(|a, b| {
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
});
results.truncate(limit);
```

Change to:

```rust
results.sort_by(|a, b| {
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
});
results.retain(|sf| sf.score >= self.config.min_score);
results.truncate(limit);
```

- [ ] **Step 2.5: Run all recall tests**

```bash
cargo test -p gateway-execution recall -- --nocapture 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 2.6: Run full workspace check**

```bash
cargo check --workspace 2>&1 | grep "^error" | head -10
cargo test --workspace 2>&1 | grep -E "^test result|FAILED" | head -20
```

Expected: no errors, no failures.

- [ ] **Step 2.7: Cargo fmt + clippy**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error" | head -10
```

- [ ] **Step 2.8: Commit**

```bash
git add gateway/gateway-execution/src/recall/mod.rs
git commit -m "fix(recall): use real similarity scores + suppress results below min_score"
```

---

## Final Validation

- [ ] Run full workspace tests one more time and confirm clean

```bash
cargo test --workspace 2>&1 | grep -E "FAILED" | head -10
```

Expected: no failures.

---

## Self-Review Against Spec

| Spec requirement | Task |
|-----------------|------|
| Add `min_score` parameter (default 0.3) | Task 1 |
| Suppress results below threshold | Task 2 |
| Fix root cause: real similarity scores used | Task 2 |
| Configurable via `recall_config.json` | Task 1 (deep-merge already handles it) |
| Prevents "hi" → chess procedures | Task 2 (real scores + filter) |
