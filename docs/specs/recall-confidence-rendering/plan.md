# Plan: Recall Confidence Rendering

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T3 done, gates green)

## Approach
One-line format change in `format_scored_items`: include `item.score` as `{:.2}` after the kind tag for non-belief items. Beliefs untouched (already inline-confidence). TDD a red confidence-presence test, then update the four existing format tests that asserted the old `- [tag] content` shape.

## Tasks
### T1: Red — confidence-presence test
**Depends on:** none — `format_scored_items_renders_confidence_for_non_beliefs` asserts `- [fact 0.95]` / `- [fact 0.31]`. Fails today (no score).
### T2: Render score
**Depends on:** T1 — change `"- [{}] {}"` → `"- [{} {:.2}] {}"` with `item.score`.
**Done when:** T1 green.
### T3: Update existing format tests
**Depends on:** T2 — update `tags_each_kind` (6 assertions → `X.XX`) + `groups_beliefs` (`- [fact 1.00]`).
**Done when:** `cargo test -p gateway-execution` green (468 passed); clippy clean.

## Deferred
- Render provenance `source` alongside score (token-cost tradeoff — needs a verbosity call). Not in this slice.

## Changelog
- 2026-06-29: initial plan (light mode); T1–T3 executed same day.
