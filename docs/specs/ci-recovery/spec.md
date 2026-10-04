# Spec: CI recovery

Mode: full (lockfile update adds the release-required syn 3 dependency).

- **Status:** Shipped
- **Owner:** phanijapps
- **Plan:** [plan.md](plan.md)
- **Constrained by:** none
- **Contract:** none

## Objective

The Research Stop fixture respects the existing session identity requirement,
and Clippy evaluates async traits without the generated double_must_use error.
Existing checks and runtime behavior remain intact.

## Acceptance Criteria

- [x] AC1: Idle research has no Stop button; running research with a session ID dispatches Stop; running research without a session ID has a disabled Stop button and cannot dispatch it.
- [x] AC2: The lockfile updates async-trait from 0.1.89 to 0.1.92 and adds only its required syn 3 parser; other versions remain unchanged and Clippy reports no generated double_must_use errors.
- [x] AC3: The full frontend suite, workspace check and affected Rust tests pass; any other existing gate failure is evidenced separately.

## Authorization

2026-10-03: user said “fix them” after receiving the exact fixture and
async-trait 0.1.92 remediation proposal. Scope and approach are approved.

## Boundaries

### Always do

- Retain the existing session identity guard and all CI gates.
- Use Cargo registry checksums for the official macro and its required parser.
- Preserve live provider/vault configuration and sealed Rig/session/graph plans.

### Ask first

- Change production Stop behavior or expand the dependency/compiler scope beyond the approved patch and its required parser.

### Never do

- Apply blanket lint suppression, disable a CI check, or perform a dependency sweep.
- Introduce a runtime abstraction, replace provider transport, or reset another spec's cohort.

## Testing Strategy

Existing failing ResearchPage test is the reproduction. Add the missing
identity-negative contract check and run the full frontend suite. Goal-based
Cargo check, all-target Clippy and affected crate tests validate the macro
update. Dependency audit/policy tools own advisory scanning; unrelated baseline
findings remain explicit. No production UI or API behavior changes.

## Assumptions

- The session identity requirement is already enforced in ResearchPage.tsx.
- Cargo's precise update of async-trait 0.1.92 requires syn 3; the updated async-trait 0.1.92 and syn 3.0.6 manifests require Rust 1.71, below the local Rust 1.94.1. async-trait 0.1.89 previously required Rust 1.56.
- The user approved fixing these two diagnosed failures; no live execution or settings migration is required.
