# Spec: KG Entity Type Casing

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** data
- **Mode:** light (no risk trigger fired — data-value normalization; no new module/layer/dependency)

> **Spec contract:** this document defines what "done" means. The implementing PR must match this spec, or update it.

## Objective

Eliminate the `entity_type` casing collision (e.g. `"Concept"` vs `"concept"`) at its source so the knowledge graph stores **one canonical type string per kind**. This is the foundation for hierarchical-taxonomy `is_a`/`part_of` traversal, which breaks the instant a type exists under two casings — a navigated hierarchy would treat `Concept` and `concept` as distinct types and split clusters.

**Root cause (verified):** `GraphStorage::promote_cluster_to_aggregate` (`stores/zbot-stores-sqlite/src/kg/storage.rs:~1900`) hardcodes the string literal `'Concept'` in its INSERT, bypassing `EntityType::as_str()`, which canonicalizes to lowercase `"concept"` (`services/knowledge-graph/src/types.rs`). All 135 capitalized `"Concept"` rows in the live DB are layer>0 aggregates from this path (layer>0 entity count == 135 == capitalized `"Concept"` count).

## Boundaries

### Always do
- Emit every `entity_type` via `EntityType::as_str()` — the single source of truth for casing. No type string literals in INSERT paths.

### Ask first
- Backfilling the 135 existing capitalized `"Concept"` rows to `"concept"` (a guarded, idempotent UPDATE). Data mutation — needs sign-off before it ships.

### Never do
- Change the `EntityType` enum variants or the `as_str()`/`from_str()` mapping (that would shift canonical casing broadly, not fix the leak).
- Touch entity **name** normalization (`normalize_entity_name`) or the resolver/dedup cascade — those are correct.
- Touch `memory_facts` (separate quality slice).
- Bulk-delete entities.

## Testing Strategy

**TDD** — the casing contract is a compressible invariant. Red test: `promote_cluster_to_aggregate` produces an entity whose `entity_type == "concept"` (fails today — it writes `"Concept"`). Green after the fix. Plus a **goal-based grep gate** confirming no hardcoded `entity_type` string literals remain in production INSERT paths. The conformance suite must stay green (entity round-trip is unaffected — `from_str` is already case-insensitive on read).

## Acceptance Criteria

- [x] `promote_cluster_to_aggregate` writes canonical lowercase `entity_type` (verified by a failing→passing test).
- [x] No production INSERT path hardcodes an `entity_type` string literal; all route through `EntityType::as_str()` (grep gate — `stores/ services/ gateway/ runtime/ apps/`, `#[cfg(test)]` excluded).
- [x] Conformance + existing aggregate/hierarchy tests pass; `cargo test -p zbot-stores-sqlite` (and the knowledge-graph crate) green.
- [ ] (deferred: kg-entity-type-casing-t4-backfill) existing 135 capitalized rows normalized to `"concept"` — T4, Ask-first; see `memory-bank/backlog.md`.

## Assumptions

- Technical: canonical casing is lowercase per `EntityType::as_str()` (`types.rs`) — verified.
- Technical: the 135 capitalized `"Concept"` rows are all layer>0 aggregates from `promote_cluster_to_aggregate` (layer>0 count == 135 == capitalized-Concept count) — verified via the live DB.
- Technical: `EntityType::from_str` is case-insensitive on read, so existing capitalized rows still deserialize correctly — a casing fix is read-compatible.
- Technical: `EntityType::as_str()` does NOT lowercase `Custom(s)` on the write side (returns the caller's string verbatim); only `from_str` lowercases on parse. No production INSERT builds a Custom-typed entity literal today (grep-confirmed), so this slice is complete — a write-side guard is warranted only if extraction ever constructs `EntityType::Custom(..)` directly.
- Process: light-mode work-loop (lean spec, single bounded adversarial review, no loop-cohort).
