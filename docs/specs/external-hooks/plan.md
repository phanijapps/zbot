# Plan: File-configured external hooks

- **Spec:** [`spec.md`](spec.md)
- **Status:** Drafting

## Approach

After rig-043-migration, load a small immutable hook snapshot through the gateway and adapt it at existing lifecycle boundaries. Place process execution/config parsing in agent-runtime, VaultPaths resolution and snapshot ownership in gateway-execution, and sanitized activity in existing api-logs/session projection. Reuse the established async process/cancellation helpers where their contract suffices.

## Constraints

- The approved [brief](../../product/briefs/rig-hooks-session-graph.md) and the spec's Boundaries govern scope.
- Preserve existing uncommitted work. Spec authoring does not approve implementation or reset another workflow.
- Current branch is two commits behind `origin/main`; obtain a fresh isolated implementation base before code work. Do not stage/rebase the shared dirty checkout.

## Construction tests

Cross-task integration invokes a new root Chat and a delegated Research using a temp vault/config/hooks.json and fixture executables; verifies event IDs/counts, Stop cleanup and durable metadata after reload. Contract schemas are validated with draft-2020-12 JSON Schema tooling. Rust task suites and UI Activity tests run without real credentials.

## Design (LLD)

### Interfaces & contracts

The configuration, stdin event and stdout response schemas are canonical. Payload IDs identify actual attempts, not a guessed session lifecycle. `data` carries only event-relevant input: prompt/context at ingress, provider metadata at model boundaries, allowed tool identity/credential-field-filtered arguments and outcome category where needed; credentials and internal instruction/recall state are excluded. Payload overflow fails rather than truncating security-relevant arguments (AC1–8).

### State & control flow

Gateway ingress validates the file before activating the root invocation. One immutable snapshot flows to delegates. Map `run_start` to `on_run_start`, `run_end` to settlement, `before_model` to the completion boundary and `after_model` to the completion outcome; tool-only dispatch/outcome drive before/after-tool. `on_invalid_tool_call` drives the rejected-call event. Filter effect families so completion is not invoked twice through dispatch and completion callbacks (AC2–6, AC11).

### Failure & resilience

Close stdin explicitly, read stdout/stderr concurrently, and enforce one end-to-end deadline. Kill/reap the process tree on timeout/Stop; retain only fixed-category metadata. Ordinary settlement invokes run_end; cancelled settlement follows the separate total five-second cleanup budget in the spec, with no context/control changes. Hook execution does not recursively trigger hook events (AC5–9).

### Dependencies & integration

No schema-plugin framework or new interpreter dependency. The documented environment is PATH plus platform necessities (SYSTEMROOT on Windows, UTF-8 locale where provided); HOME and provider/MCP variables are excluded. Working/script paths resolve from VaultPaths; v1 accepts only vault cwd and operator-managed file entry points outside ward/project roots. Validate canonical resolved paths, owner/write permissions and interpreter file-form syntax before spawn; no trust registry/UI. Platform process-tree semantics are verified explicitly (AC4, AC7–8).

## Tasks

### T1: Validate and snapshot operator configuration

**Depends on:** none

**Verification mode:** TDD

**Spec mapping:** AC1–2, AC8, AC10

**Tests:**
- Schema positive/negative cases including rejected ward cwd, duplicate-ID/default behavior, absent/malformed/oversized config, edits between and during runs, child snapshot identity.

**Approach:**
Add a focused config reader under runtime/agent-runtime/src and a hooks_config path in VaultPaths. Resolve it once in gateway-execution ingress; no API setter and no Settings changes. Document that implementation depends on rig-043-migration as a spec dependency.

### T2: Invoke processes through the JSON contract

**Depends on:** T1

**Verification mode:** TDD with real subprocess integration

**Spec mapping:** AC4–8

**Tests:**
- Rejected ward/project-script/symlink/writable-file and eval/module cases spawn zero processes. Fixture programs echo argv/stdin, return continue/block/context, produce malformed output, fill both pipes, hang before reading stdin, fork descendants and exit nonzero. Assert deadlines, bounds and cleanup. Test nested Authorization/api_key/OAuth/token fields, name/value header arrays, environment containers and registered provider/MCP secret sentinels; projection failure yields empty flagged arguments, never raw fallback.

**Approach:**
Reuse runtime/agent-tools/src/tools/execution/shell.rs process-group/cancellation patterns without routing hook argv through a shell. Implement a small runtime-owned command invoker and response validator.

### T3: Wire root and delegated event boundaries

**Depends on:** T2

**Verification mode:** TDD and gateway/runtime integration

**Spec mapping:** AC2–6, AC11

**Tests:**
- Table-driven cardinality fixtures cover new/existing sessions, root message, delegated run, repeated model attempts, allowed/denied tools and both delivery modes. Protected veto positive control prevents effects even with configurable continue.

**Approach:**
Bind hooks through the 0.43.0 adapter, InvokeBootstrap root ingress and delegated/continuation paths. Apply already-protected policy first or compose it so configured decisions can never restore denied capabilities. Avoid recursion and do not execute ingress hooks on read-only hydration.

### T4: Persist sanitized outcomes and finish operator workflow

**Depends on:** T3

**Verification mode:** TDD plus visual/manual QA

**Spec mapping:** AC7–11

**Tests:**
- Session details/Activity before-after reload contain expected hook metadata and no raw secret/path sentinel. Python, Node and compiled fixture journeys verify the actual file-to-runtime path.

**Approach:**
Use services/api-logs and the existing session_details projector for metadata; extend the existing Activity row contract compatibly where required. Write docs/guides/reference/external-hooks.md and a minimal end-to-end setup walkthrough; use isolated vault fixtures.

## Gates

- Run scoped Rust/UI checks named by the tasks, then the appropriate workspace build checks before implementation review.
- Run spec metadata and brief coverage lint, validate declared contracts, and check document links.
- Review spec and plan adversarially; apply findings and rerun affected gates. Security-boundary work also receives secure-design review.
- Keep per-task acceptance evidence in `verification.md` during implementation. No AC is checked merely because its plan exists.

## Risks

External programs inherit daemon host rights and may modify files; a schema is not a sandbox. The operator owns config and program integrity. Cross-platform descendant cleanup needs an actual process-control strategy; ship only verified hosts and report unsupported behavior instead of orphaning children.

## Rollout

Install schema/examples without overwriting edits. Missing/empty configuration leaves no configured hook processes; enable through the operator file only. Preserve current user vault files and require actual per-platform subprocess evidence before delivery.

## Changelog

- 2026-10-03: Draft delivery contract and construction strategy from the approved brief; no runtime implementation.
