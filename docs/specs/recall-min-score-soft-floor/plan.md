# Plan: Recall min_score Soft Floor

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T3 done, gates green)

## Approach
Replace the hard `≥ min_score` facts filter with a two-tier soft floor: collect all non-superseded facts, then `apply_soft_floor` keeps confident (≥ min_score) + up to `low_conf_tail` borderline (≥ low_conf_floor). The borderline tail enters RRF fusion with a low score (rendered inline by slice 3) so the agent sees and down-weights it. Config-gated (`low_conf_floor`/`low_conf_tail`) so the tradeoff is tunable.

## Tasks
### T1: Config fields + defaults
**Depends:** none — add `low_conf_floor: f64` + `low_conf_tail: usize` to `RecallConfig` (default 0.1 / 3). Existing partial configs deep-merge onto Default.
### T2: apply_soft_floor helper + facts wiring + test
**Depends:** T1 — add `apply_soft_floor(items, min_score, low_conf_floor, low_conf_tail)`; restructure `recall_unified` facts chain to collect-all then `apply_soft_floor`; add the unit test.
**Done when:** `cargo test -p gateway-memory` green (232); `cargo check --workspace` clean; clippy clean.

## Risks
- Precision→recall shift surfaces more borderline facts: mitigated by `low_conf_tail` cap (default 3) + confidence rendering (slice 3 lets the agent down-weight). Defaults tunable via config.
- Other crates full-literal-constructing `RecallConfig`: workspace check clean (none do — all use `..Default::default()` or Default).

## Deferred
- Apply the soft floor to the legacy `recall()` path (`:366`) — secondary path; not in this slice.

## Changelog
- 2026-06-29: initial plan (light mode); T1–T2 executed same day.
