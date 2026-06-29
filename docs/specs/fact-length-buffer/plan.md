# Plan: Fact-Length 20% Buffer

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (gates green)

## Approach
Add `MAX_FACT_CONTENT_CHARS_HARD = MAX_FACT_CONTENT_CHARS * 6 / 5` (960). `validate_fact_content` rejects above HARD (not the 800 estimate). Update the reject test to HARD+1; add a buffer-accept test.

## Tasks
### T1: HARD const + validate threshold + tests
**Done when:** `cargo test -p zbot-stores-sqlite` green (276: buffer-accept + reject-at-961 + short + exemption); clippy clean.

## Risks
- Slightly longer facts admitted (800–960) — acceptable; the cap is a guideline.

## Changelog
- 2026-06-29: initial plan (light mode); executed same day.
