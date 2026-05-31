# Spec: OpenAI Prompt Cache Contract

- **Status:** Shipped <!-- Draft | Approved | Implementing | Shipped | Archived -->
- **Owner:** TBD
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** none

> **Spec contract:** this document defines what "done" means. The implementing
> PR must match this spec, or update it. Verification must be derivable from it.

## Objective

Make Layer 0 of the compaction strategy a permanent compatibility contract
for OpenAI-compatible providers. Identical model-visible request prefixes must
remain byte-stable enough for provider-side automatic prompt caching to work,
cache-hit telemetry must remain parsed when providers report it, and no
Anthropic-only cache-control fields may be introduced into OpenAI-compatible
chat requests. Success means future changes to request construction, streaming,
tool schema wiring, or telemetry cannot silently break prompt-cache behavior
without failing targeted tests.

## Boundaries

The three-tier guard that keeps an implementing agent inside the lines.
*Always do* applies without asking; *Ask first* requires human sign-off
before proceeding; *Never do* is a hard rule, even under time pressure.

### Always do

- Preserve caller-provided message order and tool order in
`OpenAiClient::build_request_body`.
- Keep `chat` and `chat_stream` routed through the same request-body builder so
both paths share the same cache-stability contract.
- Preserve parsing for both known cache telemetry shapes:
`usage.prompt_tokens_details.cached_tokens` and
`usage.prompt_cache_hit_tokens`.
- Keep request-body fixtures focused on model-visible payload shape, not
transport-only implementation details.

### Ask first

- Changing the OpenAI-compatible request payload shape beyond adding regression
tests.
- Reordering system shards, tools, or generated system-context sections.
- Moving dynamic session state earlier in the message list than it is today.
- Replacing OpenAI-compatible automatic caching assumptions with a provider-
specific explicit-cache API.

### Never do

- Never emit Anthropic-specific `cache_control` or equivalent explicit breakpoint
fields from OpenAI-compatible request bodies.
- Never add timestamps, UUIDs, random ordering, debug markers, or other
per-call noise to the model-visible stable prefix.
- Never change response parsing, streaming token emission, tool-call parsing,
multimodal rehydration, retry behavior, or rate limiting as part of this spec.
- Never include Layer 4 durable memory-surface work in this spec.
- Never update golden request fixtures casually; fixture changes must be paired
with an explanation of the model-visible payload change.

## Testing Strategy

Request construction uses **TDD** because cache stability is a compact,
testable invariant: identical semantic inputs must serialize identically, and
forbidden fields must be absent. Telemetry parsing uses **TDD** because the
known provider usage shapes are pure JSON fixtures. Integration wiring uses a
**goal-based check**: the `agent-runtime` test package must pass without
requiring a live provider. No manual QA is required because this spec does not
change UI or live-provider behavior.

## Acceptance Criteria

- [x] `OpenAiClient::build_request_body` has tests proving identical
messages/tools produce identical serialized JSON.
- [x] Request-body tests prove the OpenAI-compatible payload does not contain
`cache_control`.
- [x] Request-body tests prove caller-provided tool order and message order are
preserved.
- [x] Request-body tests prove volatile fields such as generated UUIDs,
timestamps, and debug markers are absent from the model-visible payload.
- [x] Streaming request construction continues to use the same base request
builder as non-streaming, with only streaming transport fields added after the
base body is built.
- [x] Cache telemetry tests cover OpenAI
`usage.prompt_tokens_details.cached_tokens`, compatible-provider
`usage.prompt_cache_hit_tokens`, no-cache-info responses, and both-fields
precedence.
- [x] The compaction strategy document marks Layer 0 as implemented for
OpenAI-compatible automatic prompt caching and explicitly notes that
Anthropic-style explicit breakpoints are out of scope.
- [x] `cargo test -p agent-runtime openai` passes.

## Assumptions

- Technical: Layer 0 targets OpenAI-compatible clients, not Anthropic-native
payloads (source: user confirmation 2026-05-31).
- Technical: `OpenAiClient::build_request_body` is the request-construction
choke point for chat and streaming paths (source:
`runtime/agent-runtime/src/llm/openai.rs`).
- Technical: prompt-cache telemetry already supports OpenAI nested cache tokens
and compatible `prompt_cache_hit_tokens` (source:
`runtime/agent-runtime/src/llm/client.rs`;
`runtime/agent-runtime/src/llm/openai.rs`).
- Technical: byte-stability regression tests already exist for identical
request bodies (source: `runtime/agent-runtime/src/llm/openai.rs`).
- Technical: no runtime/gateway code currently emits `cache_control` (source:
`rg "cache_control" runtime gateway framework services apps stores`, no
matches).
- Product: Layer 4 memory-surface work is explicitly out of scope for this
spec (source: user confirmation 2026-05-31).
- Process: this spec may live under the local `docs/specs/` path prescribed by
the spec skill, even though `docs/` is currently ignored by git (source: user
confirmation 2026-05-31).
- Process: `docs/CONVENTIONS.md`, `docs/CHARTER.md`, and
`docs/specs/README.md` were absent when this spec was drafted, so no local
spec-approval convention constrained the body (source: repository read
2026-05-31).
