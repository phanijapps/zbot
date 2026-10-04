# Slide panel layout QA — 2026-09-27

The browser session exercised the current UI stylesheet with representative artifact, vault, and generic slide-over markup. It ended after checking panel and Close-button bounds and unbroken plain-text containment at 480px and 800px. Backend artifact retrieval, provider-specific drawers, and other routes were not exercised.

| Width | Panel | Left | Right | Close right | Body width / scroll width |
| --- | --- | ---: | ---: | ---: | ---: |
| 480px | Artifact / vault shared style | 0 | 480 | 464 | 479 / 479 |
| 480px | Generic | 0 | 480 | 448 | 479 / 479 |
| 800px | Artifact / vault shared style | 304 | 800 | 784 | 495 / 495 |
| 800px | Generic | 260 | 800 | 768 | 539 / 539 |

The 480px screenshot was visually inspected: the long header is truncated inside the panel, Close is visible, and long unbroken body text wraps. The Playwright regression drove the actual Vite-loaded stylesheet for artifact, vault, and generic variants at both widths; it checked both Download and Close bounds for the artifact variant, and all 2 tests passed. Normal preview interactions remain covered by the existing component tests, not repeated in this layout fixture.

Verification: `npm run lint` exited 0 with 20 pre-existing warnings; `npm run build` exited 0; the new Playwright file passed 2/2; the Playwright ownership integration test passed 2/2. The full Vitest suite passed 1359/1360, with the unrelated Research Stop-button assertion failing in isolation as well (tracked as `pre-existing-research-stop-assertion`). The full required Playwright lane passed 15/24; nine existing navigation/research cases could not reach their expected route with the local daemon unavailable at port 18791. The layout fixture does not depend on that daemon.

The repo-wide spec-status lint reports one unrelated hard violation: `docs/specs/exec-consolidation-waves/spec.md` uses `Drafting`, outside the allowed status vocabulary. The slide-panel spec itself has `Shipped` status with all acceptance criteria checked. Base freshness against `origin/develop` passed.
