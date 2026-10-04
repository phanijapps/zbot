# Plan: Desktop Session Shell

- **Spec:** [`spec.md`](spec.md)
- **Status:** Approved

> **Plan contract:** this is the implementation strategy. It can change as evidence arrives; the spec remains the user-visible contract.

## Approach

Keep the Rust daemon authoritative and the existing Chat and Research hooks separate. Add a narrow, read-only session-detail projection to existing session HTTP routing, built from persisted mode, messages, and logs; do not synthesize missing provenance. Reuse the existing QuickChat component for Session's default Chat and Chat-tab navigation; keep independent creation only for explicit New chat and read-only selected Chat hydration. Preserve legacy singleton/reset behavior. Build a React shell around the two paths, reuse its DesktopRail and tokens for full existing knowledge and administration pages, then wire a single details inspector to the durable projection and existing artifact APIs. Move the default entry only after parity tests pass; retain legacy routes and ward explorer for rollback. T1/T2 and earlier T3 presentation are already implemented and must be revalidated in the replacement run (see `disposition.md`).

## Constraints

- [RFC-0021](../../rfc/0021-conversation-first-desktop-agent.md) and its 2026-09-27 errata define the shell and hide, but retain, the ward explorer.
- [RFC-0011](../../rfc/0011-engram-memory-engine-cutover.md) owns memory-engine behavior; this plan changes only presentation and session provenance.
- [session-details.yaml](../../../contracts/openapi/session-details.yaml) defines the new detail-read and independent Chat create/open contracts; [goal-artifacts.yaml](../../../contracts/openapi/goal-artifacts.yaml) remains the file-access contract.
- Preserve the earlier CLI-default edit and test evidence, but do not change authoritative startup network defaults or user settings in this replacement run. Actual startup overrides the CLI through DiscoveryConfig, so the default-startup portion of AC13 is still unsatisfied and requires separately approved correction. Explicitly local-only isolated fixtures may verify UI parity. No default-route promotion or shipped claim until AC13 is resolved.
- No desktop host, lifecycle-hook execution, new agent builder or new settings/customization editor is included. Reuse and restyle the existing Agents, Settings and Integrations pages and editors; do not change their transport payloads, persistence or privilege boundaries. See `administration-amendment.md` for the retrofit limits and verification matrix.

## Construction tests

The PLAN gate materializes the contract-first red stubs below before production edits. T4 administration route/query stubs and T5 shared keyboard stubs are compilable files at PLAN; T1–T3/T6 retain their earlier construction artifacts/evidence. The stubs pin observable behavior; EXECUTE fills in fixtures, negative cases, and assertions before refactoring.

**Integration and end-to-end tests:** Seed persisted Chat and Research sessions with mode, message/tool/log records, one structured source-use record where available, memory recall with and without stable ID, and artifacts. Compare final shell details before and after reload; verify unknown provenance stays absent. Exercise browser Origin rejection, sanitized error bodies and logs, and cross-session artifact access through the real HTTP handlers. Compare Chat and Research streaming, Stop success/failure, final answer, artifacts, and reload against their existing routes; select a server-backed recent session and verify its recorded mode and transcript restore. Planned suites: `gateway/src/http/session_details_tests.rs` and `e2e/playwright/ui-mode/session-shell.ui.spec.ts`.

**Manual verification:** At 1280×800 and 720px, run one Chat turn and one delegated Research turn; Stop a live turn; reload both; inspect Activity, Sources, Files, Memory search/inspect/correct, and Observatory graph navigation; verify the ward explorer is absent from the shell but opens through its legacy route. Record keyboard focus traversal, screenshots, and pass/fail against the approved [mockup](../../product/zbot-desktop-ui-preview.html) in `docs/specs/desktop-session-shell/qa.md` during implementation. Also exercise existing Agents/Settings/Integrations tabs and editors with isolated seeded data, then return after reload to the same conversation. Desktop packaging, new administration features, real user configuration writes and hook execution are not exercised.

## Design (LLD)

### Design decisions

- The new shell is a presentation/navigation boundary over two existing execution hooks, not a third runtime path (AC2–AC3).
- The durable details projection is snapshot truth; WebSocket messages are live deltas keyed by stable event identity and reconciled after reconnect (AC4–AC6).
- Navigation is modular so later administration destinations can be added without changing session state ownership (AC11).

### Data & schema

- Reuse persisted session mode, messages, execution logs, and artifacts. Normalize `fast`/`chat` to `chat`, `deep`/`research` to `research`, and missing/other values to `unknown`; the UI prompts for an explicit mode when continuing an unknown session rather than guessing.
- Project only event ID/order/kind/redacted label and optional authorized memory ID. No raw recall text, tool arguments/results, filesystem path, or secret enters the details DTO or failure log. Source records require a structured citation/source-use reference with validated HTTP(S) destination. Use the Activity and Sources limits and safe-ID grammar in `session-details.yaml`; flag truncation in both cases. Omit records with invalid required IDs and omit invalid optional IDs. Missing records remain empty arrays (AC4–AC6, AC12).
- If current persistence cannot supply a stable required source-use or event identity, narrow the visible row set to what is provable. Any new persistent schema or retention policy requires approval under the spec boundary.

### Interfaces & contracts

- Add `GET /api/sessions/{sessionId}/details` under existing gateway session routing, per `contracts/openapi/session-details.yaml`; update transport types and client. Reuse the existing `SameOrigin` browser guard before any detail read and require a proven loopback bind for this endpoint, including Origin-less native callers; a LAN bind or missing bind proof fails closed with the contract's sanitized 403. Do not change other routes' reachability. Keep the served `gateway/src/http/openapi.yaml` synchronized and test that `/api/openapi.yaml` exposes `getSessionDetails` (AC2, AC4–AC6, AC12–AC13).
- Keep existing artifact-list and ID-based content routes as the sole Files data/open mechanism (AC7; `contracts/openapi/goal-artifacts.yaml`).
- Add `POST /api/sessions/chat` (no client-selected ID, mode or agent) and read-only `GET /api/sessions/{sessionId}/chat`, documented in served OpenAPI. Return `{sessionId, conversationId, created, isLive}`; creation returns 201, opening 200. Both use `SameOrigin` and `LoopbackBind` before access. Fixed JSON errors: invalid ID 400, denied 403, missing 404, incompatible mode/child or unrecoverable active routing 409, storage 500. Persist new sessions atomically with mode `fast`, root agent, Web source, queued status with no executions (`isLive: false`) and a server-owned metadata routing key equal to canonical session ID. Reuse existing `Session` metadata rather than migrating schema. Opening resolves a new session's recorded routing key, or the current reserved Chat's settings key; idle historical Chats can use canonical ID for a subsequent invocation. Derive `isLive` from nonterminal executions, not the initial session status. A running legacy session without a provable key is unavailable, not silently idle. Lifecycle reads/writes need only existing persistence; execution still reports runtime-unavailable through the existing fast invocation path, without a reset fallback (AC14).

### Component / module decomposition

- `apps/ui/src/App.tsx`: shared-shell route and legacy-route retention.
- `apps/ui/src/features/session-shell/`: shell layout, mode switch, session adapter, and Activity/Sources/Files inspector. Compose `useQuickChat` and `useResearchSession`; do not move their execution logic into one component.
- Existing Memory and Observatory data/components remain full-page destinations inside shared desktop chrome, with scoped presentation updates rather than duplicate data layers. `WardVaultExplorer` stays in legacy Research only (AC1–AC3, AC8–AC11). Knowledge URLs retain a validated local return destination so reloading still allows return to the selected session.
- `DesktopAdministrationPage.tsx` composes the existing `WebAgentsPanel`, `WebSettingsPanel` or `WebIntegrationsPanel` inside DesktopRail and one main landmark. Keep the existing page's h1 and tabs; a compact toolbar contains mobile navigation and the conversation return link. Generalize the current local-destination helper only for the five fixed knowledge/administration routes. Tab setters preserve `returnTo` while retaining existing tab URLs. Administration styling is scoped to `.session-shell--administration`, with shared semantic tokens rather than another theme (AC11).

### State & control flow

- Persisted selected-session mode wins on reopen. Default Chat mounts existing QuickChat with its no-argument bootstrap; selecting the Chat tab returns to `/session` without independent creation or reset, even from a selected Chat. Selecting Research from Chat keeps the existing new Research draft behavior. During active work, disable mode changes and New chat (AC2).
- Extend `useQuickChat` with an optional selected-session ID; no arguments retains legacy singleton boot/reset. Explicit New chat creates before mounting an ID-keyed hook; recent-session opening never creates. Reuse history/artifact/surface hydration and fast execute/Stop. QuickChat exposes an optional active-state callback for shell locking; no second hook or bootstrap is mounted. Cancel or ignore obsolete hydration and subscriptions on unmount; read `isLive` from selected Chat open response and reconcile active reopened snapshots until completion. Shell New chat and Chat-tab navigation never call `deleteChatSession`; the reused QuickChat's explicit confirmed Clear retains its existing behavior (AC14).
- Extend `useResearchSession` with optional selected-session and base-route options; default arguments retain legacy route behavior. Preserve its existing snapshot reconstruction, streaming, artifact and surface attribution logic (AC3).
- On open, hydrate the existing transcript and the details snapshot in parallel. Apply live deltas by stable ID, then reconcile against the server projection after completion or reconnect. Error states remain visible and never erase a known final answer (AC3–AC4).
- At narrow width, the inspector becomes an explicit reachable overlay/tab view, not hidden content (AC10).

### Failure, edge cases & resilience

- Details-fetch failure shows an unavailable state; it does not convert absent data into fabricated Activity or Sources. Legacy mode `unknown` is displayed as unknown until the user chooses an explicit new session (AC2, AC4–AC6).
- A LAN-bound gateway returns sanitized 403 for session details even to local requests; the shell labels Activity and Sources unavailable and continues to load Files through the existing artifact endpoint. This run does not correct the effective startup bind; explicit LAN binding and other routes retain their current behavior. The unresolved startup-default part of AC13 gates final promotion (AC7, AC13).
- Reject unsafe source URLs and unresolvable memory references server-side; client performs defense-in-depth scheme checking. File content still goes through artifact confinement (AC5–AC7).
- A failed Stop acknowledgement remains an error, not a stopped state (AC3).

### Quality attributes (NFRs)

- Use semantic tabs, labelled controls, visible focus, and keyboard operation for navigation, mode, Stop, and inspector (AC10).
- Keep the shell's three-column proportions and quiet hierarchy close to the approved mockup at desktop size; prioritize conversation/composer at narrow size (AC1, AC10).

## Tasks

### T1: Persisted session details return only attributable, redacted records

**Depends on:** none

**Tests:**
- `stub: draft (uncompiled)` — the Rust projector type does not exist yet; the red contract test below is syntax-valid and will compile after the T1 type is introduced. The gateway HTTP test uses the same fixture and asserts 200/400/403/404/500 before the handler exists. `// STUB: AC2 AC4 AC5 AC6 AC12 AC13`
  - `// STUB: AC2` `mode_spellings_and_unknown`: assert `fast`/`chat` → `chat`, `deep`/`research` → `research`, missing/other → `unknown`.
  - `// STUB: AC4` `activity_is_stable_ordered_bounded`: seed two attributable logs and one duplicate ID; assert a nonempty, ordered deduplicated result, then 501 logs → 500 rows with truncation.
  - `// STUB: AC5` `only_structured_citations_are_sources`: seed one structured used source, one uncited browse result, and unsafe/credential URLs; assert only the safe used source appears, then 101 valid sources → 100 rows with truncation. If no durable structured record exists, the persisted-store integration assertion is an empty array, never an inferred source.
  - `// STUB: AC6` `memory_activity_is_redacted`: seed a memory log with secret payload; assert a fixed label and no secret, and no memory link without a stable authorized ID.
  - `// STUB: AC12` `invalid_ids_and_errors_are_sanitized`: seed malformed required/optional IDs and a store error containing a path/secret; assert malformed rows/links are omitted and HTTP returns only a fixed bounded `error` string.
  - `// STUB: AC13` `details_guard_precedes_storage`: seed a read-counting store; assert mismatched Origin, LAN bind, and missing bind proof return 403 with zero reads; same-origin and Origin-less calls on loopback reach the store. The HTTP contract stub also checks 200/400/404/500 body shape and served OpenAPI operation ID. Earlier CLI test evidence is preserved, but does not establish the actual startup-default criterion; no network-default implementation or new acceptance test step is authorized here.
  ```rust
  // STUB: AC2 AC4 AC5 AC6 AC12 — persisted mode and attributable detail shape
  #[test]
  fn persisted_details_do_not_invent_provenance() {
      let details = SessionDetails::project("sess-1", Some("deep"), &[]);
      assert_eq!(details.mode, SessionMode::Research);
      assert!(details.activity.is_empty());
      assert!(details.sources.is_empty());
      assert!(!details.activity_truncated);
      assert!(!details.sources_truncated);
  }
  ```
- **TDD:** `gateway/gateway-execution/src/session_details.rs` tests for the four stored mode spellings plus unknown, stable event ordering/deduplication, redacted memory activity, absent-ID cases, cited/structured-used source selection, unsafe schemes and credential-bearing URL rejection (including percent-encoded userinfo), empty evidence, identifier sanitization, and boundary-plus-one truncation using the limits in `session-details.yaml` (AC2, AC4–AC6, AC12).
- **HTTP integration:** `gateway/src/http/session_details_tests.rs` tests the contract's 200/400/403/404/500 status and error-body shapes, same-origin loopback browser/native success, mismatched Origin and LAN-bind/missing-proof denial before session reads, no raw payloads in response or failure logs, and `/api/openapi.yaml` coverage for `getSessionDetails` (AC4–AC6, AC12–AC13).

**Approach:**
- Add `gateway/gateway-execution/src/session_details.rs` alongside `session_state.rs`, reusing durable message/log access and existing mode storage; add the handler in `gateway/src/http/sessions.rs` and route in `gateway/src/http/mod.rs`. Declare `#[cfg(test)] mod session_details_tests;` in `gateway/src/http/mod.rs` so the planned sibling HTTP test file is compiled and run. Update the gateway-served OpenAPI document with the same endpoint and error contract.
- Revalidate existing loopback/LAN endpoint guard tests. Preserve earlier daemon edits, but make no further network-default changes; the separately surfaced effective startup-default correction is outside this run's execution authority.
- Extend `apps/ui/src/services/transport/interface.ts`, `http.ts`, and `types.ts` for the contract. Keep source and memory links absent unless stable, authorized identifiers exist.

**Done when:** seeded HTTP tests match the contract, including empty/denied/legacy cases.

### T2: Independent Chat sessions preserve history and the existing fast runtime

**Depends on:** T1

**Tests:**
- `stub: draft (uncompiled)` — add real-router tests in `gateway/src/http/session_details_tests.rs` before handlers exist. `// STUB: AC2 AC14` Create twice with `POST /api/sessions/chat`; assert 201, distinct IDs, stored `fast`, canonical routing key, no execution and unchanged legacy settings/old messages/artifacts. Open by ID; assert 200, `created: false`, no mutation and exact selected ID. Reject invalid ID, missing row, Research/unknown/child rows, denied Origin/LAN/missing proof and failed store with bounded fixed errors. Verify active key recovery versus honest 409 when unprovable. Verify both OpenAPI operation IDs. Hook tests prove invocation failure (including unavailable runtime) remains an error without destructive fallback.
- `stub: draft (uncompiled)` — extend `useQuickChat.test.ts` and transport tests. `// STUB: AC3 AC14` Mount selected Chat A and then B: assert each selected history, fast execution uses B's session/routing IDs, active hydration stays running, Stop success/failure stays truthful, stale A bootstrap/events cannot affect B, neither mount nor new-chat action calls legacy init/delete, and no-argument legacy tests remain green.
- Construction stub: `it('creates distinct Chats without resetting the singleton', async () => { const a = await transport.createChatSession(); const b = await transport.createChatSession(); expect(a.data?.sessionId).not.toBe(b.data?.sessionId); expect(transport.deleteChatSession).not.toHaveBeenCalled(); });` Fill with the real handler fixture and hook mount; avoid a mock-only uniqueness assertion.

**Approach:**
- Add independent handlers in `gateway/src/http/chat.rs` and route them in `gateway/src/http/mod.rs`; use `state.state_service()` and existing settings/runtime access. Set session mode/metadata before one `create_session_from` write; no singleton lock/settings mutation is needed. Add matching transport methods and response types.
- Make selected-session bootstrap explicit in `useQuickChat`; preserve no-argument defaults and do not move runtime logic into the shell. Extend hydration reducer only as needed for authoritative active status; tests cover reload and existing behavior.

**Done when:** two shell Chats coexist with independently restored histories and existing fast execution, while legacy singleton tests remain green.

### T3: Chat and Research sessions render through one mode-pinned shell

**Depends on:** T1, T2

**Tests:**
- `stub: draft (uncompiled)` — the shell component is absent at PLAN; the Vitest test will be compiled and run during T2 red. `// STUB: AC1 AC2 AC3`
  - `// STUB: AC1` `recent_selection_restores_server_transcript`: seed a recent session with persisted Research mode and transcript; select it and assert both restore after remount.
  - `// STUB: AC2` `switch_is_pinned_and_idle_chat_resumes`: assert running switch is disabled, idle Chat selection resumes reserved Quick Chat without independent creation, explicit New chat creates a fresh session, and an unknown stored mode is displayed as unknown.
  - `// STUB: AC3` `both_modes_use_existing_execution_paths`: assert Chat invokes the quick-chat transport, Research invokes the research transport, Stop failure remains an error, and settled answer survives reload.
  ```tsx
  // STUB: AC1 AC2 AC3 — mode is pinned during a running turn
  it('restores persisted Research mode and prevents an active switch', async () => {
    render(<SessionShell initialSessionId="sess-1" />);
    expect(await screen.findByRole('tab', { name: 'Research' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tab', { name: 'Chat' })).toBeDisabled();
  });
  ```
- **TDD:** `apps/ui/src/features/session-shell/SessionShell.test.tsx` covers default reserved history with no selected-session hook argument, Chat-tab return from Research and selected Chat without creation/reset, explicit New chat independence, reserved active-turn locking, server-backed recents, selected transcript/mode restoration and legacy unknown mode (AC1–AC3). Write these compilable failing cases before production edits. Existing QuickChat component/hook tests preserve legacy clear, artifacts, streaming and Stop.
- Assert exactly one live QuickChat hook mount on default, selected Chat, explicit New chat and Chat-tab return (effect mount/cleanup instrumentation, not render-call counts); this detects a duplicate shell-owned bootstrap while permitting normal callback-driven rerenders.
- **End-to-end parity:** `e2e/playwright/ui-mode/session-shell.ui.spec.ts` compares Chat and Research streaming, Stop acknowledgement and failure, final answer, artifacts, and reload to their existing routes using seeded/live sessions (AC3). Existing hook and transport tests remain green.
- **Knowledge route regression:** before production routing changes, add a failing route test proving `/memory` and `/observatory` have the shared desktop rail and no legacy chrome, retain their full components, and return to the same selected session after knowledge-page reload. Validate any return destination as an exact local `/session` route; reject external, protocol-relative and malformed targets (AC8).
- `stub: true` — `apps/ui/src/App.extra.test.tsx` AC8 route tests are compilable and fail against existing routing for the intended missing desktop navigation/return-link behavior. `AgentTurnBlock.test.tsx` adds AC15 state-label assertions before presentation edits; computed colors remain browser checks, not jsdom assertions.
- **Subagent visual check:** seed running/completed/stopped/error and nested expanded cards, then assert rendered text/background contrast >=4.5 and control/focus boundary contrast >=3. Capture long names and expanded results at supported widths; existing turn and graph/memory tests remain green (AC15).

**Approach:**
- Add `apps/ui/src/features/session-shell/` with a small session adapter over `useQuickChat` and `useResearchSession`; keep their execution implementations intact.
- Compose the top-center switch, conversation, composer, server-backed recent-session list, and modular left rail. Reuse the existing session-list transport; selecting a recent item opens it by ID and hydrates its recorded mode and transcript. Keep legacy routes in `apps/ui/src/App.tsx` until parity gates pass.
- Mount the existing QuickChat for default Chat rather than duplicating its composer/transcript/artifact UI. An optional callback reports its running state; default mounting, history, confirmed Clear, artifacts and auto-scroll retain the existing behavior. Keep selected ChatConversation and ResearchConversation adapters. Browser parity demonstrates `/chat` history restored at `/session`, reload and idle Chat-tab return, while explicit New chat still creates a distinct session. Use the existing scoped palette and width bands; no new styling system.
- Extract the shared desktop rail into one small presentation component and render existing Memory/Observatory components under desktop knowledge routes. Keep conversation execution ownership in its existing adapters; use a validated URL return destination for refresh-safe restoration, without persisting another session store. Update scoped semantic tokens and knowledge/subagent presentation only. See `frontend-plan.md` for retrofit preflight and browser evidence requirements.

**Done when:** both modes use their current transport calls and mode-transition tests pass; full knowledge pages share desktop presentation with preserved actions and validated reload-safe return. Administration follows as independently reviewable T4/T5.

### T4: Existing administration routes share guarded desktop navigation

**Depends on:** T3

**Tests:**
- `stub: true` — `App.extra.test.tsx` Desktop administration routes pins desktop chrome, preserved page, reload-safe return and no session create/delete on all three routes. Uses the real CommissioningGuard with isolated transport responses: pending installations redirect to `/commission` without rendering any administration page. Those guard cases are already green and would fail if the moved routes omitted the guard; positive desktop/return cases are red against current legacy routes (AC11).
- `stub: true` — `features/session-shell/DesktopRail.test.tsx` pins the three fixed administration links with selected-session return state; current Agents/Settings lose that state and Integrations is missing. `navigation.test.ts` pins positive and unsafe cases against existing conversationDestination. The strict parser already passes and must remain the reused boundary (AC11).
- `stub: true` — existing `WebAgentsPanel.test.tsx`, `WebSettingsPanel.test.tsx` and `WebIntegrationsPanel.test.tsx` now contain failing AC11 tab-return cases against actual pages/router location; tabs must retain `returnTo`, including returning to the default tab. Preserve existing legacy Providers/Skills/Hooks/Connectors/MCP aliases and target tabs in route/browser regression coverage (AC11).
- **Visual/manual QA:** no stub (mode). Verify a selected idle Chat/Research returns after administration reload and tab changes in the isolated browser harness; no new session mutation occurs. Record route/composition evidence before T5.

**Approach:**
- Add one small `DesktopAdministrationPage` around the existing full pages, DesktopRail and recents, with a mobile navigation/return toolbar and one main landmark. Keep existing page headings and tabs. Move only `/agents`, `/settings` and `/integrations` outside WebAppShell, behind the same CommissioningGuard.
- Generalize the existing destination helper to five fixed page paths while reusing strict conversationDestination validation. Carry return state through administration sidebar links and existing page tab setters; preserve existing aliases and default-tab URL behavior. No new session store, data fetch layer, endpoint or CRUD logic. Keep other legacy routes unchanged.

**Done when:** route/guard/return/query tests and existing page action suites pass, and browser navigation/reload returns to the same selected session without session mutation.

### T5: Administration pages and editors fit the desktop palette and keyboard contract

**Depends on:** T4

**Tests:**
- `stub: true` — `components/TabBar.test.tsx` pins selected tab roving focus, ArrowRight/Home/End and visible corresponding panel; `components/Slideover.test.tsx` pins dialog naming, focus containment/Escape/restoration; `shared/ui/modal-overlay.test.tsx` pins containment/Escape/restoration for the existing embedding dialog. All are compilable before production edits; keep existing click/callback behavior and regression-test other consumers (AC11).
- **Goal-based:** no stub (mode). Run UI lint, typecheck and production build, plus all existing administration/editor/commissioning and neighboring Session/knowledge regression suites.
- **Visual/manual QA:** no stub (mode). Use current built UI, the existing same-origin isolated Playwright harness and seeded administration responses. Exercise all 3/5/2 tabs and existing create/edit/detail panels, masking/reveal, validation, visible save success/failure, destructive confirmation/cancel, customization conflicts and deep links. Assert rendered results and unchanged request payloads. Inspect route/open-editor long content at the declared width/height/rest/scroll matrix, computed contrast and keyboard focus/target size. Never mutate real user settings or contact external services; record the 14-field manifest (AC11).

**Approach:**
- Scope semantic theme/component classes to administration desktop chrome. Restyle existing headers, tabs, cards, forms, status and editor content, preserving actions and data contracts. Replace touched fixed editor-grid layouts with narrow reflow and reachable inner scrolling; keep footer actions accessible. Reuse native tokens rather than a second theme or new framework.
- Correct touched shared TabBar/Slideover/ModalOverlay keyboard/label/focus behavior with existing callback APIs and adjacent tests. ProviderSlideover and embedding panels retain current credential masking, confirmations and cancellation. Cover ModalOverlay's actual hard-coded dark surface through a scoped semantic modifier; do not change legacy consumers' palette.
- Review boundary: separate presentation/keyboard changes from T4 routing. If any task predicts above 2,000 reviewable behavior/test lines before production edits, stop and surface a dependency-ordered split before sealing; after sealing, substantive replan requires human direction. Do not broaden a task to fit review findings.

**Done when:** full existing administration tabs/editors fit the desktop palette, controls and long content remain reachable, action/regression tests pass, and rendered/accessibility observations plus warranted reviews are recorded.

### T6: Activity, Sources, and Files survive reload without invented rows

**Depends on:** T1, T3

**Tests:**
- `stub: draft (uncompiled)` — the inspector component is absent at PLAN; the Vitest test will be compiled and run during T3 red. `// STUB: AC4 AC5 AC6 AC7`
  - `// STUB: AC4` `snapshot_reconciles_live_events`: seed one live event with the same stable ID as the persisted snapshot; assert one final row after reload, in server order, and explicit empty/error/truncated states.
  - `// STUB: AC5` `sources_require_safe_server_evidence`: seed a cited HTTPS source, an uncited browse result, and a `javascript:` source; assert only the cited HTTPS link is present.
  - `// STUB: AC6` `memory_row_redacts_and_links_conditionally`: assert fixed memory label, no raw content, and no detail link when memory ID is absent or denied.
  - `// STUB: AC7` `files_use_artifact_ids_only`: seed a session artifact and an unrelated-session artifact; assert the first opens by the existing artifact ID route and the second is absent; HTTP negative tests retain confinement and symlink rejection.
  ```tsx
  // STUB: AC4 AC5 AC6 AC7 — no inferred source or memory content
  it('shows only persisted source evidence and a redacted memory event', async () => {
    render(<SessionInspector sessionId="sess-1" />);
    expect(await screen.findByText('Memory recalled')).toBeVisible();
    expect(screen.queryByText('secret fact payload')).not.toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'uncited.example' })).not.toBeInTheDocument();
  });
  ```
- **TDD:** `apps/ui/src/features/session-shell/SessionInspector.test.tsx` covers live/snapshot deduplication, redacted memory labels, no raw content, empty/error/truncated states, safe source schemes, and artifact-only Files (AC4–AC7).
- **End-to-end and HTTP integration:** `e2e/playwright/ui-mode/session-shell.ui.spec.ts` reopens seeded sessions and compares the settled panel with the persisted detail/artifact responses; gateway artifact tests reject other-session and symlink-escape opens (AC4–AC7).

**Approach:**
- Build the inspector and reconciliation logic in `apps/ui/src/features/session-shell/`; use `getSessionDetails` as snapshot truth and existing live events as transient deltas.
- Use `listSessionArtifacts` and the existing artifact content endpoint for Files; route authorized memory IDs into existing Memory detail only when resolvable.

**Done when:** settled live and reload views match, and negative link/file tests pass.

### T7: The default shell preserves knowledge destinations and legacy rollback

**Depends on:** T3, T5, T6

**Tests:**
- **Route and end-to-end tests:** `apps/ui/src/App.test.tsx` and `e2e/playwright/ui-mode/session-shell.ui.spec.ts` verify Memory search/inspect/correct, Observatory graph node interaction, Settings and existing agent routes, absent shell ward-explorer affordance, and the working legacy `/research` and `/research/:sessionId` explorer routes for a ward-backed Research session (AC8–AC9, AC11).
- **Visual/manual QA:** Record desktop/narrow screenshots and keyboard journey from new session through Stop, three tabs, Memory, Observatory, and back in `docs/specs/desktop-session-shell/qa.md`, including explicit mockup comparison and deviations (AC1, AC8–AC10).

**Approach:**
- Make the shell the default conversation entry in `apps/ui/src/App.tsx` only after the cross-cutting parity tests pass.
- Preserve bookmarked legacy `/chat`, `/research`, and `/research/:sessionId` routes and `WardVaultExplorer` for rollback; retain the migrated existing administration routes and aliases, but do not add new editors or hook controls.

**Done when:** route and accessibility checks pass and the manual QA record is attached to the implementation PR.

## Rollout

Ship the read-only details endpoint before making the new shell the default. The old Chat/Research routes remain available for rollback; no schema migration or desktop installer is part of this slice. If the durable stores lack evidence for a row, ship the honest empty state rather than widening persistence without approval. Browser and CLI remain supported clients.

## Risks

- Existing messages/logs may not carry durable source-use records or memory IDs; the panel can legitimately be sparse. Do not fill gaps from text.
- Dual live/snapshot event paths can duplicate rows unless stable IDs and ordering are tested.
- Hidden legacy explorer could become accidentally unreachable if route cleanup is bundled into the shell change.
- Existing `recalled_facts` contains raw content and must not be reused directly in the new details response.

## Changelog

- 2026-09-27: Initial plan for the approved UI-plus-minimal-provenance slice; user confirmed the read-only HTTP detail contract and existing live WebSocket stream.
- 2026-09-27: User-approved replacement plan adds independent Chat lifecycle before the shell, preserving completed T1 and all legacy singleton behavior.
- 2026-09-27: User-approved correction makes default Chat and Chat-tab navigation reuse reserved QuickChat; explicit New chat stays independent. Tracking-only reset preserves earlier implementation/evidence; revalidate T1/T2 before T3.
- 2026-09-28 UTC: User-approved presentation amendment expands AC11 into dependency-ordered T4 administration navigation and T5 presentation/keyboard slices, preserving data/action contracts. Original inspector/rollout tasks are now T6/T7; their outstanding obligations are unchanged. Tracking-only reset preserves completed work; revalidate T1/T2 and existing T3 presentation before continuing. Build-strategy approval remains a separate gate.
