# Spec: Provider-aware intent output

- **Status:** Shipped
- **Owner:** AgentZero maintainers
- **Plan:** [plan.md](plan.md)
- **Constrained by:** [RFC 0008](../../rfc/0008-simplified-provider-model-configuration.md)
- **Contract:** existing IntentAnalysis and intent gateway events
- **Shape:** integration
- **Mode:** full (model output, deserialization, provider I/O)

## Objective

Root sessions receive a validated intent decision across native schema providers, direct z.ai JSON mode, Ollama Cloud tool calling, and legacy JSON providers. Analysis failures remain explicit and never manufacture ward creation or resource grants.

## Boundaries

### Always do

- Preserve the IntentAnalysis shape, greeting/procedure bypasses, Quick Chat fast path, and existing capability authorization.
- Reuse runtime schema encoding, host resource catalogs, and configured provider/model selection. Custom prompts influence the rubric but never remove the output contract.
- Keep catalog lookup, all requests, capability fallbacks, and one corrective attempt within the existing 45-second budget and at most three LLM requests.

### Ask first

- Change provider credentials, live model selection, or the user's explicit output token setting.
- Change delegation policy, public UI/event shapes, or unrelated ongoing work.

### Never do

- Execute model-submitted intent as a tool effect, trust retrieved descriptions as instructions, or grant capabilities outside the host catalog.
- Retry auth, retirement, network, or rate-limit errors as unsupported-format errors; log raw prompts, provider bodies, or model output on failure.
- Convert a failed analysis into a recommendation to create an invented general ward.

## Testing Strategy

TDD: exercise the real analyzer with scripted HTTP responses and a fake fact store; verify decisions and bounded failure outcomes, plus runtime request encoding for z.ai/native schema. Integration: existing intent injection and session replay tests, bootstrap regression tests, gateway execution suite. Manual QA: run the built analyzer through the same public entry point with a synthetic request against the configured Ollama Cloud model; direct z.ai requires a configured credential and is otherwise covered by HTTP contract tests. Static: formatting, workspace check, focused clippy.

## Acceptance Criteria

- [x] AC1: Native schema output, direct z.ai json_object mode, submit_intent arguments (including empty response text), and legacy JSON share deserialization and validation.
- [x] AC2: Known Ollama Cloud uses tool arguments directly when tools are allowed; explicitly disabled tools use validated plain JSON; unsupported output formats downgrade only on explicit capability errors. No arbitrary provider ID controls z.ai mode; parsed endpoint host does.
- [x] AC3: A small host-sourced resource catalog is supplied without the broken memory-tool loop. Unknown resource recommendations, invalid enum/complexity, empty intent, and unsafe ward names are rejected before routing.
- [x] AC4: The vault prompt override is honored while transport instructions and the generated schema remain authoritative.
- [x] AC5: Use the raw client without nested retry wrappers. At most three actual outgoing requests, at most one corrective request, and one 45-second deadline include resource lookup. Failure reason and request mode are observable without sensitive response content.
- [x] AC6: Failed analysis is persisted as labeled fallback, preserves an existing configured ward or uses scratch, and cannot trigger automatic ward creation. Quick Chat fallback remains distinguishable.
- [x] AC7: Explicit output token limits and provider settings remain unchanged; omitted intent output limits use 4096. Existing greeting/procedure and injection/replay behavior remains compatible.

- [x] AC8: Model and catalog free text is bounded, control characters removed, JSON encoded and labeled untrusted when rendered into root/planner guidance. Host action and capability authorization remain authoritative; delimiter/directive canaries cannot break the serialized advisory data.

## Assumptions

- Product/process: user approved the proposed provider-aware fix with “go for it” in this conversation (2026-10-02); approval covers this scoped implementation and its verification.
- Technical: LlmClient::chat_with_schema and generated IntentAnalysis schemas exist in this repository.
- Technical: z.ai documents json_object plus host validation: https://docs.z.ai/guides/capabilities/struct-output . Ollama Cloud excludes native structured outputs: https://docs.ollama.com/capabilities/structured-outputs .

Intent preflight exception to RFC0008: omitted intent-specific output limits default to 4096 to bound this decision request; explicit intent limits remain authoritative. Other model slots and runtime defaults follow the RFC unchanged.

## Deferred

None.
