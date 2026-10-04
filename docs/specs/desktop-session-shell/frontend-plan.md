# Desktop shell frontend retrofit

Scope: `desktop-session-shell`, production UI. Reuse existing Memory/Observatory behavior and Research turn components; unify presentation and navigation. User references: approved ChatGPT-like warm-neutral mockup (`docs/product/zbot-desktop-ui-preview.html`) and explicit request for high-contrast subagent cards. No new external assets or analytics calls.

The user-approved administration expansion reuses full existing Agents, Settings and Integrations pages and editors. Its brownfield findings, scoped token seed, state matrix, guard/credential constraints and route/editor capture requirements are maintained in [administration-amendment.md](administration-amendment.md). Existing knowledge/default Chat verification is preserved; it is not evidence that these additional pages have been migrated.

Design handoff: no `[design]` section configured (repository and user-profile layout files absent). Approved mockup supplies direction. XD genre routing: skipped (experience-design pack absent). Creative-direction/design-review, experience-reviewer and frontend-reviewer are unavailable; main-agent visual inspection is provisional, with adversarial/security/quality review still required.

## Brownfield inspection

| Item | Finding / preserve |
| --- | --- |
| What to preserve | Memory command-deck filters, search, inspection and correction; graph filtering, entity inspection and canvas controls; Research nesting, expand/collapse and safe tool audit. |
| Duplicated systems | Knowledge routes currently render under legacy top navigation; extract one desktop rail rather than creating another navigation inventory or data service. |
| Hard-coded values | Existing graph and Research have inline color/size decisions; new presentation uses scoped semantic theme tokens. Change only touched presentation, not unrelated graph simulation logic. |
| Accessibility debt | Existing nested subagent headers have icons without visible state text; add status text. Knowledge routes need route-heading focus and explicit return navigation. Retain keyboard interactions. |
| Responsive debt | Memory's three columns use viewport breakpoints despite narrower shell content. Use shell-scoped layouts and reachable scrolling; graph entity detail must remain accessible at narrow width. |
| Regression risk | Shared components also render on legacy Research; scope palette/layout changes to desktop chrome. Existing turn, graph, Memory and legacy navigation tests remain required. |

## Token seed and contrast

Follow the repository's native token names (`theme.css`), not a second competing token system. Rebind legacy `--color-*` aliases at the desktop scope so inherited aliases cannot retain dark-root values.

```css
.session-shell {
  --subagent-surface: #eef3f8;
  --subagent-foreground: #25344b;
  --subagent-muted: #48576a;
  --subagent-border: #52667e;
  --success: #25613c;
  --warning: #78500d;
  --destructive: #a0322a;
  --blue: #254f82;
}
```

These are planned semantic values, not an assertion of verified contrast. Browser checks compute actual foreground/background and control/focus contrast after composition. Status labels and icons remain present independently of color. No opacity reduction on completed card content.

## State coverage

| State | Treatment / verification |
| --- | --- |
| Loading | Preserve knowledge component loading states; route header and return link remain present; selected conversation opening is announced. |
| Empty / first-run | Existing memory/graph empty states remain explicit, with existing creation/recovery actions. |
| Content / success | Full knowledge tools remain, not summary placeholders; return navigates to canonical session ID. |
| Error / offline / denied / blocked | Existing request errors remain visible; keep route navigation usable; never reset/delete a session as recovery. No new privileged operation. |
| Partial / large-data-set | Preserve existing paging/filtering; recents explicitly represent a bounded recent window. |
| Disabled | Active conversation mode switching remains unavailable; card labels remain readable even while controls are disabled. |
| No-results | Preserve search query and existing no-results recovery. |
| Destructive-confirmation | Preserve existing Memory deletion confirmation; navigation introduces no destructive action. |
| Long-content | Wrap long names and request/results; scroll long bodies; nested cards fit within the tape. |
| High-zoom / keyboard-only | Shared navigation and return links operable; focus visible; page heading receives route focus; narrow content reflows. |
| Reduced-motion | No new motion; cancel inherited decorative motion in the desktop scope for reduced-motion preference. |

## Browser verification and evidence

Supported minimum width 320 CSS px. Declared shell breakpoints 760 and 1100; required bands `[320,760)`, `[760,1100)`, `[1100,infinity)`, captured at 320, 760 and 1100. For `/session`, `/memory` and `/observatory`, capture heights 600 and 900 at rest and scrolled where scrollable. Record route without query/fragment, actual width/height, actual vertical offset, and whether scrolling is possible. Fixed viewport surfaces may scroll inside their content region; name that region and record its actual offset as additional evidence. Use isolated seeded data, never screenshots of private user memory.

Inspect captures separately for occlusion, clipping, overflow, undersized targets, illegible text and crowding. Contrast checks include expanded subagents and collapsed running/completed/stopped/error headers. Run structural/automated accessibility checks and manual target-size/focus checks. Record WCAG 2.2 gaps, browser coverage, console/network errors, performance evidence, unchanged measurement behavior, and outstanding security/reliability reviews in `qa.md`; a screenshot filename is not a passing inspection.

Network setup: a test daemon can be explicitly local-only. That test does not establish that the default startup bind is local. The separately surfaced startup-default defect remains outstanding and must not be hidden by the fixture.

## Default Quick Chat correction

Retrofit only the default Chat body and Chat-tab navigation. Preserve the approved ChatGPT-like chrome, full existing QuickChat history/composer/artifacts/confirmed Clear, selected independent conversations, knowledge destinations and Research behavior. Do not duplicate bootstrap or create another data layer. Existing QuickChat CSS owns legacy presentation; add only scoped flex sizing/flat empty surface where embedding requires it, using the shell's current tokens. Loading disables sending until existing bootstrap completes; running reports the mode/New chat lock; existing failures/Stop/Clear remain in their current path. Capture `/session` with reserved history and empty state at 320/760/1100 × 600/900, at rest and inner-scroll where available, using isolated seeded data. Live parity must prove `/chat` history appears unchanged on `/session`, reload and Chat-tab return, and explicit New chat remains distinct. This does not satisfy remaining T3 Stop/artifact journey, T4, T5 or AC13.
