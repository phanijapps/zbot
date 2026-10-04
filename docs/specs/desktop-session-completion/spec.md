# Spec: Desktop session completion

- **Status:** Draft
- **Owner:** @videogamer
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** [RFC-0021](../../rfc/0021-conversation-first-desktop-agent.md); [existing Desktop Session Shell](../desktop-session-shell/spec.md)
- **Brief:** docs/product/briefs/rig-hooks-session-graph.md
- **Discovery:** none
- **Contract:** [session-details.yaml](../../../contracts/openapi/session-details.yaml); [goal-artifacts.yaml](../../../contracts/openapi/goal-artifacts.yaml)
- **Shape:** mixed
- **Mode:** full (dependency, interface, security, or user-facing structural change)

> **Spec contract:** Objective, Boundaries, Testing Strategy, and Acceptance Criteria define delivery. The implementation matches this contract or changes it in the same reviewed work.

## Objective

The existing /session experience is complete and testably coherent: the visible conversation owns a working Activity/Sources/Files inspector, Chat/Research navigation and Stop preserve their recorded identity, and the accepted quiet desktop styling is readable and operable at desktop and narrow widths. This completion contract references the existing shell requirements rather than replacing their semantics or creating a second shell.

## Boundaries

### Always do

- Treat desktop-session-shell/spec.md AC1–AC15 and accepted amendments as the authoritative baseline; record each criterion’s current evidence and remaining gap before editing.
- Reuse SessionShell, Conversations, QuickChat, existing details/artifact APIs, shared tokens and administration pages.
- Preserve all existing user edits and workflow tracking; use seeded/isolated browser data.

### Ask first

- Change Chat/Research execution, persisted mode, cancellation, memory/source exposure or authoritative startup network defaults beyond the existing approved contract.
- Reset an in-flight cohort, change its locked plan, add a UI framework or replace the approved design.

### Never do

- Create a competing shell or copy the original AC definitions into another canonical home.
- Enable hook configuration/Test UI or infer sources, files or memory use from model prose.

## Testing Strategy

TDD component/transport tests cover proven QuickChat/session identity, actual tab selection, stale-response protection and mode/Stop/navigation behavior. Isolated HTTP and browser journeys verify server-backed details and artifacts across reload. Visual/manual QA compares the rendered surface to the approved mockup at 1280×800, 720px and 390px, with keyboard/focus/contrast checks and long content. The original contract’s acceptance matrix is the full baseline; this spec adds completion and evidence obligations rather than restating it.

## Acceptance Criteria

- [ ] **AC1 — Baseline reconciliation: every original shell AC has a recorded test/manual artifact or a named unmet gap in this spec’s completion-matrix.md. Delivery requires all original criteria pass, including effective default network binding; a plan or unchecked historical AC is not evidence.**
- [ ] **AC2 — Inspector ownership: the current selected Chat/Research owns a single functional Activity/Sources/Files inspector. Default QuickChat supplies a server-proven session identity or an explicit unavailable state. Tab state changes content and aria-selected; opening a tab does not create/reset a session. Stale details cannot appear under a new conversation.**
- [ ] **AC3 — Conversation parity: default QuickChat and explicit New chat remain distinct as specified; recent selection, mode lock, streaming, scoped Stop, final answers and artifacts survive navigation and reload with persisted identity. A failed Stop does not claim cancellation.**
- [ ] **AC4 — Presentation: at 1280×800 the centered mode switch, main conversation/composer and secondary inspector match the approved visual hierarchy; at 720px and 390px navigation/details are operable drawers and long Markdown/code/cards do not cause horizontal page overflow or obscure the composer. Shared tokens provide measured 4.5:1 text and 3:1 necessary control/focus contrast.**
- [ ] **AC5 — Administration continuity: existing Agents/Settings/Integrations actions and editor validation remain usable in the accepted shell styling; return-to-session and query state survive reload without destructive mutations. Hook management stays absent.**
- [ ] **AC6 — Evidence and completion: isolated journeys exercise empty/populated, running/completed/stopped/error, reconnect/reload, missing details and denied artifacts. Keyboard/focus and screenshot comparisons are retained. The original shell cannot be marked shipped through this companion without its own verification/review and authorized workflow transition.**

## Assumptions

- Technical: SessionShell renders placeholder outer tabs; the details endpoint and existing conversation components already exist (apps/ui/src/features/session-shell/SessionShell.tsx; contracts/openapi/session-details.yaml).
- Product: the existing approved mockup and completion request govern UX; no redesign is assumed (user approval 2026-10-03).
- Process: original plan is locked/Approved and spec is Implementing; no substantive plan edit or cohort reset is performed during this authoring pass.
- Design: experience-design pack is absent; the approved docs/product/zbot-desktop-ui-preview.html and apps/ui/ARCHITECTURE.md ground hierarchy and tokens, while final rendered review remains mandatory.
