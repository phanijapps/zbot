# Spec: Tool-Result Compaction Cap

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light — **behavior change** (user-approved). No new module/layer/dependency.

## Objective
Make `ContextEditingMiddleware` actually fire in normal sessions so tool results stop accumulating for an entire session. Today the trigger is `context_window × trigger_pct` (chat 80% / deep 70%) — on a 200K window that's **140–160K tokens**, which normal sessions never reach, so the middleware (which clears old tool results, keeping only the recent 5/8) **never fires** and tool results re-send every turn (observed: a 2.3M-token session). The fix caps the threshold at a normal working-set size (chat 32K / deep 64K).

**Root cause (verified):** `build_runtime_middleware_pipeline` (`gateway-execution/src/invoke/executor.rs:388`) set `threshold = context_window_tokens × trigger_pct` with no absolute cap; on large windows the fraction threshold is unreachable.

## Boundaries
### Always do
- Cap the threshold at `compaction_cap` (chat 32K / deep 64K) via `min(fraction_threshold, cap)`.
- Keep `ContextEditingMiddleware`'s existing keep-policy (recent 5/8 tool results) — only the *trigger* changes.
### Ask first
- Adjusting the cap values (32K/64K) — precision/retention tradeoff; tune by observation.
### Never do
- Change the keep-policy, the clear-vs-summarize behavior, or the pipeline order.
- Remove the fraction threshold (small windows still need it).

## Testing Strategy
**TDD** — `effective_compaction_threshold(window, pct, cap)` is a pure fn: `caps_on_large_context_windows` (200K×70% → 64K) + `uses_fraction_on_small_context_windows` (8K×80% → 6.4K, below cap). Existing `runtime_middleware_order_*` tests confirm the pipeline still builds correctly.

## Acceptance Criteria
- [x] Threshold = `min(window × pct, compaction_cap)`; chat cap 32K, deep cap 64K.
- [x] `effective_compaction_threshold` tests pass; middleware-order tests green (no regression); `cargo check --workspace` clean; clippy clean.

## Assumptions
- Technical: `ContextEditingMiddleware` (which runs before `SummarizationMiddleware`) reclaims space by clearing old tool results; Summarization then sees the reduced count and rarely fires. So sharing the lowered threshold is safe.
- Product: keeping only the recent 5/8 tool results is acceptable — the agent re-fetches older data on-demand (the plan-on-demand slice 6 + the orchestrator's re-read guidance). The cap is the lever for the tokens-vs-retention tradeoff.
- Process: light-mode work-loop.
