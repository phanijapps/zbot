# Plan: OpenAI Prompt Cache Contract

- **Spec:** [`spec.md`](spec.md)
- **Status:** Done <!-- Drafting | Executing | Done -->

> **Plan contract:** this is the implementation strategy. Unlike the spec, this
> document is allowed to change as you learn. When it changes substantially
> (a different approach, not just a re-ordering), note why in the changelog
> at the bottom.

## Approach

This is a regression-hardening change around the existing OpenAI-compatible
Layer 0 implementation. The implementation should keep request behavior
unchanged, add focused tests around cache-stable request construction and
cache telemetry, and update the compaction-strategy future-state document so
Layer 0 is no longer treated as unimplemented for OpenAI-compatible providers.
The riskiest part is over-specifying JSON shape in a way that blocks legitimate
provider-compatible changes, so tests should assert cache-critical invariants
instead of every incidental field.

## Constraints

- Follow the spec boundary: OpenAI-compatible automatic prompt caching only;
do not add Anthropic-style `cache_control`.
- Keep all behavior changes inside `runtime/agent-runtime/src/llm/openai.rs`
unless a failing test proves another file is part of the request-construction
contract.
- Treat `memory-bank/future-state/compaction-strategy.md` as the status
document to reconcile after tests define the contract.

## Construction tests

**Integration tests:** none beyond `agent-runtime` unit tests; no live provider
should be required.

**Manual verification:** none.

## Tasks

### T1: Request-body cache invariants are covered

**Depends on:** none

**Touches:** `runtime/agent-runtime/src/llm/openai.rs`

**Tests:**
- Add or strengthen tests for Acceptance Criteria 1-4: identical serialized
JSON for identical inputs, no `cache_control`, preserved message/tool order,
and no volatile model-visible fields.

**Approach:**
- Reuse the existing `fixture_messages`, `fixture_tools`, and `test_client`
helpers where possible.
- Assert invariants against the `serde_json::Value` and serialized bytes rather
than copying the whole request body into a brittle golden string.

**Done when:** the new request-body invariant tests fail before a violating
change and pass on current behavior.

### T2: Streaming request construction stays tied to the base builder

**Depends on:** T1

**Touches:** `runtime/agent-runtime/src/llm/openai.rs`

**Tests:**
- Cover Acceptance Criterion 5: streaming adds only streaming transport fields
after the base request body is built.

**Approach:**
- Extract a small test-only helper if needed to compare the base body with a
streaming body after removing `stream` and `stream_options`.
- Avoid network calls; this must stay a pure unit test.

**Done when:** a future change that builds streaming requests through a
different payload path fails a unit test.

### T3: Cache telemetry parsing is locked down

**Depends on:** none

**Touches:** `runtime/agent-runtime/src/llm/openai.rs`,
`runtime/agent-runtime/src/llm/client.rs`

**Tests:**
- Cover Acceptance Criterion 6: OpenAI nested shape, compatible flat shape,
unreported cache info, and both-fields precedence.

**Approach:**
- Keep `TokenUsage.cached_prompt_tokens` as the public runtime field.
- Expand existing tests only if they do not already cover the full acceptance
set.

**Done when:** all supported usage shapes are covered by pure JSON fixture
tests.

### T4: Layer 0 status is reconciled in the compaction strategy

**Depends on:** T1-T3

**Touches:** `memory-bank/future-state/compaction-strategy.md`

**Tests:**
- Goal-based check for Acceptance Criterion 7:
`rg -n "Layer 0|OpenAI-compatible|cache_control|implemented" memory-bank/future-state/compaction-strategy.md`.

**Approach:**
- Update the document to distinguish OpenAI-compatible automatic prompt caching
from Anthropic explicit cache breakpoints.
- Mark Layer 0 as implemented for the OpenAI-compatible path and keep explicit
breakpoints out of scope unless a future provider-specific spec reopens them.

**Done when:** the future-state document no longer implies Layer 0 is wholly
unimplemented for the user's provider path.

### T5: Agent-runtime gate passes

**Depends on:** T1-T4

**Touches:** none

**Tests:**
- Run `cargo test -p agent-runtime openai` for Acceptance Criterion 8.

**Approach:**
- Fix only regressions introduced by this spec's tests or documentation.
- If unrelated failures appear, record them in the implementation summary
instead of broadening the diff.

**Done when:** the targeted `agent-runtime` OpenAI tests pass or unrelated
pre-existing failures are documented.

## Rollout

No feature flag and no migration. The expected runtime behavior is unchanged:
this work only turns the existing Layer 0 behavior into a tested contract and
updates documentation status.

## Risks

- Tests that assert too much incidental JSON shape could make legitimate
OpenAI-compatible request evolution harder. Keep assertions focused on
cache-relevant invariants.
- `docs/` is ignored by git in this repo, so spec files may require force-add
or relocation if this needs to be reviewed through normal git workflows.
- Live provider cache hit rates can still vary by provider thresholds and TTL;
this spec guarantees cache-compatible requests, not a specific provider-side
hit percentage.

## Changelog

- 2026-05-31: initial plan.
- 2026-05-31: implemented request-body invariant tests, streaming body helper,
compaction-strategy status update, and targeted `agent-runtime` test gate.
