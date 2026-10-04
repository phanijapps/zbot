# Desktop knowledge and subagent UI evidence

## 2026-09-28 — administration implementation (browser QA pending)

User explicitly authorized implementation with unit/build verification while local socket/browser execution is unavailable. This evidence covers AC11 administration code only, not whole-spec acceptance or rendered layout verification. Existing knowledge/Chat capture evidence below predates these changes and is not reused as administration evidence.

| Required field | Evidence / status |
| --- | --- |
| routes | `/agents`, `/settings`, `/integrations`; legacy `/providers`, `/skills`, `/hooks`, `/connectors`, `/mcps` alias tests. Commissioning and strict session-return routing retain existing guards. |
| viewports | Intended declared channels `[320,760)`, `[760,1100)`, `[1100,infinity)`, minimum 320, no discarded breakpoints; required heights 600/900. No administration viewport captures attained. |
| browsers | None in this pass: isolated harness fails when creating its loopback socket with `Operation not permitted`. |
| states | Unit-tested content/navigation, remount/return, invalid return, commissioning denial, tabs, existing page/editor loading/errors/actions/confirmations, customization conflicts, keyboard dialog containment/Escape/restore and mobile navigation focus/inert background. Computed long-content/high-zoom/reduced-motion and visual state coverage remain pending. |
| screenshots | No new administration screenshots. Required route and representative open-editor rest/scroll matrix is pending. |
| inspection observations | `skipped-no-browser` / no pass claim. Build succeeds, but no rendered inspection, overflow, clipping or visual-fit acceptance has been performed. |
| a11y result | Keyboard and accessible dialog/tab contracts pass in unit tests. Native customization file selection uses aria-pressed, not invalid aria-selected on buttons. No current axe, computed contrast, manual 2.5.8 target-size or 2.4.13 AAA focus inspection. WCAG 2.2 AA gaps 2.4.11/2.5.7/3.2.6/3.3.7/3.3.8 remain unverified. |
| perf result | Production build passes; main JS 2,388.15kB / gzip 654.35kB; existing >1000kB chunk warning remains. No CWV/Lighthouse or runtime performance claim. |
| console/network result | No browser console/network observation. No new backend, request payloads, credential storage or third-party endpoints; existing action suites verify current mocked transport behavior only. |
| analytics events | No new measurement or analytics calls; existing behavior unchanged and browser firing unverified. |
| known exceptions | User-approved browser deferral; cached origin/develop accepted with remote freshness unverified. Full UI suite retains registered pre-existing ResearchPage Stop assertion failure (workspace backlog pre-existing-research-stop-assertion); not suppressed or changed. |
| unverified items | Browser visual/a11y/action/payload preservation journeys and captures, effective startup-default AC13, original Session live parity, T6 inspector and T7 rollout. No full-spec or runtime-parity completion claim. |
| security/privacy review status | Presentation includes existing credential-bearing provider/MCP forms. Guard and return-helper preservation tests pass; scoped implementation security review Clean. Only synthetic unit fixtures used; no user configuration or external provider QA writes. |
| reliability/recovery status | Existing action/error/conflict tests pass; production browser recovery remains pending. Quality review Clean after unknown-tab and closed-backdrop regressions; runtime SLO/alert ownership remains unchanged. |

Implementation scope: reuse full existing pages and DesktopRail; validated return URLs survive reload/tab changes; administration-only styles and reflow; shared tab/dialog keyboard lifecycle with unchanged action callbacks. No new agent, hook, settings or execution capability. Semantic test correction changes FileList's expected ARIA attribute to aria-pressed, preserving selected state. The selected-Chat hook-count regression now awaits its effect mount before inspecting the count, retaining the maximum-one-live-hook assertion; no execution code changed.

Final bounded code gates: 305 tests across 32 files pass; typecheck/build and lint (zero errors / 19 existing warnings) pass. Full UI suite: 1438 passed and one registered pre-existing ResearchPage Stop assertion failed. Adversarial, security, quality and frontend implementation reviews have no remaining code findings; rendered frontend lens remains skipped-no-browser. Current assets rebuilt in root `dist/`, main `index-SLPdIWzr.js`. No daemon restart or user configuration mutation. Broader spec remains Implementing/T3.

---

Scope: the T3 Memory/Observatory retrofit and subagent contrast amendment. This is not completion of the desktop spec: inspector wiring (T4), remaining live execution/manual parity checks, default promotion (T5), and the effective startup-network-default correction remain outstanding. Engine/cohort stay in implementation, wave T3.

| Required field | Evidence / status |
| --- | --- |
| routes | `/session`, `/session/:id`, `/session?mode=research`, `/memory`, `/observatory`; legacy `/chat` and `/research` as execution parity controls. Capture records omit query and fragment. |
| viewports | Declared channels `[320,760)`, `[760,1100)`, `[1100,infinity)`; minimum 320; none discarded. Captured 320, 760, 1100 at heights 600 and 900; additional 1280×800 and 720×800. Shell media endpoints 759/1099 preserve these bands. |
| browsers | Playwright Chromium only. Firefox/WebKit and native desktop-host rendering not verified. |
| states | Real isolated Chat/Research final answer and reload; knowledge navigation/return after reload, including Memory Graph new-tab/reload/return to the same conversation; seeded running/completed/stopped/failed subagents, expanded long results; Memory content/empty-selection, scope selection, deletion confirmation unit tests; graph populated and empty/error unit cases; invalid return destinations and unavailable session/recents unit cases, including success-then-denied/rejected recent refresh. Remaining live Stop/artifact/manual mutation cases are not claimed. |
| screenshots | `desktop.png`, `narrow.png`; per-route `<session|memory|observatory>-<320|760|1100>-<600|900>-<rest|scrolled>.png` in `e2e/playwright/test-results/ui-mode-session-shell.ui-k-13e80-readable-at-each-shell-band-chromium/`. Browser attachment `layout-and-contrast` records actual viewport, document offset/scrollability, selected inner scroll region/offset, and computed contrast. No private user vault is used. |
| inspection observations | `completed` / `pass` for the scoped seeded knowledge/subagent presentation, provisional main-agent inspection, not an independent visual sign-off. Inspected all 31 final rest/scroll captures separately across the 18 route/viewport combinations. Initial captures found mobile controls visible on desktop, inherited graph/Memory styling, and narrow Memory metadata squeezing text; corrected. Final captures show wrapped long results, visible labelled statuses, no document horizontal overflow, a reachable composer/curation region, a quiet graph surface and wrapped legend. The visible inspector placeholder is explicitly unfinished T4, not a passing functional inspector claim. |
| a11y result | Computed subagent text/status ≥4.5:1 and card boundary ≥3:1; active Memory controls ≥4.5:1 asserted at all six viewports. Final axe 4.13.0 `wcag2a`, `wcag2aa`, `wcag21aa`: zero violations on populated `/session/:id`, `/memory`, `/observatory` at 1100×900. Corrected count-badge contrast, Graph link inside tablist, and nested Memory inspect/delete controls. Small Memory mode/chip/link targets raised to 24px; subagent header ≥32px. Manual focus/target coverage is partial. WCAG 2.2 gaps 2.4.11, 2.5.7, 3.2.6, 3.3.7, 3.3.8 and comprehensive 2.4.13 AAA enhancement remain unverified. |
| perf result | Production build/typecheck passes. Final main JS 2,385.25kB / gzip 653.63kB, pre-existing Vite >1000kB warning. No CWV/Lighthouse mobile/desktop measurements; no performance claim. |
| console/network result | Real daemon is same-origin and explicitly local-only in an isolated fresh vault; provider calls replay against local mock LLM with zero drift. No new third-party calls. Existing SPA deep-link document responses may return 404 while index renders. Comprehensive console/network-error inventory is not yet verified. |
| analytics events | No new analytics or measurement calls. Existing completion-event measurement was not separately audited. |
| known exceptions | This report does not establish WCAG 2.2 AA conformance, performance-floor completion, or an independent design-review pass. Unavailable experience/frontend-reviewer roles were recorded in frontend-plan.md. These are visible gaps, not accepted shipping waivers. |
| unverified items | Browser engines beyond Chromium, keyboard-only/mobile-overlay focus recovery, graph drag alternatives, error-monitoring/SLO ownership, real live Stop/artifact parity, remaining inspector functionality, and actual startup bind default. Explicit fixture localOnly is not default-startup evidence. |
| security/privacy review status | Existing Memory/graph services/authentication retained. Only local safe conversation return destinations accepted; no new storage/service/privileged operation. Pre-execution and implementation security review clean (preexec_security agent), including the Memory semantic corrections. |
| reliability/recovery status | Recent-list failures clear stale choices and use an unavailable state; invalid/unavailable conversations are preserved and labelled, not reset/deleted. Existing knowledge request errors retained. Implementation recovery/test review clean (knowledge_quality agent); operational alerting and SLO ownership not asserted. Same-origin browser harness always rebuilds current sources and cleans up on failure. |

Final local gates: 83 UI tests across ten integration/route/knowledge/turn/recovery suites, including independent inspect/delete and return-link/recent-recovery construction tests (red before fixes, green after); lint zero errors with 20 existing warnings; production typecheck/build and diff check pass. Final browser run: **2 passed (1.1m)**, with a mandatory fresh UI build, including answer persistence, knowledge and Graph-popup return/reload, 18 viewport combinations, computed contrast and the full classic-AA axe audit.

Browser/a11y command, from `e2e/playwright`:

```bash
DESKTOP_AXE_SCRIPT=/home/videogamer/.npm/_npx/e003b6b07d062486/node_modules/axe-core/axe.min.js npx playwright test ui-mode/session-shell.ui.spec.ts
```

The script path is the local npm cache created by `npm exec --yes --package=@axe-core/cli -- axe --version`, not a project dependency. On another machine, locate that installation's `axe-core/axe.min.js` and supply its absolute path. Without this environment variable the browser suite still checks layout/contrast, but does not claim an axe audit.

## Default Quick Chat correction

Scope: reserved QuickChat reuse at `/session` and idle Chat-tab navigation, with independent explicit New chat. Research, runtimes, storage, reset protocol, user settings and default-route rollout are unchanged. This is a bounded T3 correction, not a shipped spec.

| Required field | Correction evidence / status |
| --- | --- |
| routes | `/session`, selected Chat/Research `/session/:id`, legacy `/chat` and `/research`; knowledge regression routes above retained. |
| viewports | Same declared minimum 320 and bands bounded by 760/1100; 320,760,1100 × 600,900 for reserved history and empty state; none discarded. |
| browsers | Chromium only. |
| states | Default reserved history, empty, reload, tab return from Research/selected Chat, explicit independent New chat, running lock, failed Stop lock callback, failed creation retaining transcript. Existing confirmed Clear/artifact/Stop tests retained. |
| screenshots | 18 `quick-<history|empty>-<320|760|1100>-<600|900>-<rest|scrolled>.png` captures under `e2e/playwright/test-results/ui-mode-session-shell.ui-r-cf55d-ith-history-and-empty-state-chromium/`. JSON reporter retains actual capture records, not only filenames. |
| inspection observations | `completed` / `pass`, provisional main-agent visual judgement. Inspected all 18 final captures separately using retained actual viewport/document and inner-scroll metadata. Both history and empty states keep the top-center switch and composer visible at every width/height; long history is confined to its scrolling region, first content is readable at rest and the final evidence row is reachable at the end. No occlusion, top clipping, horizontal overflow, illegible text or crowded controls found in this scoped capture set. The inspector remains the explicitly unfinished T4 placeholder. Existing knowledge/subagent inspection above preserved. |
| a11y result | Browser suite passes classic-AA axe on both default Chat states plus three knowledge/subagent routes; existing WCAG 2.2/manual-focus/target gaps above remain, no conformance claim. |
| perf result | Fresh production build/typecheck succeeds; no new dependency. CWV remains unmeasured. |
| console/network result | Real isolated same-origin/localOnly daemon proves `/chat` history restored identically at `/session` and after reload/tab return. No independent creation until explicit New chat; exactly one distinct session then created. Mock-provider drift zero. Seeded layout checks are not live execution evidence. |
| analytics events | No changes/new third-party calls. |
| known exceptions | Earlier T3/T4/T5/AC13 gaps preserved. Repo-wide status lint reports unrelated pre-existing `exec-consolidation-waves` Status Drafting; captured in backlog, not relabelled here. Existing contract backlink warning remains warn-only. |
| unverified items | Same manual/live Stop/artifact, browser-engine and performance gaps above. |
| security/privacy review status | No new boundary/API/storage/reset or execution semantics; prior security review preserved. Current correction security-design/implementation pass not warranted. |
| reliability/recovery status | Single live QuickChat hook guarded by mount/cleanup tests on default/selected/new/tab-return; optional active callback preserves failed Stop lock; failed creation keeps current history visible. Independent quality review Clean after adversarial Clean; operational SLO ownership remains unverified. |

Local gates: **314 tests / 19 suites** pass; lint zero errors / 20 existing warnings; typecheck and diff checks pass. Real/seeded browser suite **3 passed (1.2m)**; final JSON-evidence run repeats these checks with mandatory current-source production build. The first attempt found a test-only innerText/textContent mismatch and a seeded Memory fixture's dependency on the preceding successful test; fixed comparisons and made the fixture self-contained rather than weakening application checks.

Final retained-evidence run: **3 passed (1.2m)**. `e2e/playwright/test-results/desktop-shell-report.json` contains `quick-chat-layout` and `layout-and-contrast` attachment bodies. All 18 Quick Chat captures have document offset 0 and document-scrollable false; history inner end offsets at 600/900 heights are 2049/1749 (width 320), 1132/832 (760), and 1188/888 (1100). Empty states have no inner scroll. Axe returns zero violations for both default states. Current built asset `dist/assets/index-DNRN8H2F.js` matches the user daemon's served root; no daemon/configuration change or private-data mutation was made.
