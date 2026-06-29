# Spec: Plan Compaction / On-Demand

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light — **behavior change** (user-approved). No new module/layer/dependency.

## Objective
Stop re-inlining the full `plan.md` verbatim on every continuation. Today `build_continuation_message` reads the entire plan and embeds it (`"[DELEGATION COMPLETED. YOUR PLAN IS BELOW.…]\n\n{plan}"`) each time the orchestrator resumes — so a 5K-char plan is re-sent on every continuation (4× in the observed session), and the orchestrator re-reads all steps every time ("reading all the steps doesn't make sense"). The plan is **already persisted to `ctx.<sid>.plan`** by `plan_snapshot`; the fix inlines a **compact step-outline** instead and points the orchestrator to fetch full step details on-demand.

**Root cause (verified):** `build_continuation_message` (`gateway-execution/src/runner/core.rs:272`) inlined `{plan}` verbatim even though `plan_snapshot` already wrote it to `ctx.<sid>.plan` for on-demand fetch.

## Boundaries
### Always do
- Inline only a compact step outline (one line per `Step N` header; fallback to first non-empty lines if no headers).
- Keep `[DELEGATION COMPLETED.` prefix (no code parses the old marker, but it's a stable signal).
- Keep `plan_snapshot` writing the full plan to `ctx.<sid>.plan`.
### Ask first
- Also compacting `PlanBlockMiddleware`'s per-turn plan block (separate mechanism; bigger change).
### Never do
- Remove the full plan from `ctx.<sid>.plan` (it's the on-demand source).
- Change `find_latest_plan` / `plan_snapshot`.

## Testing Strategy
**TDD** — `compact_plan_summary` is a pure fn: `extracts_step_headers_only` (step headers kept, bodies/non-step content excluded) + `falls_back_to_first_lines_when_no_step_headers`. The message wiring is verified by compile + the existing continuation/middleware tests staying green.

## Acceptance Criteria
- [x] `build_continuation_message` inlines `compact_plan_summary(&plan)` + an on-demand pointer (`memory(get_fact, key="ctx.<sid>.plan")`), not the verbatim plan.
- [x] Full plan still written to `ctx.<sid>.plan` (plan_snapshot unchanged).
- [x] `compact_plan_summary` tests pass; existing continuation/plan/middleware tests green (no regression); `cargo check --workspace` clean; clippy clean.

## Assumptions
- Technical: `plan_snapshot` writes `ctx.<sid>.plan` (verified at core.rs:294) — the orchestrator can fetch it via `memory(get_fact)`.
- Product: the compact outline (step headers) is enough for the orchestrator to track progress; it fetches full details only when needed. Safe because the full plan is one get_fact away.
- Process: light-mode work-loop.
