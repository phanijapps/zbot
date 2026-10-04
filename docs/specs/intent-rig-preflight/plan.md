# Plan: Rig intent preflight recovery

- **Spec:** [spec.md](spec.md)
- **Status:** Done

## Approach

Add one single-completion helper beside the existing Rig structured helper. Restore typed LlmError across nonstreaming Rig completion, map response text/tool arguments back to existing host response, and use the helper from the current intent loop. Add only allowlisted reasoning_effort serialization to the existing transport and select low effort for GLM-5.3 intent requests on direct z.ai and Ollama. No new state machine or loop.

## Tasks

### T1: Restore Rig and verify exact intent input

**Depends on:** none
**Verification mode:** TDD plus manual QA
**Touches:** runtime/agent-runtime/src/rig_adapter/structured.rs, runtime/agent-runtime/src/rig_adapter/model.rs, runtime/agent-runtime/src/llm/openai.rs, gateway/gateway-execution/src/middleware/intent/agent.rs, gateway/gateway-execution/tests/intent_output_tests.rs, docs/specs/intent-rig-preflight/*, docs/specs/README.md, workspace.toml

**Tests:**
- stub: true — intent_output_tests.rs::zai_intent_uses_low_reasoning_and_preserves_output_limit (AC2): real HTTP request plus validated decision, must be confirmed red for omitted effort before implementation.
- stub: true — intent_output_tests.rs::ollama_glm_intent_uses_low_reasoning_for_fenced_fallback (AC2): low effort and 5000 tokens on both actual HTTP attempts, with validated fenced decision; confirm red before adding the Ollama condition.
- Rig helper single-completion typed-error/tool-response tests (AC1/3): no tool dispatch, lossless arguments, fixed unsupported error through SDK.
- OpenAI request-body test for reasoning_effort allowlist and absence of arbitrary/reserved provider keys (AC2).
- intent_output_tests.rs::rig_provider_error_is_redacted (AC3): injected private error-body canary never appears in public fallback diagnostics or emitted tracing.
- Existing 22 intent HTTP tests (AC1/3), workspace check, scoped clippy, formatter and diff checks.
- Manual QA (AC4): exact saved public current-affairs request, current vault rubric, populated resource catalog, real configured z.ai API endpoint and 5000 output tokens; record JSON mode, validator result and elapsed time without raw prompts/keys. Repeat compiled controlled Ollama fallback; do not start actual delegated user research as the diagnostic.

**Approach:** Red behavioral HTTP assertion first. Implement minimum helper/config forwarding changes, run focused gates then required broader gates. Obtain adversarial, security and quality reviews and apply only scoped edits.

**Done when:** AC1–AC5 verified, all review findings resolved, exact-input provider test succeeds and unrelated edits preserved.

## Constraints and risks

Provider latency remains external; low effort cannot guarantee every call finishes. The prior smoke did not reproduce the real input or endpoint detection. Research token counts are cumulative across many prompts, not one response budget. Expected behavior/test tail below 500 lines.

## Declined additions

- Generic provider parameter forwarding: serialize only the needed reasoning_effort field.
- A Rig execution loop or executable submit tool: preflight only needs one completion per existing attempt.
- Larger deadlines/output limits and delegation redesign: not the evidenced intent fix.

## Resolve-vs-surface disposition

Open: resolve scoped findings; record remaining research cost issues separately, pre-existing freshness/lint/knowledge limitations, and live-probe evidence limits.
