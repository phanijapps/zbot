# Plan: KG Entity Type Casing

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T3 done, gates green, adversarial review clean; T4 deferred — see `memory-bank/backlog.md`)

## Approach

The casing collision is a single-source bug (hardcoded `'Concept'` literal) plus possibly sibling literals in other INSERT paths. Fix at source: route every `entity_type` through `EntityType::as_str()`. First enumerate all hardcoded `entity_type` literals in production INSERTs (grep), then replace each with the enum's canonical string. TDD pins the aggregate path; a grep gate pins "no literals remain." Backfill of the 135 existing rows is an opt-in, sign-off-gated follow-up.

## Construction tests

- Per-task tests live under **Tasks** below.
- **Cross-cutting:** conformance suite (`zbot-stores-conformance`) stays green — the casing fix must not regress entity round-trip. Manual verification: after T3, query `SELECT entity_type, COUNT(*) FROM kg_entities GROUP BY entity_type` on a fresh extraction and confirm no capitalized variants for enum types.

## Tasks

### T1: Red — aggregate casing test

**Depends on:** none

**Tests:**
- `promote_cluster_to_aggregate` creates an entity whose stored `entity_type == "concept"` (lowercase). Fails today — it writes `"Concept"`. Verifies AC#1.

**Approach:**
- Add a unit test in `stores/zbot-stores-sqlite/src/kg/storage.rs` (tests module) that promotes a small cluster to an aggregate, then reads the row back and asserts `entity_type == "concept"`.

**Done when:** the test compiles and fails (red) on the current hardcoded `'Concept'`.

### T2: Enumerate all hardcoded entity_type literals (goal-based)

**Depends on:** none

**Tests:** no stub (goal-based check).

**Approach:**
- `grep -rnE "entity_type" stores/ services/ gateway/ runtime/ apps/ --include=*.rs`, then filter for INSERT/VALUES sites carrying a string literal (e.g. `'Concept'`, `'Person'`) outside `#[cfg(test)]`. Produce the complete list of production sites to fix.

**Done when:** a complete list of hardcoded `entity_type` literals in production INSERT paths exists (T3 fixes each).

### T3: Green — route entity_type through EntityType::as_str()

**Depends on:** T1, T2

**Tests:**
- T1 turns green; add an equivalent assertion for any other source T2 found.

**Approach:**
- Replace each hardcoded literal with `EntityType::<Variant>.as_str()` (start with `promote_cluster_to_aggregate`'s `'Concept'` → `EntityType::Concept.as_str()`). Confirm the sqlite crate imports `EntityType`.

**Done when:** T1 green; grep gate (no production `entity_type` string literals in INSERTs) clean; conformance green; `cargo test -p zbot-stores-sqlite` green.

### T4: Backfill existing capitalized rows (opt-in — Ask first)

**Depends on:** T3

**Tests:**
- Idempotent: re-running the backfill is a no-op (0 rows updated on second run).

**Approach:**
- Guarded `UPDATE kg_entities SET entity_type = ? WHERE entity_type = ? AND ...` mapping known capitals (`'Concept'` → `"concept"`, etc.), behind a CLI/flag, idempotent. Only the enum-type capitals; leave `Custom` types untouched.

**Done when:** 0 rows with capitalized enum `entity_type`; re-run is a no-op. **Skipped this slice unless explicitly approved** (recorded as deferred otherwise).

## Rollout

- **Delivery:** the source fix (T1–T3) ships directly — behavior change only for *new* aggregates; existing rows unaffected. The backfill (T4) is opt-in, reversible (re-runnable; `from_str` is case-insensitive so reads never broke).
- No schema migration; no new infra; no new dependency.

## Risks

- T2 may surface more hardcoded-literal sites than the one confirmed. Scope grows linearly but stays mechanical.
- `EntityType::Custom(String)`: `as_str()` returns the caller's string **verbatim** (NOT lowercased) on the write side; only `from_str` lowercases on parse. No production INSERT constructs a Custom-typed entity literal today (grep-confirmed), so this slice is complete — track a write-side guard if extraction ever builds `EntityType::Custom(..)` directly instead of via `from_str`.

## Changelog
- 2026-06-29: initial plan (light mode).
