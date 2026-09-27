# Spec: Slide Panel Overflow

- **Status:** Shipped
- **Owner:** @videogamer
- **Plan:** [`plan.md`](plan.md)
- **Constrained by:** none
- **Contract:** none — CSS layout only
- **Shape:** ui
- **Mode:** light (no risk trigger fired)

## Objective

Artifact, vault-file, and generic slide-over panels keep their header controls and preview content usable within the viewport at desktop and narrow widths, including long filenames and unbroken text.

## Boundaries

### Always do

- Keep existing panel content, preview behavior, and close/download actions.

### Ask first

- Change any provider-specific drawer or the content rendering/format of a preview.

### Never do

- Add a new dependency, panel abstraction, or application route for this layout fix.

## Testing Strategy

- Browser layout regression at 480px and 800px: visual/manual QA with measured panel, action, and content bounds, because CSS intrinsic sizing is a rendered-browser behavior.
- Existing UI typecheck and tests: goal-based checks for unchanged component behavior.

## Acceptance Criteria

- [x] At 480px and 800px viewport widths, the artifact/vault slide-out and generic slide-over stay within the viewport; Close and other header actions remain visible with a 220-character filename or subtitle.
- [x] A 400-character unbroken preview line stays within the panel content region by wrapping or scrolling inside it, without extending the panel or viewport horizontally.
- [x] Normal-length content remains readable and the existing close, download, and preview interactions remain available.

## Assumptions

- Technical: artifact and vault file previews share `.artifact-slideout`, while generic drawers use `.slideover` (source: `apps/ui/src/styles/components.css`, `ArtifactSlideOut.tsx`, `VaultFileSlideOut.tsx`).
- Technical: browser reproduction shows a 541px generic drawer on a 480px viewport and long-content overflow in the artifact panel (source: read-only Playwright layout probe, 2026-09-27).
- Product: this fix covers artifact, vault, and generic slide-over panels, not provider-specific drawers (source: user confirmation 2026-09-27).
- Process: experience-design pack not installed; design intent for this surface is ungrounded (source: available skill roster).
