# Spec: Recall min_score Soft Floor

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light — but a **behavior change** (user-approved). No new module/layer/dependency.

## Objective

Stop silently dropping borderline-relevant facts. SQLite-vec hybrid routinely scores legitimately-relevant facts **0.1–0.3**, but `recall_unified`'s facts path hard-filtered at `min_score = 0.3` — so those facts never entered fusion (gap-analysis root cause #1 for "hit and miss"). With the soft floor, recall keeps the confident set (≥0.3) **plus a small bounded tail** of borderline facts (≥0.1, capped at 3) that enter fusion with their low score rendered inline (slice 3) — so the agent sees them and down-weights them instead of never seeing them.

**Root cause (verified):** `recall_unified` facts chain filtered `.filter(|item| item.score >= self.config.min_score)` at `recall/mod.rs:417` before RRF fusion. `RecallConfig.min_score` defaulted to 0.3 (`lib.rs:270`).

## Boundaries

### Always do
- Keep confident items (≥ `min_score`) unchanged; cap the borderline tail at `low_conf_tail` (default 3) so noise can't flood recall.
- Drop items below `low_conf_floor` (default 0.1).
### Ask first
- Adjusting the defaults (0.1 / 3) — they're a precision/recall tradeoff; tuned by observation.
### Never do
- Apply the soft floor to the legacy `recall()` path (`:366`) in this slice — it's the secondary path; unify later if needed.
- Remove the floor entirely (keep it bounded; the tail cap is the noise guard).

## Testing Strategy
**TDD** — `apply_soft_floor_keeps_confident_plus_bounded_borderline_tail` (confident kept; top-2 borderline kept; tail capped; below-floor dropped). The helper is a pure fn over `&mut Vec<ScoredItem>`, so the test is deterministic.

## Acceptance Criteria
- [x] `RecallConfig` has `low_conf_floor` (default 0.1) + `low_conf_tail` (default 3); existing config files deep-merge onto Default (no breakage — workspace check clean).
- [x] `recall_unified` facts path applies `apply_soft_floor` instead of the hard `≥ min_score` drop.
- [x] `cargo test -p gateway-memory` green (232, incl. new test); `cargo check --workspace` clean; clippy clean.

## Assumptions
- Technical: the borderline tail entering RRF fusion ranks low (low score) and the final `budget` truncation bounds the surfaced set — so the net effect is a few more low-conf facts visible, not noise flooding.
- Product: this is a deliberate precision→recall shift (user-approved behavior change); the confidence rendering (slice 3) is what makes it safe (agent can down-weight).
- Process: light-mode work-loop.
