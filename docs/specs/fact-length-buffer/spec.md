# Spec: Fact-Length 20% Buffer

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light (no risk trigger — relaxed an existing validation constant).

## Objective
Treat the 800-char fact-content cap as a **soft estimate with a ~20% buffer** so borderline-over content is accepted, not rejected. Observed failure: `save_fact failed for skill duckduckgo-search: fact content too long: 836 chars (max 800)` — a skill description 36 chars over the cap was rejected outright. With the buffer, the hard reject moves to 960 (800 × 1.2); 836 passes.

**Root cause (verified):** `validate_fact_content` (`stores/zbot-stores-sqlite/src/memory_fact_store.rs:45`) rejected any content > `MAX_FACT_CONTENT_CHARS` (800) with no tolerance.

## Boundaries
### Always do
- Keep `MAX_FACT_CONTENT_CHARS = 800` as the documented estimate; enforce the hard reject at `MAX_FACT_CONTENT_CHARS_HARD = 800 × 6/5 = 960`.
### Ask first
- Changing the buffer % (20%) — it's a tuning constant.
### Never do
- Remove the cap entirely (genuinely oversized facts still rejected > 960).
- Touch the `ctx` / `primitive` exemption (machine-generated categories stay exempt).

## Testing Strategy
**TDD** — `validate_fact_content_accepts_within_20pct_buffer` (836 + exactly-960 accepted) + updated `validate_fact_content_rejects_oversized_fact` (961 rejected). Existing short-fact + exemption tests unchanged.

## Acceptance Criteria
- [x] Content ≤ 960 chars accepted; > 960 rejected (was > 800).
- [x] `ctx` / `primitive` categories still exempt (any length).
- [x] `cargo test -p zbot-stores-sqlite` green (276); clippy clean.

## Assumptions
- Technical: the 800→960 relaxation doesn't materially harm recall quality (facts stay concise; the buffer only admits slightly-longer legitimate content like skill descriptions).
- Product: the cap is a guideline ("1-3 sentences"), not a hard contract — a buffer matches that intent.
