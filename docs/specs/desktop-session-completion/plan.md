# Plan: Desktop session completion

- **Spec:** [`spec.md`](spec.md)
- **Status:** Executing

## Approach

Reconcile the existing implementation and its evidence, then close verified gaps in the same components. Keep the original shell spec/plan unchanged during this authoring pass. This companion plans the remaining delivery work; before code overlaps an active locked task, coordinate that task or obtain its authorized re-plan instead of bypassing its cohort.

## Constraints

- The approved [brief](../../product/briefs/rig-hooks-session-graph.md) and the spec's Boundaries govern scope.
- Preserve existing uncommitted work. Spec authoring does not approve implementation or reset another workflow.
- Refresh from the execution base branch (`origin/develop`) into an isolated worktree before code work; never stage or rebase the shared dirty checkout.
- Every browser journey boots the isolated harness with a seeded vault (`--fresh-vault`) and loopback binding (`--local-only`, or an asserted effective loopback bind); the host data dir is neither read nor written.

## Construction tests

Existing isolated browser fixtures provide the primary evidence. Include request-count assertions on navigation to prove read-only selection and test failed Stop/no-proven-live-key states. Visual captures use generated content only, per the spec's AC6 retention clause. Never rely on the default live view as proof of active/reload parity.

## Design (LLD)

### Component / module decomposition

`SessionShell.tsx` and `Conversations.tsx` own selection and the single inspector; QuickChat provides proven server identity through its existing transport state. Reuse an existing details panel if available before creating a focused presentation component. Original shell ACs remain in one canonical document (companion AC1–3).

### State & control flow

Selected/proven identity → bounded fetch → loading/ready/empty/unavailable/error. Every response is tied to its initiating identity; tab changes do not mutate conversations. Narrow drawers restore focus to their trigger and remain keyboard operable (AC2–5).

### Quality attributes

Typography, surfaces and controls use existing theme.css/components.css tokens, guided by the approved mockup. Visual evidence includes contrasting agent cards, readable Markdown/code and pinned composer rather than a component-render smoke assertion (AC4–6).

## Tasks

### T1: Audit current shell against the accepted baseline

**Depends on:** none

**Verification mode:** Goal-based

**Spec mapping:** AC1, AC6

**Tests:**
- Re-run existing SessionShell/QuickChat/conversation/administration tests and isolated e2e/playwright/ui-mode/session-shell.ui.spec.ts (with `--fresh-vault` and `--local-only`). Retain failures rather than changing the expected behavior.
- completion-matrix.md records the overlap disposition — owner authorization 2026-10-04: this companion delivers the inspector/presentation scope still outstanding in the original cohort's remaining waves; the original cohort is inspected read-only, never reset, and its own completion remains a separate authorized transition (AC6). T2 may not start without this recorded disposition.
- completion-matrix.md names the CLI `--host` vs settings-backed resolution precedence conflict (gateway/src/server.rs silently overrides an explicit CLI host in both directions) as a gap requiring owner disposition — a precedence correction approval or an AC13 amendment to config-only LAN selection — before T4 claims AC13.

**Approach:**
Create completion-matrix.md enumerating every original Acceptance Criterion (as amended) with its current artifact or specific unresolved gap, including effective network bind. Inspect the active shell cohort read-only and record the overlap disposition above; do not reset it.

### T2: Make one session-bound inspector functional

**Depends on:** T1 (including its recorded overlap disposition)

**Verification mode:** TDD

**Spec mapping:** AC2–3

**Tests:**
- Tab content/ARIA, default proven identity, unavailable identity, conversation switch with a late response, empty/failed details and artifact boundary cases.
- `apps/ui/src/features/session-shell/SourcesPanel.test.tsx` (stub: true — red stub asserts the Sources tab renders a server-provided source row)
- `apps/ui/src/features/session-shell/FilesPanel.test.tsx` (stub: true — red stub asserts the Files tab renders the artifact manifest and opens content by artifact ID)
- `apps/ui/src/features/session-shell/SessionShell.test.tsx` additions for tab-switch identity and stale-response protection

**Approach:**
Reuse existing components and APIs in SessionShell/Conversations and chat-v2 state. Unify duplicate inspectors only where verified; preserve QuickChat behavior and selected-session semantics. Any required identity contract correction receives HTTP-negative tests.

### T3: Close audited presentation and continuity gaps

**Depends on:** T2

**Verification mode:** TDD plus visual/manual QA

**Spec mapping:** AC3–5

**Tests:**
- Mode/Stop/reload and administration return journeys; desktop/narrow/code/long-card captures, contrast readings and focus restoration. Browser journeys run with `--fresh-vault` and `--local-only` (or asserted effective loopback bind).
- Startup-default correction verified by actual startup tests enumerating the deciding states: no `settings.json`; `settings.json` without a `network` block; unreadable/corrupt `settings.json` (must fail closed to loopback); explicit `exposeToLan=true`; explicit `advanced.bindHost`.

**Approach:**
Apply only gaps in completion-matrix.md to current components/styles; preserve existing source edits. The effective startup-default correction (gateway/src/server.rs resolves the bind from settings-backed network config, defaulting to LAN exposure) is executed under the owner's standing authorization recorded with this spec's approval (2026-10-04): default loopback with explicit LAN opt-in, verified by actual startup tests; any change beyond that scope requires fresh approval. Coordinate any locked-plan change before executing.

### T4: Complete full acceptance evidence and original handoff

**Depends on:** T3

**Verification mode:** Goal-based end-to-end and visual/manual QA

**Spec mapping:** AC1–6

**Tests:**
- Full original Acceptance Criteria matrix (as amended, enumerated in completion-matrix.md), both execution routes, unauthorized details/files, active Stop and final-answer reload; no external calls or private-vault reads or mutations in fixtures, with `--fresh-vault` and an asserted effective loopback bind mechanizing both isolation dimensions.
- Administration journeys assert no hook-management control or hook execution action is rendered (AC5).

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
- 2026-10-04: Owner approved spec/plan (recorded on develop in 87a2a711) and authorized two scoped decisions: the original-cohort overlap disposition (this companion delivers the outstanding inspector/presentation scope; the original cohort is never reset) and the effective startup-default correction (default loopback with explicit LAN opt-in under actual startup tests; anything beyond requires fresh approval). Pre-EXECUTE adversarial + security review rounds applied.
