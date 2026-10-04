# Verification: Ollama fenced JSON intent fallback

## Evidence and scope

User session sess-da6e58a6-55ef-5ede-b00a-c7ce3c140453 had a completed tool response rejected with unknown_resource, then a persisted deadline_exceeded fallback at the explicit 5000-token setting. A code fence is a transport boundary, not semantic authority: recommendations still pass the unchanged host validator. No resource sanitization or settings change is part of this follow-up.

Engram identified the implementation path but its retrieved body predates the prior local fix; current source was inspected. Existing generic JSON helpers perform lenient recovery, so the existing strict serde_json parser is reused after a small exact fence extractor. No parser framework or dependencies added.

## Verification

- TDD: ollama_missing_submission_falls_back_to_fenced_json compiled and failed on the unchanged analyzer at its expected valid-decision assertion before implementation; passes after the change.
- cargo check --workspace: passed in the isolated accepted baseline.
- cargo clippy -p gateway-execution --all-targets -- -D warnings: passed.
- Complete gateway-execution suite: 553 unit tests and all enabled integration tests passed; opt-in live/download tests retain their existing ignores.
- Intent HTTP suite: 22 passed, one opt-in live test ignored by default. Covers transport switch, one fenced correction, strict block ambiguity, unknown resources, safe fallback, terminal failures, max-three requests, native compatibility, explicit tokens and prompt canaries.
- cargo fmt --all -- --check and git diff --check: passed.

## Compiled live fallback

2026-10-03: the existing public analyzer was exercised using only a synthetic travel-research request and the configured Ollama endpoint credentials in process memory. The probe injected one controlled empty/missing-submission first response, then forwarded the actual tool-free fallback request to glm-5.3-flash:cloud with 5000 output tokens. Request modes were tools -> fenced_json: two analyzer requests, one real provider request. The parser and unchanged host validator accepted the model's fenced JSON graph decision in 13.87 seconds. Raw model output, prompts and credentials are not retained in this record.

The probe used metadata for 48 real skills, 7 real agents and 37 ward entries; existing context limits selected at most 20 skills. MCP catalog and procedure store were empty, and recall returned empty ranking data. This verifies the compiled fallback with populated host metadata; it is not a full-daemon latency or reliability-rate claim. Indexing/retrieval and provider calls remain under the unchanged production deadline; resource errors and timeouts can still occur.

## Process disposition

Pre-execute adversarial and security reviews are clean. Frontend/experience reviews skipped: no UI or event-shape change. Post-implementation adversarial, security and quality reviews are clean after the two mode-preservation corrections below.

Base freshness retains the user-approved local baseline plus the prior intent fix; origin/main is two commits ahead and unrelated dirty edits are preserved without rebase or stash. Global spec lint has the already-recorded invalid Drafting status in the untouched exec-consolidation-waves spec. Project-knowledge enquiry not requested; capture remains unavailable under the previously observed staged_dual_writer migration guard. No parallel implementation work or external publication is requested.

## Review corrections

Adversarial findings about disabled-tools plain-mode preservation and native invalid-JSON wording are resolved: only failed Ollama tool decisions switch to fenced mode, schema rejection with explicitly disabled tools chooses Plain, and bare-JSON correction wording remains mode-specific. Two HTTP regressions cover successful native/plain recovery without fences.

## Final disposition

All acceptance criteria are verified. The strict fence transport and bounded correction are shipped; catalog authority, deadline and explicit output tokens remain unchanged. No unresolved review findings or follow-up implementation work remain in this scope.

## Applied checkout verification

The scoped patch is applied to the original checkout. cargo check --workspace and all 22 enabled intent HTTP tests pass there; git diff --check passes. The shared target initially reused stale runtime artifacts from the isolated baseline; touching the existing runtime module to invalidate its fingerprint rebuilt it successfully without changing source content. All 69 other pre-existing tracked edits match their captured hashes after subtracting only the additive spec-index/workspace registrations. The three applied code/test files are byte-identical to the reviewed worktree. Current provider-z.ai / glm-5.3 / 5000-token settings are preserved.

Tail triage: 87 analyzer lines, 2 router lines and 220 HTTP test lines changed; the scoped diff is below the review-volume threshold. The isolated branch receives a conventional commit; the original checkout retains the user’s existing branch and dirty changes. No external merge or publication is performed.
