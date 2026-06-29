# Spec: Recall Confidence Rendering

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light (no risk trigger — additive prompt rendering; no new module/layer/dependency)

## Objective

Render each recalled item's `score` inline so the agent can **weight recall** — a 0.95 schema fact vs a 0.31 borderline guess. Today `format_scored_items` emits `- [fact] {content}` for non-beliefs with no score, so every recalled item reads as equally trustworthy. That uniform weighting is a direct "hit and miss" root cause: the LLM can't down-weight weak recall or lean on strong recall. (Beliefs already show `[belief <conf>]` inline.)

**Root cause (verified):** `format_scored_items` (`gateway/gateway-execution/src/recall/mod.rs`) formats non-belief lines as `"- [{}] {}"` — kind tag + content, dropping `ScoredItem.score` (`gateway-memory/src/recall/scored_item.rs:54`).

## Boundaries

### Always do
- Render `score` for non-belief items as `- [tag X.XX] content` (compact, token-efficient).

### Ask first
- Also rendering provenance `source` — deferred; adds tokens per item, needs a call on verbosity vs cost.

### Never do
- Change ranking / RRF / MMR (that's the deferred "accuracy" work).
- Change belief rendering (already shows confidence inline).

## Testing Strategy

**TDD** — `format_scored_items_renders_confidence_for_non_beliefs` (red: score absent → green: `- [fact 0.95]` / `- [fact 0.31]`). The existing `format_scored_items_*` tests asserted the old `- [tag] content` format and were updated to `- [tag X.XX] content`.

## Acceptance Criteria

- [x] Non-belief items render `- [tag X.XX] content` (score inline, 2 decimals).
- [x] Belief rendering unchanged (`- {content}` under `## Active Beliefs`).
- [x] Existing `format_scored_items_*` tests updated and green; `cargo test -p gateway-execution` green (468 passed); clippy clean.

## Assumptions
- Technical: `score` is the primary weight signal; provenance `source` is secondary and deferred for token economy.
- Process: light-mode work-loop; lean spec + TDD.
