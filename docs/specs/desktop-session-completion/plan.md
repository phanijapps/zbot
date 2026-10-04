# Plan: Desktop session completion

- **Spec:** [`spec.md`](spec.md)
- **Status:** Approved

## Approach

Reconcile the existing implementation and its evidence, then close verified gaps in the same components. Keep the original shell spec/plan unchanged during this authoring pass. This companion plans the remaining delivery work; before code overlaps an active locked task, coordinate that task or obtain its authorized re-plan instead of bypassing its cohort.

## Constraints

- The approved [brief](../../product/briefs/rig-hooks-session-graph.md) and the spec's Boundaries govern scope.
- Preserve existing uncommitted work. Spec authoring does not approve implementation or reset another workflow.
- Current branch is two commits behind `origin/main`; obtain a fresh isolated implementation base before code work. Do not stage/rebase the shared dirty checkout.

## Construction tests

Existing isolated browser fixtures provide the primary evidence. Include request-count assertions on navigation to prove read-only selection and test failed Stop/no-proven-live-key states. Visual captures use generated content instead of user messages. Never rely on the default live view as proof of active/reload parity.

## Design (LLD)

### Component / module decomposition

`SessionShell.tsx` and `Conversations.tsx` own selection and the single inspector; QuickChat provides proven server identity through its existing transport state. Reuse an existing details panel if available before creating a focused presentation component. Original shell ACs remain in one canonical document (AC1–3).

### State & control flow

Selected/proven identity → bounded fetch → loading/ready/empty/unavailable/error. Every response is tied to its initiating identity; tab changes do not mutate conversations. Narrow drawers restore focus to their trigger and remain keyboard operable (AC2–5).

### Quality attributes

Typography, surfaces and controls use existing theme.css/components.css tokens, guided by the approved mockup. Visual evidence includes contrasting agent cards, readable Markdown/code and pinned composer rather than a component-render smoke assertion (AC4–6).

## Tasks

### T1: Audit current shell against the accepted baseline

**Depends on:** none

**Verification mode:** Goal-based and visual/manual QA

**Spec mapping:** AC1, AC6

**Tests:**
- Re-run existing SessionShell/QuickChat/conversation/administration tests and isolated e2e/playwright/ui-mode/session-shell.ui.spec.ts. Retain failures rather than changing the expected behavior.

**Approach:**
Create completion-matrix.md mapping original AC1–AC15 to current artifacts and specific unresolved gaps, including effective network bind. Inspect the active shell cohort read-only and name any overlapping locked task; do not reset it.

### T2: Make one session-bound inspector functional

**Depends on:** T1

**Verification mode:** TDD

**Spec mapping:** AC2–3

**Tests:**
- Tab content/ARIA, default proven identity, unavailable identity, conversation switch with a late response, empty/failed details and artifact boundary cases.

**Approach:**
Reuse existing components and APIs in SessionShell/Conversations and chat-v2 state. Unify duplicate inspectors only where verified; preserve QuickChat behavior and selected-session semantics. Any required identity contract correction receives HTTP-negative tests.

### T3: Close audited presentation and continuity gaps

**Depends on:** T2

**Verification mode:** TDD plus visual/manual QA

**Spec mapping:** AC3–5

**Tests:**
- Mode/Stop/reload and administration return journeys; desktop/narrow/code/long-card captures, contrast readings and focus restoration.

**Approach:**
Apply only gaps in completion-matrix.md to current components/styles; preserve existing source edits. Effective startup-default correction uses the original approved local-only requirement with LAN opt-in and actual startup tests; coordinate any locked-plan change before executing.

### T4: Complete full acceptance evidence and original handoff

**Depends on:** T3

**Verification mode:** Goal-based end-to-end and visual/manual QA

**Spec mapping:** AC1–6

**Tests:**
- Full original AC1–AC15 matrix, both execution routes, unauthorized details/files, active Stop and final-answer reload; no external calls or private-vault mutations in fixtures.

**Approach:**
Record verification.md and refreshed QA artifacts; run npm run build and relevant Vitest/Playwright suites plus gateway HTTP checks. Propose original workflow completion only after its independent gate passes.

## Gates

- Run scoped Rust/UI checks named by the tasks, then the appropriate workspace build checks before implementation review.
- Run spec metadata and brief coverage lint, validate declared contracts, and check document links.
- Review spec and plan adversarially; apply findings and rerun affected gates. Security-boundary work also receives secure-design review.
- Keep per-task acceptance evidence in `verification.md` during implementation. No AC is checked merely because its plan exists.

## Risks

Original shell workflow is already active and has sealed plan hashes. This companion cannot supersede that approval or silently reset it. Runtime/network fixes require scoped coordination; a cosmetic screenshot pass is insufficient for baseline completion.

## Rollout

Preserve legacy routes and the original rollback path. Promote only after the baseline matrix and current completion checks pass. Hook Activity enrichment is delivered by external-hooks and is not a hard prerequisite for completing the existing shell.

## Changelog

- 2026-10-03: Draft delivery contract and construction strategy from the approved brief; no runtime implementation.
