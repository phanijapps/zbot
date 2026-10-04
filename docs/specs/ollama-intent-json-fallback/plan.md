# Plan: Ollama fenced JSON intent fallback

- **Spec:** [spec.md](spec.md)
- **Status:** Done

## Approach

Add one output mode and a small fence extractor to the existing intent decision state machine. Switch recognized Ollama decisions to fenced text when tool output is missing/invalid or tools explicitly unsupported. The switch consumes an actual request but leaves one corrective attempt for the text mode. Keep the same contract validator, three-request limit and deadline.

## Design (LLD)

The extractor recognizes standalone triple-backtick json opening and closing lines, allows surrounding prose, rejects all extra/wrong-language/unclosed fences, and parses only the selected block. JSON-looking prose is never extracted. Existing safe error codes gain missing_json_block/invalid_json_block and are admitted to persisted fallback reasons. Correction messages identify the failure class and reinforce exact catalog IDs or empty recommendation arrays. Responses remain untrusted and no tool executes.

## Tasks

### T1: Add and verify Ollama fenced JSON fallback

**Depends on:** none
**Verification mode:** TDD plus manual QA
**Touches:** gateway/gateway-execution/src/middleware/intent/agent.rs, gateway/gateway-execution/src/middleware/intent/router.rs, gateway/gateway-execution/tests/intent_output_tests.rs, docs/specs/ollama-intent-json-fallback/*, docs/specs/README.md, workspace.toml

**Tests:**
- stub: true — intent_output_tests.rs::ollama_missing_submission_falls_back_to_fenced_json (AC1/2/3): public HTTP fixture produces a real validated graph decision after the transport switch; confirmed red before implementation.
- intent_output_tests.rs::ollama_missing_fence_gets_one_specific_correction (AC2/4): no fence -> specific correction -> valid result in three requests.
- intent_output_tests.rs::ollama_fenced_fallback_rejects_ambiguous_or_unsafe_decisions (AC2/3/4): malformed fences/JSON/unknown resources never produce a decision.
- intent_output_tests.rs::ollama_fenced_prompt_keeps_catalog_directives_in_encoded_data (AC6): fallback fence/delimiter/directive canary retains bounded encoded_data and authoritative schema.
- Existing native downgrade, terminal errors, bounded call count and disabled-tools tests (AC4).
- Manual compiled fallback (AC5): controlled first missing submission, then real Ollama text requests, populated catalog, synthetic research task, fixed token setting; assert validator success and record provider/model/token limit, populated catalog counts, request modes/count, elapsed time, validator outcome and reliability limits in docs/specs/ollama-intent-json-fallback/verification.md.

**Approach:**
- Add and run the behavioral red test using the existing HTTP server/decision fixture.
- Extend the existing state machine and reuse strict validation; no new dependencies or retry wrapper.
- Run targeted tests, workspace check, gateway clippy and diff/fmt checks; independent adversarial, security and quality reviews; apply only scoped edits to the original checkout.

**Done when:** AC1–AC6 verified and warranted reviews clean.

## Constraints and risks

A valid fence can contain invalid recommendations; preserve catalog validation. Deadline exhaustion can still occur. A successful synthetic exercise proves the compiled fallback works, not a reliability rate. The user did not authorize settings changes. Expected review tail below 500 behavior/test lines.

## Declined additions

- Automatic resource-ID sanitization: separate semantic behavior, not requested by this fallback.
- Unlimited retries, larger token defaults or deadline changes: preserve explicit settings and bounded preflight.
- A general markdown/parser/provider framework: one fence extractor suffices.

## Resolve-vs-surface disposition

Open: resolve all scoped review findings; record the pre-existing freshness/spec-lint/knowledge migration limitations. Preserve unrelated edits and deliver a scoped local patch.

## Changelog

- 2026-10-03: User-requested Ollama fenced JSON fallback, following observed missing submission, unknown resource and deadline failures.
