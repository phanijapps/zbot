# Spec: Rig intent preflight recovery

- **Status:** Shipped (2026-10-03)
- **Owner:** AgentZero maintainers
- **Plan:** [plan.md](plan.md)
- **Constrained by:** [RFC 0008](../../rfc/0008-simplified-provider-model-configuration.md)
- **Contract:** existing IntentAnalysis and provider settings
- **Shape:** integration
- **Mode:** full (provider I/O and untrusted response parsing)

## Objective

Restore a real Rig Agent single completion for intent analysis and bound GLM-5.3 reasoning on direct z.ai and Ollama so preflight is suitable for the existing deadline. Preserve the working Ollama JSON-block fallback and host validation.

## Evidence

The current-affairs session's first intent request received no completion before the shared 45-second deadline; no decision parser or fence failure preceded it. The earlier rewrite replaced the main-branch Rig path with raw client calls. GLM-5.3 defaults to max reasoning when reasoning_effort is omitted: https://docs.z.ai/guides/llm/glm-5.3 . The previous proxy smoke changed the parsed endpoint host, so it exercised a different response_format; exact-input validation must call the configured endpoint directly. Timeout is observed; max reasoning is a supported contributing hypothesis, not a proven exclusive cause. The exact-input Ollama fenced fallback also exhausted 5000 output tokens with no content (finish_reason length); diagnostic injection of supported low effort yielded a validated decision in 4.76 seconds including one correction. Separate research agents incurred large cumulative input use and rate-limit/offload errors; no claim is made that intent recovery alone cures every research cost problem.

## Boundaries

### Always do

- Reuse LlmCompletionClient, the pinned Rig Agent completion API, existing catalog/validator/fence parser and OpenAiClient transport.
- Each host attempt performs exactly one completion, with no Rig tool execution loop or hidden retries; retain three requests and 45 seconds.
- Preserve terminal provider failure handling, malformed argument rejection, native JSON and Ollama modes, and the 5000-token user setting.

### Ask first

- Change selected provider/model/credentials, output-token limits, deadline, delegation policy or resource authority.

### Never do

- Add a transport/provider framework, generic arbitrary provider-parameter forwarding, executable intent tools, or another retry layer.
- Log secrets, raw prompts or model responses; copy or revert unrelated dirty edits.

## Acceptance Criteria

- [x] AC1: Every actual intent provider request passes through one real Rig Agent completion; submitted tools remain data and cannot execute.
- [x] AC2: GLM-5.3 intent requests on direct z.ai and Ollama explicitly carry reasoning_effort low with the configured output limit. Other providers/models are unchanged. Only reasoning_effort is added from the existing provider_params configuration.
- [x] AC3: Capability errors and invalid tool arguments retain their typed host classifications through Rig, with no extra requests; all existing intent HTTP boundary tests still pass. Preserved errors expose stable safe status/codes only; provider-body, prompt, model-output and credential canaries never reach logs or returned diagnostics.
- [x] AC4: The exact current-affairs input is exercised against the real configured z.ai endpoint with vault rubric and populated catalog; a validated graph decision and measured latency are required to ship; any unresolved live failure keeps this spec open. A compiled real Ollama fallback is verified too.
- [x] AC5: Gates and independent reviews pass; scoped patch applied with unrelated edits and stopped session preserved.

## Testing Strategy

TDD: HTTP regression for low effort and explicit tokens must fail before production changes. Runtime Rig single-completion/error tests verify the actual SDK path; existing 22 HTTP regressions cover output, rejection and request budgets. Manual QA calls configured endpoints directly with exact input and rubric, preserving keys in process memory. Workspace check, scoped clippy, formatter and diff checks precede reviews.

## Assumptions

- User explicitly authorized fixing the regression and required Rig on 2026-10-03; no repeat approval is required.
- Pinned Rig Agent::completion().send() has no tool loop and calls the model once (local pinned source inspected).
- The existing 45-second deadline and explicit 5000-token setting remain authoritative.
- Baseline is the reviewed prior fix; origin/main is two commits ahead, and rebasing would alter unrelated ongoing work. Preserve the local baseline and compare its intent implementation read-only.

## Deferred

Separate research search pacing and offload-result recovery require their own evidence-scoped fix if still reproducible after intent routing recovery.
