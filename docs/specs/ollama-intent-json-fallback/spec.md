# Spec: Ollama fenced JSON intent fallback

- **Status:** Shipped
- **Owner:** AgentZero maintainers
- **Plan:** [plan.md](plan.md)
- **Constrained by:** [RFC 0008](../../rfc/0008-simplified-provider-model-configuration.md)
- **Contract:** existing IntentAnalysis and gateway events
- **Shape:** integration
- **Mode:** full (provider I/O and untrusted response parsing)

## Objective

Ollama intent analysis can recover from an unavailable or rejected tool decision by requesting one fenced JSON block, checking that the block is complete and unambiguous, then validating its contents before routing.

## Boundaries

### Always do

- Reuse the existing host catalog, generated contract, strict decision validator, raw client and labeled failure outcome.
- Keep at most three actual requests, one correction in the fenced mode, and the existing shared 45-second deadline.
- Preserve explicit provider, model and token settings, native/z.ai paths, and valid tool decisions.

### Ask first

- Change resource authorization, discard invalid resource IDs automatically, alter deadline or output-token settings, or modify public event/UI shapes.

### Never do

- Execute submitted tools, use arbitrary prose/bracket recovery, accept ambiguous code blocks, or treat a code fence alone as a valid decision.
- Retry auth, retirement, rate-limit or network errors as formatting failures; log raw model text or credentials.

## Acceptance Criteria

- [x] AC1: A rejected/missing Ollama tool decision or explicit unsupported tool mode switches to a request without tools or response_format that asks for exactly one fenced json block. Valid tool output still succeeds in one request.
- [x] AC2: Only one complete triple-backtick json block is extracted; missing, unclosed, wrong-language or multiple blocks are rejected. Surrounding prose is ignored. JSON-looking prose and tool calls are not accepted as the fenced response.
- [x] AC3: Extracted JSON passes the existing shape, resource, capability and ward validator. Invalid JSON/resources cause a specific corrective prompt; invalid decisions never reach routing.
- [x] AC4: The tool-to-text switch and one fenced correction share the three-request cap and deadline. Exhaustion remains labeled; terminal provider errors stay terminal. Native/z.ai and explicitly disabled-tools plain JSON behavior is preserved.
- [x] AC5: HTTP regressions exercise the public analyzer, and a real compiled fallback with a populated catalog is exercised against the configured Ollama model using a synthetic request. Results and reliability limits are recorded.

- [x] AC6: Fenced prompts reuse the authoritative output contract/generated schema and existing bounded encoded_data for request/catalog text; delimiter/directive/fence canaries cannot escape the data block.

## Testing Strategy

TDD through scripted HTTP responses: missing submission then valid fenced result; missing block then correction; malformed and ambiguous blocks; invalid recommendations; bounded request count; native compatibility. Existing intent HTTP regressions, workspace type check, gateway clippy and formatter checks. Manual QA uses the compiled analyzer, real configured provider credentials in environment, and a populated synthetic catalog. A controlled first missing-submission response exercises the real fallback without executing user work.

## Assumptions

- User instruction explicitly requests JSON-code-block fallback for Ollama; prior approval authorizes implementing the intent reliability fix.
- The 5000-token explicit setting remains unchanged; no output format guarantees semantic correctness or provider latency.
- Engram's retrieved intent body is stale; direct current source confirms the decision/validator/HTTP fixture reuse.
- Approved baseline includes the previous local intent fix; origin/main freshness warning and unrelated dirty edits are preserved without rebase/stash.

## Deferred

None.
