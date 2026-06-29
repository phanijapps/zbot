# Plan: KG Junk-Entity Filter

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executed (T1–T3 done, gates green; T4 deferred — see `memory-bank/backlog.md`)

## Approach

Single-source tightening of `extract_file_paths` (the private matcher that feeds `extract_shell`'s `File` entities). Add reject rules for globs, code syntax, shell expansion, API routes, and symbol-only segments; widen the trailing-punct trim to include backtick/angle. TDD pins rejection + a real-path guard.

## Tasks

### T1: Red — junk-rejection + real-path tests
**Depends on:** none
**Tests:** `extract_file_paths_rejects_junk_paths` (fails on old matcher) + `extract_file_paths_keeps_real_file_paths` (guard).
**Done when:** rejection test compiles and fails red on the current matcher.

### T2: Tighten extract_file_paths
**Depends on:** T1
**Approach:** widen `TRIM_CHARS` (add `` ` `` `<` `>`); add `SYNTAX` reject (`* ? < > ` |`); reject `${`, `$(`; reject `/api/` prefix; require the last path segment to contain an alphanumeric char.
**Done when:** T1 green.

### T3: Gates
**Depends on:** T2
**Done when:** `cargo test -p gateway-execution` green (467 passed); `cargo clippy -p gateway-execution --lib` clean; existing shell/file-path tests unchanged.

### T4: Backfill existing junk file-entities (opt-in — Ask first)
**Depends on:** T3
**What:** remove/normalize the existing junk `file` entities already in the KG (the `/*`, `/api/*`, `/tmp/zbot-*`, backtick-suffixed rows from the data-quality assessment).
**Status:** deferred — data mutation; needs sign-off. Recorded in `memory-bank/backlog.md`.

## Risks
- Over-rejection: mitigated by the real-path guard test (`/tmp/foo.rs`, `./AGENTS.md` must survive).
- A real file path under `/api/…` would be dropped — acceptable (no project file lives under `/api/`).

## Changelog
- 2026-06-29: initial plan (light mode); T1–T3 executed same day.
