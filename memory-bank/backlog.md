# Backlog

Deferred items captured during work-loop slices — version-controlled so they don't rot in PR comments or chat. Each entry pairs with a `(deferred: <anchor>)` marker in its spec's Acceptance Criteria.

## kg-entity-type-casing — T4 entity-type backfill

- **Anchor:** `kg-entity-type-casing-t4-backfill`
- **Source:** `docs/specs/kg-entity-type-casing/` (AC#4, plan task T4).
- **What:** backfill the ~135 existing capitalized `"Concept"` `kg_entities` rows (all layer>0 aggregates from the pre-fix `promote_cluster_to_aggregate`) to canonical lowercase `"concept"`. The source matcher is fixed; these are historical residue.
- **Status:** **Ask-first** — data mutation; needs sign-off. Not done in the casing-fix slice (T1–T3 only).
- **Approach when picked up:** guarded, idempotent `UPDATE kg_entities SET entity_type = 'concept' WHERE entity_type = 'Concept'` (map known enum capitals only; leave `Custom` types untouched); assert re-run is a no-op; confirm 0 capitalized enum `entity_type` rows after.

## kg-junk-entity-filter — T4 junk file-entity cleanup

- **Anchor:** `kg-junk-entity-filter-t4-cleanup`
- **Source:** `docs/specs/kg-junk-entity-filter/` (plan task T4).
- **What:** remove/normalize the existing junk `file` entities already in the KG — `/*`, `/>`, `/api/*` routes, `/tmp/zbot-*` globs, backtick-suffixed path rows surfaced by the data-quality assessment. The source matcher is now fixed; these are historical residue.
- **Status:** **Ask-first** — data mutation (deletion/rewrite of existing rows); needs sign-off. Not done in the filter slice.
- **Approach when picked up:** select `file`-typed entities whose name matches the now-rejected junk patterns (globs, `/api/` prefix, symbol-only segments, trailing backtick), review the candidate set, then delete or re-link; verify no relationships dangle.
