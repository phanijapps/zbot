# Plan: Provider-aware intent output

- **Spec:** [spec.md](spec.md)
- **Status:** Done

## Approach

Replace the unconstrained search/tool loop with bounded host retrieval and one decision request. Generate the transport schema from IntentAnalysis. Existing OpenAiClient schema encoding uses native json_schema, with json_object for exact z.ai origins. The gateway selects tool submission directly for known Ollama Cloud, probes native output elsewhere, and downgrades only on explicit unsupported-format/tool errors. Parse and validate every output before applying it; one correction and a three-request ceiling share the existing deadline.

## Constraints

Explicit RFC0008 exception: this bounded system preflight uses 4096 only when the intent-specific limit is omitted; user-provided limits retain priority. This exception is scoped to intent output and was part of the approved reliability proposal.


No new dependencies, credentials, UI controls, or event contract. Keep Rig inside runtime; the intent decision request uses the existing neutral LlmClient directly. Preserve unrelated dirty files in the user's checkout.

## Construction tests

HTTP boundary tests verify all transport paths and error behavior; bootstrap tests verify fallback ward safety and prompt wiring. A live synthetic model smoke uses the built analyzer, not a hand-written HTTP-only surrogate.

## Design (LLD)

### Interfaces and data

IntentAgentDeps carries the host capability catalog. The catalog uses existing skill/MCP/agent service IDs plus procedure names; resource descriptions are bounded untrusted data. Ward names remain server-authoritative. A typed analyzer error has safe stable failure codes; router fallback includes that reason without raw output. The current public event shape stays unchanged.

### State and failure handling

Select initial mode by endpoint/model and explicit tool support. Requests exposing submit_intent never execute that tool. Native or legacy output and tool arguments use the same validator. Capability rejection advances the mode; invalid output permits one corrective request. Other provider errors terminate. Use raw OpenAiClient, with no nested RetryingLlmClient; maximum three actual outgoing requests and one deadline cover the full operation. Failure is determined before Quick Chat explanation rewriting and ward reconciliation.

### Advisory data safety

Encode bounded model/catalog strings as JSON data with an explicit untrusted-data notice. Remove control characters and escape framing delimiters. Keep deterministic action/delegation instructions and capability grants outside that data. Tool arguments use JSON string encoding, never raw interpolation. Verify delimiter/directive canaries and existing injection behavior.

### Observability

Log mode, attempt, safe validation/failure code, usage counts, and provider finish reason. Do not persist raw output in errors. Fallback metadata carries a user-readable degraded-analysis reason.

## Tasks

### T1: Implement bounded provider-aware intent decisions

**Depends on:** none
**Verification mode:** TDD plus manual QA
**Touches:** gateway/gateway-execution/src/middleware/intent/*, gateway/gateway-execution/src/runner/invoke_bootstrap.rs, gateway/gateway-execution/tests/intent*, runtime/agent-runtime/src/llm/openai.rs, runtime/agent-runtime/src/rig_adapter.rs, docs/specs/intent-output-fallback/*

**Tests:**
- stub: true — gateway/gateway-execution/tests/intent_output_tests.rs::cloud_tool_arguments_are_the_decision_even_when_text_is_empty (AC1/2), confirmed red on current analyzer.
- stub: true — runtime/agent-runtime/src/llm/openai.rs::tests::direct_zai_schema_request_uses_documented_json_object_mode (AC1/2), compiled and confirmed red.
- stub: true — intent_output_tests.rs::unknown_resource_recommendations_do_not_become_routing_hints (AC3) and ::vault_prompt_override_reaches_the_model_request (AC4); extend HTTP boundary fixture during execution.
- AC2 goal artifact (no stub): intent_output_tests.rs::ollama_cloud_with_tools_disabled_uses_plain_json — validated decision accepted, no tools or response_format payload.
- AC5 goal artifacts (no stub): intent_output_tests.rs::downgrade_and_correction_share_three_request_budget, ::terminal_provider_errors_do_not_retry, ::stalled_retrieval_hits_the_analyzer_deadline — actual request counts and timeout result.
- AC6 goal artifacts (no stub): invoke_bootstrap.rs::tests::failed_intent_keeps_current_ward_or_scratch and ::quick_chat_keeps_fallback_label — no ward creation and visible degraded explanation.
- AC7 goal artifacts (no stub): invoke_bootstrap.rs::tests::intent_token_default_preserves_explicit_override; intent_output_tests.rs::greetings_bypass_provider — defaults and no HTTP call.
- AC8 goal artifact (no stub): intent_analysis_tests.rs::advisory_directives_stay_inside_encoded_data — framing/directive canary encoded, host action stays singular; ::planner_advisory_is_bounded_and_encoded — size/control/framing safeguards.

- Valid native and tool responses produce the same nonempty decision, even with no text in a tool response; malformed/multiple/unexpected calls never execute effects.
- Exact z.ai hosts use JSON-object encoding; native providers retain JSON-schema encoding; known Ollama Cloud starts with tool submission.
- Unsupported-mode responses downgrade; 401/403/410/429, unrelated 400, and network errors do not.
- Invalid output gets at most one correction; provider downgrade/correction together never exceed three requests; stalled retrieval and calls share the deadline.
- Override prompt reaches the real request; supplied host resource IDs are accepted and unknown/unsafe recommendations rejected.
- Fallback is visibly degraded, uses scratch/current ward without creation, and preserves token overrides and bypass behavior.

**Approach:**
- Add failing behavioral tests against the current analyzer and runtime encoding.
- Reuse schema encoding, recall_facts, catalog metadata, and existing routing; remove the now-orphaned agent_with_tools helper if no references remain.
- Run formatting, workspace type check, focused tests/clippy, independent review, and synthetic built-analyzer smoke.

**Done when:** AC1–AC8 are verified; all warranted reviews clean; only scoped changes are applied to the user's checkout.

## Rollout

Rebuild/restart the daemon through the normal user workflow. Live credentials and settings remain untouched. No database migration; reverting the code restores the previous behavior.

## Risks

Provider behavior varies: native schema support is established by explicit responses, z.ai JSON mode still needs host validation, and tool calling can return invalid arguments. A small relevant catalog can omit a useful resource; recommendations remain hints and the executor retains existing authorization/discovery.

## Declined additions

- A general provider-capability registry and new settings UI: unnecessary for the existing neutral API and conservative fallback.
- Raising the user's explicit 1000-token setting: not authorized as a silent configuration edit.
- New tool-running orchestration or unrestricted memory context: the analyzer only needs bounded resource metadata.

## Resolve-vs-surface disposition

Open: implement the accepted fix and resolve reviewer findings. Base freshness warns that origin/main would remove existing local intent fixes; isolated checkout retains the user-authorized HEAD baseline. No rebase, stash, or unrelated changes. Existing workspace drift is outside this task; register this spec explicitly.

## Changelog

- 2026-10-02: Initial plan, scoped to the user-approved provider-aware intent fix and direct z.ai compatibility.
