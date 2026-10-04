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

Resolved: T2 dependency/bridge and T3 lifecycle/context/MCP migration pass the
frozen regression suite. T4 proves Chat/Research reload, Research delegation
join, real pending-request cancellation and continuation, and bounded live
intent checks for configured Ollama and Z.AI at 5000 tokens. See verification.md
for exact commands/results and baseline gate exceptions.

Review shape: the 2302-line runtime diff is split for review into released
model/structured/client plus private tool-scope/results, then runner/lifecycle/
context and mechanical MCP fixture mappings. Released signatures are coupled
at compile time; both groups share the complete working tree and retained
regression assertions. No intermediate incompatible engine is introduced.

Bundled fix resolved: the new compiler's multimodal Object match guard keeps
the same condition/body and passes strict clippy. Existing rustls advisory,
distillation license metadata and unrelated spec vocabulary remain registered
baseline issues; this migration neither suppresses nor expands their scope.

Resolved: adversarial, security and whole-spec quality reviews each report
Clean — ready to commit. All AC1–8 evidence is verified; no review findings
remain. The code loop awaits the human merge decision after PR publication.
Project-knowledge not requested. No production frontend changes in this Rig
slice; frontend review does not fire, and experience-reviewer is unavailable
(named skip for the documentation-only reader surface).

Capture triage: the reusable callback-contract lesson is a normative verification
procedure, so it is not admitted as a project-knowledge observation. It is
captured through the required memex procedure seam. No observation receipts
exist to distill and no project-knowledge journal diff is created.
