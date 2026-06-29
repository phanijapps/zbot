# Plan: Plan Compaction / On-Demand

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T2 done, gates green; PlanBlockMiddleware compaction deferred)

## Approach
`build_continuation_message` already persists the full plan to `ctx.<sid>.plan`. Replace its verbatim `{plan}` inline with `compact_plan_summary(&plan)` (step headers only) + an on-demand fetch pointer. Pure-fn helper + message edit; TDD the helper.

## Tasks
### T1: compact_plan_summary helper + tests
**Depends:** none — pure fn: extract `Step N` headers (one line each, cap 20), fallback to first 5 non-empty lines. Two unit tests (headers-only; fallback).
### T2: continuation message uses compact outline + on-demand pointer
**Depends:** T1 — `build_continuation_message` inlines the outline + `memory(get_fact, key="ctx.<sid>.plan")` pointer instead of `{plan}`.
**Done when:** `cargo test -p gateway-execution` green (compact tests + continuation/middleware tests); `cargo check --workspace` clean; clippy clean.

## Deferred
- `PlanBlockMiddleware` (per-turn plan block re-render) compaction — separate mechanism; bigger change; not this slice.
- A config lever to toggle verbatim-vs-compact (currently always-compact; can gate if a regression appears).

## Risks
- Orchestrator needs a step's full detail → one extra `memory(get_fact)` call. Acceptable (the plan is one hop away); preferred over re-sending 5K chars every continuation.
- Plan with unusual format (no `Step N` headers) → fallback to first 5 lines; still smaller than verbatim.

## Changelog
- 2026-06-29: initial plan (light mode); T1–T2 executed same day.
