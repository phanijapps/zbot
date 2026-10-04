# Hook Activity preflight and evidence

Surface slug: external-hooks. Mode: retrofit. Scope: read-only hook Activity in
the existing session shell; no Settings controls. The approved desktop preview
and existing warm white/rust palette are the reference. Existing tokens:
--background, --background-surface, --muted-foreground, --primary, --spacing-*,
--text-sm, --radius-sm. New styles reuse these tokens.

Design handoff: no [design] section configured. Repository layout and user
profile layout files were absent. XD genre routing: skipped
(experience-design pack absent). Design-review and experience-reviewer are
unavailable; named skips apply. Frontend review will run after implementation.

Brownfield: SessionShell has a placeholder detail panel. getSessionDetails is
already available; Activity must use a confirmed session ID. QuickChat already
owns bootstrap, so its confirmed ID is surfaced without a second bootstrap.
Only hook metadata is disclosed; ordinary Activity remains available where
the existing projection supplies it. The next session UI slice owns Sources
and Files completion.

| State | Treatment |
| --- | --- |
| loading | Bounded row skeleton/status with aria-busy |
| empty | No recorded hook activity for the selected session |
| error/offline | Keep same-session rows; show unavailable notice and Retry |
| partial/large-data-set | Server truncation notice, no silent client slicing |
| content/success | Fixed server labels plus sanitized hook status/metadata |
| first-run | Select/start a conversation; no invented session ID |
| disabled | No editing controls; disabled hooks spawn no processes or Activity rows |
| permission/denied | Details unavailable, preserve conversation and Retry |
| blocked | Distinct blocked hook outcome, without private reason |
| long-content | Wrap IDs and disclose metadata with native details/summary |
| high-zoom | Reflow within panel at narrow width/zoom |
| reduced-motion | No new animation |
| keyboard-only | Native disclosure, reachable Retry, visible focus |
| no-results | Inapplicable: no search/filter introduced |
| destructive-confirmation | Inapplicable: read-only surface |

## Verification manifest

- routes: actual /session default Chat and /session/:id selected Chat.
- viewports: Chromium, widths 759/760/1100, heights 600/900.
- screenshots: /tmp/zbot-hooks-ui-evidence; final twelve captures and both
  keyboard-expanded captures independently inspected after the final passing run.
- states: completed, expanded metadata, Running→Cancelled/reload exercised
  through the real daemon; loading/empty/error/cached Retry/truncation/stale
  responses covered by component tests. Separate visual error/truncation, zoom
  and reduced-motion preference captures remain unverified. No new animation.
- a11y: axe-core 4.13.0 Activity subtree, wcag2a/wcag2aa/wcag21aa:
  zero violations, fifteen passing rules on each route. Actual keyboard
  Tab/Enter opens native metadata. Summary target 254×30px passes the
  24px minimum; visible rust outline 2px with 2px offset is observed.
  Full WCAG 2.2 AA coverage remains unverified for 2.4.11, 2.5.7, 3.2.6,
  3.3.7 and 3.3.8; no complete focus-appearance AAA claim.
- HTML: changed Activity subtree passes standard,a11y validation on both routes.
- perf: current UI build passes; no CWV/Lighthouse measurement or pass claimed.
- console/network: no comprehensive clean-console/network claim.
- analytics: inapplicable, read-only Activity adds no primary mutation.
- known exceptions: existing full-shell HTML violations registered in workspace
  backlog pre-existing-session-shell-html-validation; Sources/Files next slice.
- security/privacy and reliability: bounded projection and construction proofs;
  independent security/quality/frontend implementation reviews Clean (review.md).

## Rendered capture and separate inspection — 2026-10-04

Actual daemon/current UI: /session and /session/:id, Chromium, widths 759,
760 and 1100 (the declared <760, 760–1099 and >=1100 channels), heights
600 and 900. All twelve completed-state screenshots were captured by the
full-mode Chat construction test and separately inspected with view_image.
/tmp/zbot-hooks-ui-evidence/{default-chat,selected-chat}-captures.json records
the canonical route, dimensions, scroll and screenshot for each. Page scroll
max=0 in every capture: page-scrollable:no; the Activity panel scrolls internally.
No document horizontal overflow was observed.

Inspection: hook names/statuses remain legible without clipping; native
disclosures have clear row spacing and fixed status text. At 759/760 the
existing detail drawer overlays the conversation below the header; at 1100
it occupies the right column. Short windows scroll Activity internally, with
no lost records. Warm white/rust tokens match the established shell. Broader
conversation layout and Sources/Files remain the next approved slice. Metadata
open/focus and Stop settlement pass the final checks above; separate rendered
error/partial states remain unverified.

Full-shell standard,a11y HTML validation reports fourteen pre-existing
implicit-button/native-element/text-content violations per route, mainly the
existing ChatInput/navigation. No whole-shell HTML pass is claimed. Baseline markup was confirmed against
HEAD; the changed Activity subtree passes independently.

Specialist finding applied: persistent status region is mounted empty before
loading/error mutations; only the list is aria-busy. Component controls verify
region identity through content/error/Retry and absence of a busy ancestor.
Final recapture after all fixes: twelve viewport images and two keyboard-expanded
images inspected; axe reports zero violations and fifteen passing rules per route;
changed Activity HTML passes standard,a11y. Specialist re-review is Clean (review.md).
