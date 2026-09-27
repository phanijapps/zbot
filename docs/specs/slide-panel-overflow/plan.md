# Plan: Slide Panel Overflow

- **Spec:** [`spec.md`](spec.md)
- **Status:** Done

## Approach

Pin the overflow behavior in a browser regression test, then adjust only the shared slide-over CSS so panels cannot exceed the viewport, header actions cannot be displaced by intrinsic text width, and wide preview content stays inside its scroll area.

## Tasks

### T1: Slide panels contain long headers and preview content

**Depends on:** none

**Tests:**

- **TDD / browser layout:** `apps/ui/tests/e2e/slide-panel-layout.spec.ts` fails on the current CSS for 480px/800px panel bounds, visible Close actions, and content containment.
- **Goal-based:** From `apps/ui`, run `npm run lint`, `npm run build` (includes TypeScript), and `npm test` for the existing component suite.
- **Visual/manual QA:** Exercise the rendered fixture at both widths and record measured panel/control bounds in `docs/specs/slide-panel-overflow/qa.md`; the session ends after verifying bounds and visible controls, not backend artifact retrieval.

**Approach:**

- Update `apps/ui/src/styles/components.css` only for `.artifact-slideout` and `.slideover` shared layout rules; register the browser test in `apps/ui/playwright.config.ts`'s required lane and update the lane's ownership assertion in `apps/ui/tests/integration/playwright-config.test.ts`.
- Preserve local horizontal scrolling for intentionally wide preview formats such as tables or code.

**Done when:** the red browser test passes, the UI gates pass, and the rendered viewport has no off-screen panel/control.
