# Existing administration desktop retrofit

**Scope:** desktop-session-shell, production retrofit, AC11/T4–T5. The user approved the three-page scope and tracking-only reset; build-strategy approval remains a separate work-loop gate. Existing code and verification artifacts are preserved. Original inspector/rollout T4/T5 become T6/T7 with unchanged obligations.

## Scope and boundaries

- Agents: My Agents, Skills Library and Schedules, including existing create/edit/detail panels.
- Settings: Providers, General, Logging, Advanced and the existing Customization editor, including provider and embedding panels.
- Integrations: Tool Servers and Plugins & Workers, including existing detail/create/edit panels and authentication status UI.
- Preserve existing page actions, endpoints, payloads, validation, confirmations, defaults, credential masking, persistence and authorization. No live user configuration writes during QA.
- No new agent builder, hook control/execution, desktop host, dependency or administration data layer. Mission Control, Vault, experimental Graph and commissioning presentation remain outside this amendment. Legacy conversation routes and hidden ward compatibility remain unchanged.

## Reuse and root cause

Engram retrieved `WebAgentsPanel`, `WebSettingsPanel` and `WebAppShell` in zbot. Its graph did not resolve their callers, so the current App routes and page sources were read directly. `/agents`, `/settings` and `/integrations` remain inside WebAppShell; the desktop palette is scoped to `.session-shell`, so these routes inherit legacy navigation and dark tokens. AC11 previously required only links.

Reuse `DesktopRail`, `useRecentSessions`, `conversationDestination`, the existing full administration pages, TabBar, Slideover, ProviderSlideover and AgentEditPanel. Do not copy their data fetching or CRUD logic. Preserve Settings' existing Customization file editor and conflict handling, rather than treating it as a future feature or creating a replacement.

## Design preflight

Surface slug: `desktop-session-shell`. Aesthetic reference: the approved ChatGPT-like desktop mockup `docs/product/zbot-desktop-ui-preview.html`: warm neutral surfaces, compact sidebar, restrained brown accent, high-contrast text and controls, content-first panels. Administration is a full-page destination; do not add a Chat/Research mode switch to configuration pages or an Activity/Sources/Files inspector there.

Design handoff: no `[design]` section configured; repository-root and user-profile `agentbundle-layout.toml` absent. XD genre routing: skipped (experience-design pack absent). Design-review, experience-reviewer and frontend-reviewer unavailable; main-agent visual inspection is provisional, not independent design certification. Adversarial, secure-design and quality review remain required at their applicable gates.

### Brownfield inspection

| Item | Finding / treatment |
| --- | --- |
| Preserve | Existing tabs, query deep links, CRUD/configuration actions, credential masking, provider selection/test, schedules, MCP authentication, customization conflicts and cancellation. No schema/API/runtime changes. |
| Duplicated systems | Legacy chrome differs from desktop; compose existing DesktopRail in a small administration wrapper. Keep one destination inventory and one palette. |
| Hard-coded values | Existing pages/editors contain inline grid widths and layout decisions. Replace only touched layout decisions with native token-driven classes; no whole-page logic rewrite or unrelated cleanup. |
| Accessibility debt | TabBar lacks roving arrow-key navigation; shared Slideover lacks dialog labelling/focus containment/restoration. EmbeddingProgressModal's ModalOverlay only sets initial focus, not a complete trap or return-focus contract. Touched tab/dialog paths must have usable keyboard and focus behavior, with regression tests for their legacy consumers. Do not claim existing dialogs already meet APG. |
| Responsive debt | Customization's fixed 260px + 1fr grid and multi-column editor fields risk overflow after the sidebar reduces available content width. Scope reflow to administration content; let long bodies scroll and keep editor actions reachable. |
| Regression risk | Shared tabs/dialogs/styles render elsewhere. Scope palette/layout changes to administration desktop chrome; shared keyboard corrections retain existing click/callback contracts and require adjacent regression tests. |

### Token seed

Use the existing native semantic contract in `apps/ui/src/styles/theme.css`, inherited from `.session-shell`, not a second `--ds-*` system. This is the seed already approved for desktop composition:

```css
.session-shell {
  --background: #ffffff;
  --background-surface: #f7f7f5;
  --background-elevated: #f0efec;
  --foreground: #222b3b;
  --muted-foreground: #566171;
  --primary: #a75935;
  --primary-foreground: #ffffff;
  --primary-hover: #824226;
  --border-hover: #52667e;
  --success: #25613c;
  --warning: #78500d;
  --destructive: #a0322a;
}
```

Existing component aliases must resolve to these scoped roles; any additional semantic role belongs in theme.css. Components.css owns reusable layout, React owns structure/logic. Verify composed text/background contrast >=4.5:1 and necessary control/focus boundaries >=3:1; do not assume that the existing low-emphasis decorative border is a sufficient input boundary. Status retains text/icons, not color alone. No new decorative motion.

## Build strategy

1. Keep the current T1/T2 code and revalidate their contract suites before advancing replacement tracking. Revalidate existing Session/Memory/Observatory/QuickChat presentation and route tests before modifying administration.
2. In T4, use the materialized failing route/navigation/tab tests. Add a small `DesktopAdministrationPage.tsx` with the existing DesktopRail, recents, mobile navigation button, conversation-return link and one main landmark. Render each existing full page directly; keep its h1 and tabs instead of adding duplicate chrome. Move only the three routes outside WebAppShell, retaining CommissioningGuard. Existing positive/negative return-parser tests and the three real-guard pending-installation tests already pin preserved behavior.
3. Extend the existing destination helper to five fixed knowledge/administration paths and carry only a validated local conversation `returnTo`. Preserve that value when page tabs update query parameters. Keep existing aliases and active-tab semantics. Route focus goes to a main landmark or the loaded page heading; opening a menu/dialog must not leave hidden background navigation actionable.
4. In T5 (depending on T4), apply administration-scoped layout/semantic styling to page headers, tabs, cards, forms, status, empty/error states and editors. Add narrow reflow classes to touched fixed editor grids. Use materialized shared keyboard tests to correct touched tab/dialog labels, focus containment and restoration without replacing request/action logic. Preserve confirmations and credential masking.
5. Run UI lint, typecheck, regression suites and production build, then real built-UI browser journeys using the isolated same-origin daemon and seeded administration responses. Inspect required captures and record the production evidence manifest. Adversarial review precedes warranted specialist review; apply findings and reverify.

Primary touch set: `App.tsx`, `features/session-shell/{DesktopAdministrationPage,DesktopRail,navigation}`, the three `Web*Panel.tsx` tab setters, `styles/{theme,components}.css`, and touched editor layout/shared tab/dialog files (`components/{TabBar,Slideover}.tsx`, `shared/ui/modal-overlay.tsx`, `agent/AgentEditPanel.tsx`, `settings/{ProviderSlideover,EmbeddingProgressModal}.tsx`, `settings/customization/{CustomizationTab,FileEditor}.tsx`). Modify additional same-page editor markup only when rendered checks demonstrate it is needed. Each change must trace to AC11, not an unrelated refactor. ModalOverlay has a hard-coded dark surface; a desktop-scoped semantic modifier must cover the actual embedding dialog without changing legacy consumers' theme.

Tests: App route suites, new navigation/DesktopRail tests, three page suites, existing AgentEditPanel/provider/embedding/customization/Integration suites, materialized shared TabBar/Slideover/ModalOverlay keyboard cases, and the existing `e2e/playwright/ui-mode/session-shell.ui.spec.ts` or a sibling administration spec using the same harness. Goal-based tests use the real build/typecheck; visual QA has no stub (mode). TDD route/query/keyboard artifacts exist at PLAN and must be observed red for missing behavior before production implementation, while preservation invariants remain green.

Administration is split before sealing: T4 routes/guard/return/tab semantics depends on existing T3; T5 scoped presentation/editor reflow/keyboard and browser verification depends on T4. Each task predicts below 2,000 reviewable behavior/test lines. Review shape is MIXED; review routing separately from presentation/keyboard. If a task predicts above that threshold before production edits, stop and surface a dependency-ordered split rather than silently expanding it. After sealing, substantive task changes require human direction. No universal page framework. Overall existing spec remains multi-loop, not a single completion claim.

## State and preservation matrix

| State | Migration obligation / evidence |
| --- | --- |
| Loading | Existing load states stay visible; shared navigation/return remains available. Status is labelled/announced on touched paths. |
| Empty / first-run | Retain existing create/configure affordances; no invented records or alternative data store. |
| Content / success | Every listed tab and editor remains full functionality; seeded save/create/edit visibly updates existing page state, with unchanged payloads. |
| Error / offline | Preserve existing error and unsaved-input behavior; verify failed saves and retry/recovery where currently available. Do not expand transport error semantics under a styling migration. |
| Partial / large-data-set | Preserve existing filtering/list behavior and explicit existing limits. Exercise multiple long records; do not silently truncate records as a layout fix. |
| Disabled / blocked | Preserve disabled controls, credential/provider prerequisites and explanatory text; maintain readable labels. |
| No-results | Preserve query and existing recovery; test search returning no Agents/Skills/Schedules. |
| Permission/denied | Keep CommissioningGuard and server-side denial handling. A new wrapper or deep link never bypasses either; no role/auth policy change. |
| Destructive-confirmation | Preserve existing confirmations and safe cancel/default. Test seeded confirmation/cancel and ensure navigation alone never triggers mutation. No new deletion capability. |
| Long-content | Long names, URLs, JSON/config text and multiline editor content wrap or scroll inside their own regions; actions remain reachable. |
| Keyboard-only | Operate sidebar, tabs, editors and confirmation controls with visible focus; labelled dialog trap/Escape/return-focus and tabs arrow/Home/End navigation. |
| High-zoom | Reflow at narrow-equivalent viewport/200% zoom; no lost form actions or whole-page horizontal overflow. |
| Reduced-motion | No new motion; touched inherited motion respects the existing reduced-motion desktop rules. |

This retrofit does not introduce new data states; any existing backend failure/recovery defect beyond presentation is surfaced separately rather than changing persistence behavior silently. State checks must record observed exceptions explicitly, not equate an inherited implementation with verified coverage.

## Browser evidence and security

Use only isolated seeded/mocked administration responses with the real current built UI and isolated daemon configuration. Never screenshot real provider keys, user prompts/config files or private administration content. Never save user network settings, test external providers, trigger real schedules or authorize external MCP OAuth as QA. Seeded successful/failed responses demonstrate UI/payload preservation, not external service availability.

Required routes: `/agents`, `/settings`, `/integrations`. Exercise all 3/5/2 tabs and existing create/edit/detail panels; preserve tab deep links and legacy aliases. Preserve an idle selected Chat and Research destination through administration navigation, tab changes, reload and return. Reject external, protocol-relative, credential-bearing, traversal, overlong-ID and malformed return targets via `conversationDestination`, before rendering any navigation target; fallback `/session` introduces no session creation or reset. Validate return state independently of arbitrary tab/query data. Preserve masked-key/reveal behavior without copying a key into links, storage, console or error UI.

Supported minimum 320 CSS px, existing shell breakpoints 760 and 1100. Required channels `[320,760)`, `[760,1100)`, `[1100,infinity)` at widths 320/760/1100; no discarded breakpoints. Capture 600/900 heights at rest and scrolled (or measured non-scrollable) for every route/channel. At those same sizes capture representative open long editor content for each route, using synthetic data. Additional tab/state checks may be interaction/assertion coverage rather than a screenshot of every tab at every size. Record actual route without query/fragment, width, height, scroll offset and scrollable status; record inner panel/body offset as additional evidence. Every captured size requires its rest/scroll pair. Inspect each required capture for clipping, occlusion, overflow, illegibility, target size and crowding; screenshots alone are not inspection.

Run structural/accessibility checks and computed contrast, plus keyboard/focus/target-size checks. Record classic-AA axe results and manual 2.5.8 target size/2.4.13 AAA focus enhancement outcomes, and explicitly retain the WCAG 2.2 gaps 2.4.11/2.5.7/3.2.6/3.3.7/3.3.8 unless separately exercised. No full WCAG certification, independent design review or unmeasured CWV claim.

Write all 14 production evidence fields in `qa.md`: routes, viewports, browsers, states, screenshots, inspection observations/result-state/verdict, a11y, performance, console/network, analytics, known exceptions, unverified items, security/privacy review status, reliability/recovery status. Existing analytics behavior remains unchanged; no new third-party call. Security reviewer verifies unchanged guards/credential behavior and local return validation; quality reviewer verifies preservation/recovery/test coverage. Rendered main-agent inspection stays provisional because design specialists are unavailable.

The real default-startup bind, remaining Session live parity and Activity/Sources/Files wiring remain separate outstanding original criteria. An explicitly local-only test daemon does not prove startup-default AC13. No default-route promotion, commit/PR or whole-spec Shipped status in this amendment's planning pass.

## Planning verification

Before adding construction tests, the existing administration page/editor suites plus desktop navigation/App routes pass: 213 tests, 19 suites, 4.56s. This is baseline preservation evidence, not migration acceptance. PLAN now materializes AC11 App/DesktopRail/page-tab and shared keyboard tests; the real CommissioningGuard is exercised with pending and complete readiness responses. The strict existing return helper has positive/negative contract cases. TypeScript compilation passes. Migration-specific tests are intentionally red until implementation; prior cases must remain green. `git diff --check` passes. Repository status lint still reports the already-registered unrelated `exec-consolidation-waves` invalid Drafting status and the existing goal-artifacts backlink warning; neither was introduced or fixed here. Rendered migration evidence remains outstanding, not a completed planning claim.

Construction validation: 15 expected failures / 115 passing cases in 9 suites (4.76s); failures name missing admin chrome/return, sidebar destination state, page tab query preservation and shared keyboard behavior. Three real-guard pending-installation cases and all strict return-parser cases pass. Lint: 0 errors / 20 existing warnings. No production implementation or browser migration acceptance yet.
