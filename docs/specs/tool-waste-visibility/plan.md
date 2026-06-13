# Plan: Tool Waste Visibility

- **Spec:** [`spec.md`](spec.md)
- **Status:** Done

> **Plan contract:** this is the implementation strategy. Unlike the spec, this
> document is allowed to change as you learn. When it changes substantially
> (a different approach, not just a re-ordering), note why in the changelog
> at the bottom.

## Approach

Keep the change narrow and make the existing telemetry honest. First, change
runtime blocked-hook events so they carry an error/reason while preserving the
tool-result message needed by model transcripts. Second, teach gateway logging
to persist blocked-hook results as warnings with structured metadata and
duration when supplied. Third, improve common tool-schema error messages and
planner skill references so agents recover without trial-and-error. Finally,
run focused Rust tests across the touched crates and update the active spec
index.

## Constraints

- No ADR/RFC constraints.
- Keep the existing `execution_logs` schema.
- Do not add a new UI surface, API endpoint, persistence table, service
  boundary, or runtime dependency.
- Preserve provider-compatible tool result messages for blocked calls.

## Construction tests

**Integration tests:** focused runtime/gateway tests proving blocked-hook
events log as warnings and existing tool-result logs still work.
**Manual verification:** none beyond targeted log-shape tests and grep checks
for planner skill references.

## Tasks

### T1: Blocked hook events are warning-grade telemetry

**Depends on:** none

**Touches:** `runtime/agent-runtime/src/executor.rs`,
`gateway/gateway-execution/src/invoke/event_logging.rs`,
`gateway/gateway-execution/src/invoke/stream_event_processor.rs`

**Tests:**
- TDD: runtime executor test proves a blocked tool emits `StreamEvent::ToolResult`
  with a non-empty error/reason while preserving `[blocked by hook]` as the
  tool-result content.
- TDD: gateway event logging test proves `result="[blocked by hook]"` plus
  blocked error metadata persists/logs as warning with structured
  `blocked_by_hook`.

**Approach:**
- Change the blocked-hook runtime `ToolResult` event to include an error string
  such as `blocked_by_hook`.
- Update gateway logging metadata to include `blocked_by_hook: true` when the
  error or result indicates hook blocking.
- Preserve existing handling for ordinary tool failures.

**Done when:** focused tests prove blocked hooks are no longer logged as
successful completions.

### T2: Tool result durations propagate to logs

**Depends on:** T1

**Touches:** `runtime/agent-runtime/src/executor.rs`,
`runtime/agent-runtime/src/types/events.rs`,
`gateway/gateway-execution/src/invoke/event_logging.rs`,
`gateway/gateway-execution/src/invoke/stream_event_processor.rs`

**Tests:**
- TDD: runtime executor test proves executed tool-result events carry
  `duration_ms`.
- TDD: gateway event logging test proves `duration_ms` lands on
  `ExecutionLog.duration_ms` when supplied.

**Approach:**
- Extend the stream tool-result event with optional `duration_ms`.
- Measure wall-clock elapsed around runtime tool execution.
- Thread the optional duration through gateway stream processing and log entry
  construction.

**Done when:** focused tests can inspect a tool-result log/event with populated
duration.

### T3: Invalid tool arguments include recovery hints

**Depends on:** none

**Touches:** `runtime/agent-tools/src/tools/execution/shell.rs`,
`runtime/agent-tools/src/tools/memory.rs`,
`runtime/agent-tools/src/tools/execution/skills.rs`

**Tests:**
- TDD: shell tool test for missing `command` expects the error to include an
  example `{ "command": "..." }` shape.
- TDD: memory tool test for missing `action` expects the error to include an
  example `{ "action": "get_fact", ... }` shape.
- TDD: load_skill missing skill test expects a recovery hint and available
  planner skill alias guidance when the requested name is `planning-highlevel`
  or `planning-decompose`.

**Approach:**
- Improve error strings at existing validation sites; avoid new validation
  wrappers or schema engines.
- Add narrow alias/hint handling for legacy planner skill names.

**Done when:** focused agent-tools tests pass and the errors are actionable.

### T4: Planner template references installed planning skills

**Depends on:** T3

**Touches:** `gateway/templates/agents/planner-agent.md`,
`gateway/templates/skills/*`

**Tests:**
- Goal-based: grep the planner template and prove it references installed
  `spec-builder` and `plan-composer` or supported aliases.
- TDD if an existing template test file is present; otherwise a focused grep
  check is sufficient because this is static content.

**Approach:**
- Update planner-agent instructions to load `spec-builder` then
  `plan-composer`.
- If aliases are implemented in `load_skill`, document the legacy names as
  supported compatibility rather than primary names.

**Done when:** planner-agent no longer instructs agents to load a missing
  skill first.

### T5: Gates and review

**Depends on:** T1-T4

**Touches:** `docs/specs/tool-waste-visibility/*`, `docs/specs/README.md`

**Tests:**
- Goal-based: run focused Rust tests for touched crates.
- Goal-based: run `cargo fmt --check`.
- Goal-based: run targeted clippy for touched crates where practical.
- Goal-based: run work-loop implement and review checks.

**Approach:**
- Update `docs/specs/README.md`.
- Run the work-loop gates and self-review for scope drift.

**Done when:** gates pass or any pre-existing blocker is documented with exact
command output.

## Rollout

Ship as a direct telemetry and template correction. Existing clients continue
to receive the same tool-result transcript content; logs become more accurate.

## Risks

- Some tests or UI code may assume blocked hooks have `error: null`; those
  should be updated because the old shape hid waste.
- Duration timing in async tool execution must be measured around the actual
  tool future and not around later result post-processing.
- Adding compatibility aliases must not hide genuinely unknown skill names.

## Changelog

- 2026-06-09: initial plan.
