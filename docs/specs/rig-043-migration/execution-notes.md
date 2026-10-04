# Execution disposition record

Scope: Rig migration T1 first, followed by dependency/type and runner/hook layers in the approved plan.

Assumptions: edits stay in runtime/agent-runtime/rig_adapter and manifests/toolchain requirements that the published release proves necessary; adapter/gateway fixture tests plus cargo check/clippy prove parity; configured providers, live vault and execution policy remain preserved.

Declined: alternate executor/compatibility framework because Rig remains the only runtime; provider replacement because the existing LlmClient already owns transport; mass search/replace because hook lifecycle semantics changed.

Pre-execution adversarial and secure-design evidence: ../rig-hooks-session-graph-review.md, both Clean. User selected develop base and authorized all four scopes/plans on 2026-10-03. Original shell cohort is copied unchanged and its plan hash guard passes.

Project-knowledge not requested. No eligible reusable scratch is admitted at approval/plan-lock; no knowledge diff is created.

Resolved: new branch contains current develop and published work; no stale-base recovery needed. Published Rig 0.43 requires Rust >=1.95 and explicit agent feature. Stable 1.94.1 is insufficient; installed 1.97.0 is available for isolated compiler checks.

Resolved: T1 migration-map.md cites the released adapter signatures and runtime semantic traps; the frozen 456-test baseline passes.

Resolved: T2 release-only typed/runtime probe passes three contract tests using
Rust 1.97.0 and exact rig 0.43.0, agent/rmcp enabled. Scope privacy, single-call
tool data/absent usage, transcript-only response history and dispatch gating are
proven for the SDK. Native integration resolves rmcp 2.2.0, requiring deliberate
compatibility work with host 1.7.0. No host migration gate is claimed passed.

Open: T2 dependency/bridge implementation, T3 runner/hook/context port, and T4 migrated-release parity. Existing tests remain the regression authority; do not weaken sequential-terminal or checkpoint assertions.
