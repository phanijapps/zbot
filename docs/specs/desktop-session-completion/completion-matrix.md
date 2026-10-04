# Desktop session completion — baseline reconciliation matrix

Audit date: 2026-10-04. Base: `origin/develop` @ af9ff454 (PR #278 merge) on branch
`feat/desktop-session-completion`. The authoritative baseline is every Acceptance
Criterion of [`desktop-session-shell/spec.md`](../desktop-session-shell/spec.md)
**as amended** (enumerated below), plus its accepted amendments
(`administration-amendment.md`, `ui-integration-amendment.md`).

## Overlap disposition (owner authorization 2026-10-04)

The original shell cohort is mid-flight at `CODE-IMPLEMENTATION` (wave 2 of 6;
T3–T7 outstanding under a sealed plan hash). The owner authorized this companion
spec (brief `rig-hooks-session-graph`, approved spec/plan) to deliver the
outstanding inspector/presentation scope — the substance of the original cohort's
remaining T3 (mode-pinned shell rendering), T5 (administration palette/keyboard),
T6 (Activity/Sources/Files reload) and T7 (knowledge destinations/rollback)
obligations. The original cohort is **never reset**; its engine/cohort state is
inspected read-only; the original spec cannot be marked shipped through this
companion without its own verification/review and authorized workflow transition
(this spec's AC6).

## Matrix

| AC | Status | Current evidence | Remaining gap |
| --- | --- | --- | --- |
| AC1 shared entry | Pass (component+e2e) | `SessionShell.test.tsx` (entry, recents, mode switch, knowledge destinations, no ward link); `session-shell.ui.spec.ts` t1 (reload restore) | None |
| AC2 pinned mode | Pass | `SessionShell.test.tsx` (QuickChat reservation, unknown mode, recent selection by ID); e2e t1 | None |
| AC3 runtime parity | Pass with baseline note | e2e t1 (Chat/Research answers + reload, Stop absent after completion); `Conversations.tsx` Stop wiring | `pre-existing-research-stop-assertion` backlog entry (ResearchPage component-level test) is a known baseline failure, not a shell regression; failed-Stop journey asserted in T3/T4 |
| AC4 truthful Activity | Pass | `ActivityPanel.tsx` + `ActivityPanel.test.tsx`; `session-details.yaml`; `session_details_tests.rs` (order, truncation, hook rows) | None |
| AC5 truthful Sources | **Delivered (T2)** | SourcesPanel + tests (safe-destination links, evidence labels, truncation, unavailable/retry, stale-guard); contract + backend unchanged | None |
| AC6 truthful memory activity | Pass | `session_details_tests.rs` `details_reopens_root_and_continuation_activity_redacted`; labels rendered without raw content | Memory-record link-through not asserted in UI; retained as-is unless details contract exposes stable IDs (it currently does not) |
| AC7 safe Files | **Delivered (T2)** | FilesPanel + tests (manifest, open-by-ID via ArtifactSlideOut, wrong-session filter, settle reread, denial state); artifacts.rs confinement unchanged | None |
| AC8 knowledge destinations | Pass | `DesktopKnowledgePage.tsx`; `/memory` + `/observatory` routes in `App.tsx`; e2e t2 | None |
| AC9 ward compatibility | Pass | e2e t2 asserts no ward link; legacy `/vault` (`VaultPage`) direct route preserved | None |
| AC10 usability/visual | **Delivered (T3)** | Refreshed captures (evidence/): 1280 tabs + 390 drawers; measured contrast min 6.28:1 (session-contrast.spec); rendered HTML validates clean (0 violations; backlog slug closed) | None |
| AC11 administration | **Delivered (T3/T4)** | DesktopAdministrationPage.tsx + test; e2e session-admin.continuity.spec.ts (all three pages, return-to-session intact, no hook management) | None |
| AC12 sanitized failures | Pass | `session_details_tests.rs` `details_denies_lan_bind_and_cross_origin_with_fixed_errors` (bounded body, denial precedes read) | Re-verify unchanged in T4 full matrix |
| AC13 local details boundary | **Default half delivered (T3)** | Details handler guard + loopback CLI default (pre-existing); effective default now loopback with fail-closed load errors (discovery default flip + server.rs), startup-state unit tests + fresh-vault real-daemon proof | CLI `--host` vs settings precedence conflict pending owner disposition (below) — the `--host`-selects-LAN half of AC13 stays unverified until resolved |
| AC14 independent chat lifecycle | Pass | `SessionShell.test.tsx` (New chat independence, failure keeps QuickChat, single creation, no reset); `createChatSession` gateway contract from original T2 | None |
| AC15 visible subagents | Pass | e2e t2 (all subagent states, bands, contrast in QA artifact) | Re-capture after T3 styling work |

## Named gap: CLI `--host` vs settings-backed precedence (owner disposition required)

`gateway/src/server.rs:183-189` lets settings-backed resolution silently override
the daemon CLI `--host` in **both directions**: explicit `--host 0.0.0.0` is
demoted to loopback by a loopback-leaning settings file; explicit
`--host 127.0.0.1` is promoted to `0.0.0.0` by a LAN-leaning one. The existing
daemon test (`main.rs:143-151`) proves parse-level acceptance only. Original AC13
reads "an explicit `--host` or config can still select a LAN bind" — as written,
its `--host` half cannot pass effective-bind verification under T3's authorized
scope (default correction only). **Disposition required before T4 claims AC13**:
either (a) fresh owner approval for a precedence correction (explicit CLI host
wins, or a documented precedence rule), or (b) amend AC13's second clause to
config-only LAN selection. Surfaced to owner 2026-10-04.

## Audit runs (2026-10-04, worktree feat/desktop-session-completion @ 59828ad0)

- `npx vitest run src/features/session-shell/` — 39 passed / 2 files failed (the
  3 red T2 stubs by design; no pre-existing failures).
- `npx playwright test ui-mode/session-shell.ui.spec.ts` (fresh-vault,
  local-only, same-origin harness) — 3 passed.
- Original cohort state inspected read-only: `engine-state.json`
  `CODE-IMPLEMENTATION`, wave 2/6, `plan_review_status: approved` — untouched.
