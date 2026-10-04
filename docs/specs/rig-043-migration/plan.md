# Plan: Rig 0.43.0 execution parity

- **Spec:** [`spec.md`](spec.md)
- **Status:** Done

## Approach

Migrate through the existing runtime/agent-runtime/src/rig_adapter boundary. Acquire the released Cargo/model/run/tool contracts first, then update adapters in dependency order. Retain zbot’s LlmClient and tool inventory instead of replacing its provider stack.

## Constraints

- The approved [brief](../../product/briefs/rig-hooks-session-graph.md) and the spec's Boundaries govern scope.
- Preserve existing uncommitted work. Spec authoring does not approve implementation or reset another workflow.
- Current branch is two commits behind `origin/main`; obtain a fresh isolated implementation base before code work. Do not stage/rebase the shared dirty checkout.

## Construction tests

An isolated gateway fixture reuses recorded tool/terminal events and an in-process provider to compare the user-visible result. Live provider checks are bounded and opt-in. Run `cargo test -p agent-runtime`, relevant `gateway-execution` tests, `cargo check --workspace`, and scoped clippy after migration.

## Design (LLD)

### Dependencies & integration

Published `rig` facade or narrow release components replace the old dependency only where required. Exact versions and required agent/MCP features are proven with `cargo metadata` (AC1–2). The pinned 0.43.0 source, not main-branch examples, is the oracle.

### Interfaces & contracts

`rig_adapter/model.rs`, `factory.rs`, `engine.rs`, `tool_hook.rs`, `context_inputs.rs` and `structured.rs` retain responsibility boundaries. Map `on_dispatch`/`on_outcome` for the tool family and release lifecycle callbacks to the existing zbot behavior (AC3–7). Do not import provider transport defaults over the established LlmClient.

### Failure & resilience

Explicit error mapping preserves cancellation, invalid calls and optional usage provenance. Progress/terminal persistence remain in existing gateway code. Adapter changes are divided into working, reviewable layers; tests prove no second runtime path (AC2–8).

## Tasks

### T1: Freeze parity fixtures and release migration map

**Depends on:** none

**Verification mode:** TDD and goal-based check

**Spec mapping:** AC1–8

**Tests:**
- Extend existing factory/model/result/live-context/MCP test modules with representative admitted/denied calls, respond-only terminal output, cancellation and optional usage.
- Record released facade/model/runner/tool signatures and feature closure without invoking a provider.

**Approach:**
Read runtime/agent-runtime/Cargo.toml, rig_adapter modules and existing adapter tests. Produce migration-map.md with old/new symbols and ownership. Keep tasks below 2000 reviewable behavior/test lines; split a large adapter task into working layers rather than a single rewrite.

### T2: Migrate release dependency and model/tool types

**Depends on:** T1

**Verification mode:** TDD and goal-based check

**Spec mapping:** AC1–4

**Tests:**
- Cargo metadata asserts exact releases and no 0.39.0 closure; focused model/structured/provider-error and tool-policy tests compile and pass.

**Approach:**
Update the runtime manifest/lockfile and the smallest model/tool interfaces; retain the common LlmClient. Run cargo check --workspace and cargo clippy -p agent-runtime --all-targets -- -D warnings.

### T3: Migrate runner, hooks, context and MCP lifetime

**Depends on:** T2

**Verification mode:** TDD

**Spec mapping:** AC2, AC4–7

**Tests:**
- Existing live-context, result, snapshot, control and MCP tests pin veto/rewrite order, stream parity, recall, cancellation and close-on-drop behavior.

**Approach:**
Update factory.rs, engine.rs, tool_hook.rs and context inputs against the release API. Keep gateway call sites on build_execution_engine and existing actor policy.

### T4: Verify gateway/provider parity and document release

**Depends on:** T3

**Verification mode:** Goal-based integration and end-to-end checks

**Spec mapping:** AC3, AC6–8

**Tests:**
- Isolated Chat and delegated Research complete, Stop, reload and retain final answers.
- Opt-in bounded intent checks use Ollama and z.ai without altering live settings; missing credentials are a named unverified result.

**Approach:**
Run runtime tests plus relevant gateway-execution intent/cancellation suites; record commands and evidence in verification.md and the release migration guide.

## Gates

- Run scoped Rust/UI checks named by the tasks, then the appropriate workspace build checks before implementation review.
- Run spec metadata and brief coverage lint, validate declared contracts, and check document links.
- Review spec and plan adversarially; apply findings and rerun affected gates. Security-boundary work also receives secure-design review.
- Keep per-task acceptance evidence in `verification.md` during implementation. No AC is checked merely because its plan exists.

## Risks

0.43.0 changes model ownership, optional usage, run protocol and component crates. Inventory APIs before editing; no compatibility engine or mass search/replace. External provider availability never counts as a passing parity check.

## Rollout

Ship the migrated release after parity gates, then implement external-hooks against that release. Preserve vault/provider data; roll back only the scoped dependency/adapter change if parity fails.

## Changelog

- 2026-10-03: Draft delivery contract and construction strategy from the approved brief; no runtime implementation.
