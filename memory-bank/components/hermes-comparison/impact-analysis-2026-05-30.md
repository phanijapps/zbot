# Hermes Comparison Impact Analysis - 2026-05-30

Current code-backed read of the Hermes delta documents:

- Historical report: [`deltas.md`](./deltas.md)
- Counter-assessment: [`deltas-rebuttal.md`](./deltas-rebuttal.md)
- Earlier future-state tracker: [`../../future-state/2026-05-22-hermes-comparison-gaps.md`](../../future-state/2026-05-22-hermes-comparison-gaps.md)

This analysis used Codemem impact/query/memory search, then direct working-tree
verification. A later Codemem refresh indexed 1,143 files with 0 parse errors
and populated `source_references`, which made the Hermes comparison searchable
as current working-tree context rather than only historical memory.

## Executive Take

The raw delta report is useful as a capability checklist, but several priorities
are stale or over-framed. The rebuttal is closer to current code reality:

- z-Bot already has a stronger self-improvement substrate than the original
  report credits: learned procedures are mined, dispatchable, and measured.
- The highest product gap is distribution and first-success documentation, not
  another memory-loop feature.
- The most actionable engineering gaps are one reference bridge-worker adapter,
  credential pooling/failover, raw session transcript search, and multimodal-out.
- A refreshed Codemem pass also confirms several secondary gaps from the
  future-state tracker: durable Kanban-style work queue, stricter subagent role
  gating, first-class clarify flow, plugin lifecycle hooks, and residual
  per-task model routing beyond curator/intent-analysis.
- Browser/computer-use, MoA, IDE integration, and i18n are real but lower
  priority unless they become explicit product commitments.

## Current Priority Matrix

| Priority | Area | Current assessment | Primary impact |
|---|---|---|---|
| P0 | Release packaging + first-run docs | Real adoption blocker. Existing installer builds from source; release workflow appears stale against current binary names. | New users cannot reliably install and reach first success. |
| P1 | Reference platform adapter | Real gap, but not 22 separate gaps. Bridge worker protocol exists; need one production adapter and template. | Proves "agent everywhere" integration story. |
| P1 | Credential pooling/failover | Real reliability gap. Rate limiting/retry exists, key rotation does not. | Prevents long sessions failing when one key is exhausted. |
| P1 | Raw session search | Real UX gap. Memory search is strong, but transcript FTS/search_sessions is missing. | Lets users and agents answer "what did we discuss?" from original text. |
| P1 | Multimodal-out | Real gap. Vision analysis exists; TTS/STT/image generation do not. | Enables voice/output-media workflows. |
| P2 | Durable work queue | Real long-running-work gap. z-Bot has cron, session state, delegation continuation, and procedures, but not a Kanban-style durable dispatcher. | Supports work that spans sessions/agents with failure limits and resumption. |
| P2 | Browser automation | Real first-class tool gap. Skills mention Lightpanda/CDP, but runtime has no core browser tool. | Useful for web tasks, but can be bridged externally first. |
| P2 | Remote/serverless execution | Real deployment gap. Shell is local-only. | Matters for VPS/GPU/isolated worker deployments. |
| P2 | Subagent role gating | Partially present as injected role rules; not strict tool-call capability enforcement. | Prevents leaf/reviewer agents from taking unsafe or out-of-role actions. |
| P2 | Per-task model routing residual | Partially implemented for curator and intent analysis; other side tasks still need routing cleanup. | Cost/reliability tuning for auxiliary LLM work. |
| P3 | create_skill tool | Tactical gap only. Procedure mining/dispatch covers the self-improvement loop. | Nice for agent-authored markdown skills, not foundational. |
| P3 | Clarify tool | `request_input` exists as UI tooling, but no canonical `clarify` workflow/tool contract. | Cleaner mid-task user questions and resumable answers. |
| P3 | Plugin lifecycle hooks | Hook registry handles inbound/response routing, not broad pre/post LLM/tool/session plugin hooks. | Needed for extension authors who want middleware-style behavior. |
| P3 | MoA | Specific feature gap. Orchestration/delegation is wired; parallel multi-model synthesis is not. | Quality differentiator, not baseline orchestration. |
| P3 | Computer use / IDE / i18n | Real but niche or post-adoption. | Useful after install/integration basics are solid. |

## Code-Backed Notes

### Self-Improvement Is Mostly Closed

The original report claims z-Bot has no self-improving loop. Current code does
not support that framing.

- `RunProcedureTool` loads a learned procedure, validates each step against the
  live tool registry, dispatches steps, and records success/failure telemetry.
  See `runtime/agent-runtime/src/tools/run_procedure.rs`.
- Procedure rows track `success_count`, `failure_count`, `avg_duration_ms`,
  `avg_token_cost`, and `last_used`. See
  `stores/zero-stores-domain/src/procedure.rs`.
- The SQLite repository updates procedure success/failure counters and dedupes
  duplicate procedures by highest success count. See
  `stores/zero-stores-sqlite/src/procedure_repository.rs`.
- `PatternExtractor` mines successful session pairs into dispatchable
  `PatternStep` procedures with a live tool whitelist. See
  `gateway/gateway-memory/src/sleep/pattern_extractor.rs`.

Remaining work is narrower:

- Optional `create_skill` runtime tool for agent-authored markdown skills.
- Inner-step procedure events if procedure observability becomes important.
- A planner-spec-to-procedure promotion hook, already noted as future work in
  the run-procedure implementation plan.

### Platform Adapters Should Use Bridge Workers First

`gateway-connectors` still stubs WebSocket/gRPC/IPC outbound dispatch, but the
newer bridge-worker path is a better substrate for platform adapters.

- `gateway/gateway-connectors/src/dispatch.rs` only implements HTTP/CLI and
  returns `UnsupportedTransport` for WebSocket/gRPC/IPC.
- `gateway/gateway-bridge/src/protocol.rs` already models inbound messages,
  worker capabilities, resources, ACK/fail delivery, and outbox pushes.
- `workers/echo_worker/README.md` documents a reference WebSocket worker that
  self-registers and receives capability invocations.

Impact: build one real adapter as a bridge worker before expanding connector
transports. Slack or Discord is a better proof than implementing many platform
adapters up front.

### Distribution Has a Concrete Release Blocker

The historical report calls install/docs P2. The current product impact says P0.
There is also a concrete mismatch to fix.

- Current binaries are `zbotd` and `zbot` in `apps/daemon/Cargo.toml` and
  `apps/cli/Cargo.toml`.
- `.github/workflows/release.yml` still packages `zerod` and `zero` in multiple
  jobs.
- `README.md` documents `./scripts/install.sh`, but that is still a source-build
  installer requiring Rust/Node prerequisites.
- Docker support exists under `docker/`, but it also builds locally rather than
  pulling a published image.

Impact: fix release artifact names, publish prebuilt archives/images, then write
a first-success guide. This closes more user-facing gap than most feature work.

### Durable Work Queue Is Still Distinct From Delegation

z-Bot has useful queue-like pieces, but they do not equal Hermes's durable
Kanban plugin.

- `DelegationDispatcher` is a long-lived per-session queue for spawning
  subagents and enforcing per-session/global concurrency.
- `execution-state` tracks queued/running sessions, pending delegations, and
  continuation flags.
- `gateway-cron` persists scheduled jobs.

Impact: a real parity feature would add a durable work-board model with task
state, assignment, failure limits, retry policy, multi-worker dispatch, and UI
visibility. This should not be modeled as another `delegate_to_agent` variant.

### Session Search Is Not the Same as Memory Recall

The rebuttal is right that z-Bot has a stronger distilled-memory surface than
raw FTS. That does not eliminate the transcript-search gap.

- `/api/memory/search` searches memory facts/procedures/wiki/episodes.
- `messages` are stored in `conversations.db`, but there is no `messages_fts`
  table.
- `GET /api/conversations` is currently stubbed to return an empty list.

Impact: add message FTS and a `search_sessions` tool/API. Recall middleware can
continue using distilled memory, while explicit session search gives users raw
conversation provenance.

### Credential Pooling Is Still Missing

Provider reliability is partially handled:

- `RetryingLlmClient` retries transient errors and 429-like responses.
- `RateLimitedLlmClient` gates calls through a shared provider limiter.
- Provider config has one `apiKey`, not a key pool.

Impact: introduce provider key pools, exhaustion tracking, rotation on 429/auth
quota failures, and a dashboard health surface. This should live near
`gateway-services` provider config and runtime client construction.

### Role Gating Exists As Prompt Rules, Not Hard Capability Bounds

Codemem confirmed the older gap is still mostly accurate.

- `SubagentRole::Executor` and `SubagentRole::Reviewer` are detected from task
  text and injected into specialist instructions.
- The `delegate_to_agent` tool is still a runtime tool surface; strict
  "leaf cannot re-delegate / reviewer cannot execute" restrictions are not a
  capability-level policy boundary.

Impact: if this becomes a priority, enforce tool availability by role at
executor construction time, then add tests that reviewers cannot call shell or
delegate and leaf workers cannot re-delegate.

### Clarify Exists Indirectly, Not As A Workflow Contract

`request_input` and `show_content` exist as UI tools when enabled, but there is
no canonical `clarify` tool with a typed contract for mid-task questions,
pending-answer state, or resume semantics.

Impact: a small `clarify` wrapper around the UI/input mechanism could standardize
when agents ask the user instead of improvising a conversational pause.

### Plugin Hooks Are Narrower Than Hermes

z-Bot has a hook abstraction, plugin service/config endpoints, and bridge
workers, but Codemem did not find a broad lifecycle hook surface equivalent to
Hermes's `pre_tool_call`, `post_tool_call`, `pre_llm_call`, `post_llm_call`,
`on_session_start`, and `on_session_end`.

Impact: add lifecycle middleware only after the extension story is stable. The
current bridge-worker path is enough for adapters; lifecycle hooks are for
plugins that need to observe or modify agent execution.

### Per-Task Model Routing Is Partially Closed

The future-state tracker says curator and intent-analysis routing are wired, but
other side tasks still inherit broader routing.

- `ExecutionSettings` has orchestrator, distillation, curator, intent-analysis,
  and multimodal config surfaces.
- Follow-up work remains for sleep-time stages and other `MemoryLlmFactory`
  callers that should route independently.

Impact: finish this as a cost/reliability cleanup, not a headline Hermes parity
item.

### Orchestration Exists; MoA Does Not

The raw report conflates orchestration with Mixture-of-Agents.

- Intent analysis returns strategy, graph, and recommended agents.
- `delegate_to_agent` is an active tool path.
- `DelegationDispatcher` runs child agents with per-session queueing and global
  concurrency control.

Impact: do not plan "wire the orchestrator" as if nothing exists. Plan MoA as a
separate ensemble feature: parallel model calls plus aggregator synthesis.

## Verification Guidance

For any implementation that follows this analysis:

- Rust changes: run `cargo fmt --all`, `cargo check --workspace`, then targeted
  crate tests or `cargo test --workspace` depending on blast radius.
- UI/settings changes: run `npm run build` from `apps/ui/`.
- Release/docs changes: verify binary names in release artifacts match
  `zbotd`/`zbot`, and validate install instructions from a clean checkout or
  clean container.

## Codemem Assessment

Codemem helped more after the update:

- `codemem index --no-embeddings` completed successfully after the update
  (`files_indexed: 1143`, `parse_errors: 0`).
- `codemem status` now reports populated `source_references` instead of zero,
  so local evidence is much more usable.
- It quickly connected Hermes deltas to code areas: procedures, bridge workers,
  provider config, retry/rate limiting, memory search, delegation, hooks,
  session state, and model routing.
- Typed memory found older plans and publishing docs that raw grep would not
  have made obvious, especially the `zbotd`/`zbot` rename evidence.
- It surfaced underweighted gaps from the earlier future-state tracker:
  durable Kanban queue, role gating, clarify, plugin hooks, and residual
  per-task routing.

Remaining limits:

- `impact` still returned empty `reasons` and no direct verification commands
  for these doc-centered targets.
- Embeddings were not regenerated in the refresh (`--no-embeddings`), so
  semantic recall quality still depends on existing memories rather than fresh
  vector chunks.
- Code claims still needed verification with `rg`, `sed`, and exact files.

Net: Codemem is now strong enough for first-pass gap discovery and
cross-linking. I still treat exact code behavior and priority calls as requiring
direct file verification.
