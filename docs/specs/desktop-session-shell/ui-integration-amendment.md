# Desktop UI integration amendment

**Status:** Draft

## User-authorized outcome

The user approved full visual integration of Memory and Observatory into the desktop shell and requested contrasting colors so subagent content does not disappear against the background. This records the requested amendment; it does not replace the sealed spec or plan or authorize engine progress changes.

## Scope

- Render Memory and Observatory within the same restrained desktop navigation, typography, spacing, and color system as conversation pages. Retain Memory search, inspection and correction, and the full graph workspace rather than replacing either with a decorative overview.
- Preserve the selected conversation ID and stored mode when navigating to knowledge pages and returning. Do not reset or delete a conversation on navigation.
- Give Research subagent cards visibly distinct surfaces, boundaries, readable text, and status labels. Running, completed and failed states retain textual/icon indicators; color is supplementary. Check text contrast at least 4.5:1 and necessary control/focus boundaries at least 3:1 using computed colors, including expanded content and disabled controls.
- Preserve legacy Chat/Research rollback routes, hide the ward explorer only in the new shell, and do not add agent builders, hook controls, new persistence, or runtime semantics.

## Proposed implementation strategy

Reuse existing Memory and Observatory components and services inside shared desktop navigation. Keep conversation execution hooks separate from knowledge routes. Apply one shared scoped token palette, with explicit subagent surface, text, boundary and status roles. Test navigation and restoration before production routing changes; compare rendered desktop and narrow layouts with realistic seeded content, expanded subagents and long text.

## Verification

Route tests must demonstrate that knowledge destinations use desktop chrome without duplicate legacy chrome and return to the same conversation. Existing Memory edit and graph interaction tests remain green. Browser checks must exercise both destinations, expanded subagents, keyboard navigation, narrow layouts and computed contrast. No completion claim until the existing parity gates and final reviewers also pass.

## Current baseline and required recovery

Run `859524fa-ed8a-4e63-b7a5-eb342d9ed19f` is CODE-IMPLEMENTATION at T3; T1/T2 verification is recorded in `disposition.md`. Its spec and plan hashes are sealed. Reopening them requires a reviewed replacement baseline, not editing the approved hashes or progress files by hand.

Recovery rung: steer. Request explicit authorization to reset only this spec's `state.json` and `engine-state.json` through the supplied reset commands, preserving all implementation files, spec/plan artifacts and completed-test evidence. After confirmation, amend AC8 and the task plan, review the new baseline, and revalidate completed work before continuing.

The effective startup network-default correction remains a separate unresolved approval; this UI approval does not silently change network exposure or the user's saved settings.
