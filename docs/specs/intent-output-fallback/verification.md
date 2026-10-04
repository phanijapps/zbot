# Verification: provider-aware intent output

Verified 2026-10-02 against the user-authorized HEAD baseline in an isolated worktree. Explicit provider/model/credential settings were not edited by the implementation.

- `cargo check --workspace`: passed.
- `cargo clippy -p agent-runtime -p gateway-execution -p gateway-services --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Complete agent-runtime suite: 452 unit tests passed, two existing manual/download tests ignored; MCP lifecycle/transport and procedure integrations passed.
- Complete gateway-execution suite: 553 unit tests and all enabled integration suites passed. A legacy planner-context string assertion was updated to verify the JSON advisory field required by AC8.
- Complete gateway-services unit suite: 254 passed after the override-deserialization correction (including all 6 provider service tests).
- Final focused analyzer suite: 14 passed, one explicitly configured live-provider test ignored by default. Bootstrap regression subset: 12 passed. Injection/ward pipeline: 6 and 14 passed.

## Real provider exercises

The compiled `intent_output_tests::live_provider_accepts_built_intent_analyzer` uses the public router/analyzer and only a synthetic travel comparison request. It reads provider credentials into process environment; no credentials, provider bodies, or model text are persisted in this record.

- Ollama Cloud through the configured local OpenAI-compatible endpoint, `glm-5.3-flash:cloud`, 1000 output tokens: validated graph decision, 3.96 seconds. Provider/model/token settings were unchanged.
- Direct z.ai coding endpoint, `glm-5-turbo`, 4096 output tokens: validated graph decision, 37.19 seconds. Repeated against the final inlined schema and provider correction: passed, 19.89 seconds.
- Direct z.ai, 1000 output tokens: bounded labeled fallback (`invalid_json`), 37.27 seconds. The explicit token limit therefore remains a relevant configuration choice when switching providers.

## Compatibility finding

The initial generated schema contained object references. A synthetic diagnostic observed Ollama's tool arguments containing strings for `ward_recommendation` and `execution_strategy`, consistently failing the Rust type boundary. Inlining the finite generated contract made those properties explicit objects and the unchanged 1000-token live exercise passed. A regression asserts that the on-wire tool schema contains those objects and no `$ref` keywords.

One successful synthetic request establishes integration feasibility, not a reliability rate. Retrieval, requests, one correction and capability downgrades remain bounded by one outer 45-second deadline and three outgoing completion calls.

## Scope and review

Original dirty files were preserved. Base freshness warned that origin/main would remove existing local intent fixes; this work retains the approved current HEAD and records that limitation instead of rebasing unrelated changes. No UI/event shape, database migration, or executor authorization changes.

Frontend/experience review skipped: no frontend or visual changes; existing event shape retained. Adversarial, security and quality review findings are resolved before completion.

## Knowledge capture

The project-knowledge public capture seam refused this observation with `staged_dual_writer`. No fallback journal or migration was created; the evidence remains in this verification record. Resolving the pre-existing knowledge migration is outside the intent fix.

## Review corrections

Adversarial review found that token-only model overrides inherited `tools=false` from generic registry defaults. Provider-override deserialization now defaults absent tool metadata to allowed while preserving explicit false; registry metadata defaults remain unchanged. `token_only_model_override_keeps_cloud_tool_submission` was confirmed red before the fix and passes through the real HTTP boundary afterwards. The shipped spec-index row also retains the existing two-column table shape.

Repository-wide spec status lint has a pre-existing failure: unchanged `docs/specs/exec-consolidation-waves/spec.md` declares `Drafting`. The new spec uses the valid `Shipped` vocabulary, checked acceptance criteria and resolving local links.

## Final disposition

All implementation findings were resolved: missing tool metadata no longer disables tool submission, the spec index retains its table format, and the completed plan is marked Done. Adversarial, security and quality reviews are clean; quality review confirmed the corrected plan-status nit. No behavior changes were deferred. The pre-existing base/spec-lint/knowledge-capture limitations above remain outside this fix.

Delivery is a scoped local patch to the existing working checkout; unrelated in-progress edits are retained, and no external publication or merge is requested.

## Applied checkout verification

The scoped patch was applied to `/home/videogamer/projects/agentzero`. A hash check preserved all 58 pre-existing modified-file contents (subtracting only this task's additive index/workspace entries). `cargo check --workspace`, `cargo test -p gateway-execution --test intent_output_tests` (14 passed, one opt-in live test ignored), and `git diff --check` passed in the combined working checkout. Explicit provider, model and token settings remain unchanged.

Tail triage: 1809 reviewable behavior/test lines, below the 2000-line threshold. Delivery is local; no external PR or merge was requested.
