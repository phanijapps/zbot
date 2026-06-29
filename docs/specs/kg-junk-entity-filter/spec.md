# Spec: KG Junk-Entity Filter

- **Status:** Implementing
- **Plan:** [`plan.md`](plan.md)
- **Shape:** service
- **Mode:** light (no risk trigger fired — tightens an existing private matcher; no new module/layer/dependency)

## Objective

Tighten `extract_file_paths` so the knowledge graph records only **durable file paths** from shell tool output — not code syntax, API routes, globs, or shell expansion. This keeps the KG clean for hierarchical-taxonomy navigation (junk `file` entities like `/*`, `/api/curator/cleanup`, `/tmp/zbot-*` pollute clusters and break traversal).

**Root cause (verified):** `extract_file_paths` (`gateway/gateway-execution/src/tool_result_extractor.rs:182`) accepted any token starting with `/` or `./` containing a slash — so `/*`, `/>`, `/api/curator/cleanup`, `/tmp/zbot-*`, and `./housekeep.sh\`` all became `File` entities (confirmed in the live KG via the data-quality assessment).

## Boundaries

### Always do
- Reject globs (`*`, `?`), code syntax (`<`, `>`, backtick, `|`), shell expansion (`${`, `$(`), API routes (`/api/…`), and symbol-only path segments (`/*`, `/>`).
- Keep real file paths, including legitimate `/tmp/foo.rs` (an existing test asserts it).

### Ask first
- Backfilling/cleaning the existing junk `file` entities already in the DB (data mutation).

### Never do
- Change what `extract_multimodal` / `extract_people` / the regex extractors produce (out of scope — they don't emit these path-typed entities).
- Reject all `/tmp/` paths (breaks the legit `/tmp/foo.rs` case and the existing test).

## Testing Strategy

**TDD** — `extract_file_paths_rejects_junk_paths` (red on the old matcher) + `extract_file_paths_keeps_real_file_paths` (regression guard ensuring no over-rejection). Existing `shell_success_extracts_file_paths` + `file_path_extractor_caps_at_ten` must stay green.

## Acceptance Criteria

- [x] `extract_file_paths` rejects `/*`, `/>`, `/api/curator/cleanup`, `/tmp/zbot-*`, `$(cmd)`, `${VAR}`, and trims trailing backtick/angle.
- [x] Real paths survive (`/tmp/foo.rs`, `./src/bar.rs`, `./AGENTS.md`).
- [x] Existing shell/file-path tests green; `cargo test -p gateway-execution` green (467 passed); clippy clean.

## Assumptions

- Technical: `/tmp/foo.rs` is a legitimate file path to record (existing `shell_success_extracts_file_paths` test depends on it).
- Technical: trailing backtick/angle-bracket on a token is shell-output formatting, not part of the path (trim, don't reject the whole token).
- Process: light-mode work-loop; lean spec + single bounded review folded into the casing-slice review cadence.
