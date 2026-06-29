# Plan: Tool-Result Compaction Cap

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T2 done, gates green)

## Approach
The compaction *mechanism* exists and is enabled (`ContextEditingMiddleware` clears old tool results, keeps recent N). It just never fires because the trigger (70-80% of a large window) is unreachable in normal sessions. Add an absolute token cap (`min(fraction, cap)`) so it fires at a working-set size. Pure-fn helper + one-line wiring change; TDD the helper.

## Tasks
### T1: effective_compaction_threshold helper + tests
**Depends:** none — `fn effective_compaction_threshold(window, pct, cap) -> usize { min(window*pct/100, cap) }`. Two tests: caps on large windows; fraction governs on small.
### T2: wire the cap into build_runtime_middleware_pipeline
**Depends:** T1 — 3-tuple `(trigger_pct, keep_results, compaction_cap)` (chat 80/5/32K, deep 70/8/64K); `threshold = effective_compaction_threshold(...)`.
**Done when:** `cargo test -p gateway-execution` green (472, incl. new tests + middleware-order); `cargo check --workspace` clean; clippy clean.

## Risks
- Over-compaction (cap too low) clears tool results the agent still needs → mitigated by keep-policy (recent 5/8) + on-demand re-fetch; cap is tunable.
- SummarizationMiddleware shares the threshold; it runs after ContextEditing (which reclaims space first), so it rarely double-fires.

## Deferred
- Decoupling the Summarization threshold from the ContextEditing threshold (if summarization proves too aggressive at the lowered threshold).
- Budgeted recall (Spec 3 P12) — separate; recall is currently unbudgeted.

## Changelog
- 2026-06-29: initial plan (light mode); T1–T2 executed same day.
