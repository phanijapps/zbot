# Plan: Mission Control Performance

- **Spec:** [`spec.md`](spec.md)
- **Status:** Done

> **Plan contract:** this is the implementation strategy. Unlike the spec, this
> document is allowed to change as you learn. When it changes substantially
> (a different approach, not just a re-ordering), note why in the changelog
> at the bottom.

## Approach

Add a lightweight Mission Control summary path to `execution-state` and move the
UI's initial data flow onto it. The backend work is a bounded session-page query
plus one batched execution query for that page; the UI work is a transport
method and a Mission Control hook that produces the existing list rows and token
index without calling `listSessionsFull`. After that, consolidate selected
session detail fetches so Messages and Tools do not independently pull the same
full logs on first selection. The same bounded-loading contract applies to the
Research session UI: its drawer uses the summary endpoint, and opening a deep
link fetches only the requested root session plus its child executions.

## Constraints

- No ADR/RFC constraints.
- Must preserve the existing `/mission-control` route and selected-session
  inspection UX.
- Must not change retention, archival, deletion, or cleanup policy.
- Must not add new dependencies or a new service boundary.

## Construction tests

**Integration tests:** targeted Rust tests for the summary repository/handler,
plus targeted UI tests for Mission Control data loading.
**Manual verification:** run the UI locally and confirm `/mission-control`
renders the list from the summary endpoint; use browser devtools/network or
test mocks to confirm no initial `sessions/full` request.

## Tasks

### T1: Mission Control summary API returns bounded page data

**Depends on:** none

**Touches:** services/execution-state/src/types.rs, services/execution-state/src/repository.rs, services/execution-state/src/service.rs, services/execution-state/src/handlers.rs, services/execution-state/src/lib.rs

**Tests:**
- TDD: repository test creates more than 50 root sessions and proves the summary
  query returns 50 by default.
- TDD: repository test requests an excessive limit and proves the result is
  clamped to 200.
- TDD: repository test creates root and delegated executions and proves the
  summary response includes root execution ID, total tokens, and subagent count
  without per-execution token slices.

**Approach:**
- Add `MissionControlFilter`, `MissionControlSessionSummary`, and
  `MissionControlExecutionSummary` response types.
- Add a repository method that pages root `sessions` rows and fetches all
  `agent_executions` for those session IDs in one batched query for root ID and
  subagent count derivation.
- Add service/handler/routes wiring under the existing `/api/executions/v2`
  namespace.

**Done when:** targeted `execution-state` tests for mission-control summary pass.

### T2: UI transport consumes the summary endpoint

**Depends on:** T1

**Touches:** apps/ui/src/services/transport/types.ts, apps/ui/src/services/transport/interface.ts, apps/ui/src/services/transport/http.ts, apps/ui/src/services/transport/*.test.ts

**Tests:**
- TDD: transport test proves `listMissionControlSessions({ limit: 50 })` calls
  the new endpoint and serializes query parameters.
- TDD: type-level build or targeted tests prove the response shape is available
  to Mission Control code.

**Approach:**
- Add TypeScript response/filter types mirroring the Rust API.
- Add transport interface and HTTP implementation method.
- Add or update existing transport tests.

**Done when:** targeted transport tests pass.

### T3: Mission Control initial render stops using full sessions

**Depends on:** T2

**Touches:** apps/ui/src/features/mission-control/MissionControlPage.tsx, apps/ui/src/features/mission-control/useSessionTokens.ts, apps/ui/src/features/mission-control/*.test.tsx

**Tests:**
- TDD: Mission Control page test proves initial render calls the summary
  transport method.
- TDD: Mission Control page test proves initial render does not call
  `listSessionsFull`.
- TDD: pure mapping test proves summary rows become `LogSession` rows and
  `SessionTokenIndex` entries.

**Approach:**
- Replace the dual `useSessionTokens` bootstrap with one summary hook.
- Map summaries into the existing `LogSession` list shape to minimize
  component churn.
- Build `SessionTokenIndex` from summary aggregate totals only; per-execution
  slices are loaded by the selected-session token endpoint in T8.

**Done when:** targeted Mission Control tests pass and the page no longer uses
`listSessionsFull` on initial render.

### T4: Selected-session detail fetch is shared by Messages and Tools

**Depends on:** T3

**Touches:** apps/ui/src/features/mission-control/SessionDetailPane.tsx, apps/ui/src/features/mission-control/MessagesPane.tsx, apps/ui/src/features/mission-control/ToolsPane.tsx, apps/ui/src/features/logs/useSessionTrace.ts, apps/ui/src/features/mission-control/*.test.tsx

**Tests:**
- TDD: selected detail test proves one root detail request feeds both messages
  and tools on initial selection.
- TDD: existing MessagesPane/ToolsPane tests continue to pass with the shared
  detail path.

**Approach:**
- Lift selected-session detail fetching into `SessionDetailPane` or a shared
  hook.
- Let Messages and Tools consume the shared root/child detail bundle where
  available.
- Keep live refresh behavior for running sessions, but avoid duplicate sibling
  requests on the same tick.

**Done when:** detail panes render from shared data and targeted tests pass.

### T5: Gates and review

**Depends on:** T1-T4, T6, T7, T8

**Touches:** docs/specs/mission-control-performance/*, docs/specs/README.md

**Tests:**
- Goal-based: run `cargo test -p execution-state mission_control`.
- Goal-based: run targeted UI tests for transport and Mission Control.
- Goal-based: run targeted UI tests for Research bounded loading.
- Goal-based: run targeted UI tests for duplicate request coalescing.
- Goal-based: run targeted backend/UI tests for Mission Control list-only
  payload and selected-session token loading.
- Goal-based: run `cargo fmt --check` or scoped rustfmt check for touched Rust
  files, plus UI typecheck/lint where practical.

**Approach:**
- Update `docs/specs/README.md`.
- Run the work-loop gates.
- Self-review for scope creep and performance regressions.

**Done when:** gates pass or any pre-existing blockers are documented with the
exact command and failure.

### T6: Research session route stops scanning all log sessions

**Depends on:** T2

**Touches:** apps/ui/src/features/research-v2/session-snapshot.ts, apps/ui/src/features/research-v2/useSessionsList.ts, apps/ui/src/features/research-v2/useResearchSession.ts, apps/ui/src/features/research-v2/*.test.ts

**Tests:**
- TDD: Research drawer hook test proves it calls `listMissionControlSessions`
  with a bounded limit and filters chat rows from the summary shape.
- TDD: snapshot tests prove `/research/:sessionId` uses `getLogSession` for the
  root and child execution IDs instead of `listLogSessions`.
- TDD: useResearchSession tests prove hydration and re-snapshot paths use the
  scoped detail contract.

**Approach:**
- Move `useSessionsList` onto the bounded Mission Control summary endpoint.
- Change `snapshotSession` to load the root detail by requested session ID and
  then fan out only to `child_session_ids`.
- Change reconnect recovery to use a small bounded summary page instead of a
  full log-session list.

**Done when:** focused Research tests pass and production `research-v2` code no
longer calls `listLogSessions()`.

### T7: Research startup requests are coalesced and deferred

**Depends on:** T6

**Touches:** apps/ui/src/App.tsx, apps/ui/src/features/chat/mission-hooks.ts, apps/ui/src/features/research-v2/ResearchPage.tsx, apps/ui/src/features/research-v2/useSessionsList.ts, apps/ui/src/features/research-v2/useResearchSession.ts

**Tests:**
- TDD: App initialization test proves StrictMode does not duplicate transport
  initialization, health, or connect calls.
- TDD: recent-session and Research drawer hook tests prove StrictMode mount
  replay coalesces into one request.
- TDD: Research session hook test proves direct session hydration starts one
  scoped root detail request under StrictMode.

**Approach:**
- Share the App initialization promise across StrictMode effect replay and pass
  the health response's version into the top-bar badge.
- Add in-flight guards for recent-session, drawer-session, and direct hydration
  fetches.
- Defer drawer session loading until the drawer is opened.

**Done when:** focused UI tests and production build pass.

### T8: Mission Control list payload is list-only and request-coalesced

**Depends on:** T1-T4

**Touches:** services/execution-state/src/types.rs, services/execution-state/src/repository.rs, services/execution-state/src/service.rs, services/execution-state/src/handlers.rs, services/execution-state/src/lib.rs, apps/ui/src/services/transport/*, apps/ui/src/features/mission-control/*

**Tests:**
- TDD: backend summary test proves Mission Control list rows serialize without
  `executions` and `child_execution_ids`.
- TDD: backend selected-token test proves per-execution token slices are
  available for one selected session.
- TDD: UI hook test proves StrictMode does not duplicate the list request.
- TDD: UI detail test proves selected-session token data is fetched only after
  selection/detail mount.

**Approach:**
- Remove per-execution slices and child ID arrays from the list response type.
- Add a selected-session token endpoint under the Mission Control namespace.
- Change Mission Control list mapping to use `subagent_count` directly.
- Load per-execution token slices in the selected detail pane, merge them with
  aggregate list totals for the header/list, and pass them to ToolsPane.
- Add in-flight guards to Mission Control list and detail bundle hooks.

**Done when:** focused Rust/UI tests pass and local response-size smoke shows
the 50-row list payload no longer carries execution arrays.

## Rollout

Ship as a direct replacement for Mission Control's initial data source. The
legacy `listSessionsFull` endpoint remains available for other callers.

## Risks

- Summary rows use root execution IDs to remain compatible with the existing
  log-detail APIs; mistakes here can break selected-session inspection.
- Existing KPI logic is based on visible rows, so aggregate KPI correctness may
  require a follow-up once the summary endpoint is in place.
- SQLite query changes need tests that catch accidental N+1 regressions and
  limit bypasses.

## Changelog

- 2026-06-09: initial plan.
- 2026-06-09: expanded scope to cover the same database-growth load issue on
  `/research/:sessionId` and the Research drawer.
- 2026-06-09: added duplicate-request fix for `/research` dev-mode StrictMode
  startup and deferred the hidden drawer list fetch until open.
- 2026-06-09: tightened Mission Control list payload to high-level rows only
  and moved per-execution token slices to selected-session loading.
