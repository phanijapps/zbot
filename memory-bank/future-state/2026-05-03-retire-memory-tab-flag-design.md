# Retire `memory_tab_command_deck` flag and legacy memory UI

**Status:** Design (ready to implement)
**Date:** 2026-05-03
**Owner:** phanijapps
**Target branch:** `develop` (continuing on `docs/release-on-main-workflow-plan`)

## Why

The Memory tab has shipped two parallel UI implementations for several releases:

- **Legacy** — `WebMemoryPanel.tsx` (645 LOC, agent + 12-category model) and `MemoryTabLegacy.tsx` (per-agent variant, currently uncalled).
- **Command Deck** — `command-deck/` (ward-rail / content-deck / write-rail layout, hybrid search, timewarp).

Selection is gated by a feature flag `memory_tab_command_deck` that has no UI control and defaults to `false` — so the deck is invisible to most users today even though it is the intended successor.

Storage is **already** unified behind the `MemoryFactStore` trait (Phase E6c). There is no "old" storage system to retire — the work is UI-only.

The flag is now a maintenance tax: two render paths to test, dead components in the build, and divergence risk on every memory feature. CLAUDE.md explicitly forbids "feature flags … when you can just change the code". This PR cashes that in: hard-delete the legacy components and the flag plumbing, mount the Command Deck unconditionally at `/memory`.

## Scope

UI only. No backend / migration / data changes.

### Delete (8 files)

| File | Why |
|---|---|
| `apps/ui/src/features/memory/MemoryTabLegacy.tsx` | Per-agent variant, no live callers (only `MemoryTabGate` references it). |
| `apps/ui/src/features/memory/WebMemoryPanel.tsx` | Top-level legacy panel. Sole consumer is `MemoryPage`. |
| `apps/ui/src/features/memory/MemoryTabGate.tsx` | Flag dispatcher, no longer needed. |
| `apps/ui/src/features/memory/MemoryPage.tsx` | Thin flag-aware wrapper; replaced by direct deck mount in `App.tsx`. |
| `apps/ui/src/features/memory/useFeatureFlag.ts` | Only used by the two files above. |
| `apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts` | Tests the deleted hook. |
| `apps/ui/src/features/memory/MemoryFactCard.tsx` | Sole consumer is `MemoryTabLegacy`; command deck uses its own card components (`MemoryItemCard`). |
| `apps/ui/src/features/memory/MemoryFactCard.test.tsx` | Tests the deleted card. |

### Edit (2 files)

| File | Change |
|---|---|
| `apps/ui/src/App.tsx` | Replace `MemoryPage` import with `MemoryTabCommandDeck`. Mount at `/memory` with `agentId="root"` (preserves today's behavior — that is exactly what `MemoryPage.tsx:11` passes). |
| `apps/ui/src/features/memory/index.ts` | Drop exports for `MemoryTabGate`, `MemoryTabLegacy`, `WebMemoryPanel`, `MemoryPage`. Keep the command-deck export. |

### Backend — untouched

The `feature_flags: HashMap<String, bool>` field on `ExecutionSettings` is flag-agnostic and stays in place for future flags. `gateway/src/http/settings.rs` does not reference the string `"memory_tab_command_deck"` anywhere — the flag was always interpreted on the UI side.

If a user has `{ "execution": { "featureFlags": { "memory_tab_command_deck": true|false } } }` persisted in `settings.json`, the key is silently ignored after this PR. No reader, no crash. We do not strip the key on load — settings persistence preserves unknown keys, so it sits as harmless dead data until the user clears it themselves.

## Acknowledged parity gaps

The Command Deck is not a 1:1 replacement for `WebMemoryPanel`. Each gap below has been considered and accepted as out of scope for this PR. Track follow-ups as separate issues if any user notices.

| Lost feature | Where it lived | Status |
|---|---|---|
| Agent-level filtering | `WebMemoryPanel` agent dropdown | Deck is ward-scoped via `WardRail`; covers most current use cases. |
| 12-category taxonomy (correction, instruction, user, …) | `WebMemoryPanel` chips | Deck uses 4 content kinds (facts/wiki/procedures/episodes). |
| Explicit "pin" UI | `WebMemoryPanel` add-form | Deck has no pin affordance today. Pinning the trait is still on the backend; UI can reintroduce later if needed. |
| "About Me" type distinction | `WebMemoryPanel` type selector | Deck collapses to facts. |
| Per-category stats panel | `WebMemoryPanel` header | Deck shows aggregate counts in `WriteRail`. |

No data is lost — facts persist in SQLite regardless of which UI is mounted.

## Verification

1. `cd apps/ui && npm run build` — clean.
2. `cd apps/ui && npm test` — all suites green; `command-deck/__tests__/e2e-smoke.test.tsx` already exercises the deck without flag mocking and stays green.
3. Live: navigate to `http://localhost:3000/memory` after `npm run dev`. Command Deck mounts unconditionally — no flag toggle.
4. After the change, `grep -r "memory_tab_command_deck\|useFeatureFlag\|MemoryTabGate\|MemoryTabLegacy\|WebMemoryPanel\|MemoryPage" apps/ui/src` returns **zero** matches.

## Rollout

- Single PR on the existing `docs/release-on-main-workflow-plan` branch (per Phani's instruction: continue on the same branch).
- Target branch: `develop`. Per repo workflow memory: PRs only, no direct merges to `main`.
- Kill switch: `git revert` of the squash-merge commit. The deletions are in one PR so a single revert restores the legacy path verbatim.

## Out of scope (deliberately)

- **Backend cleanup of stale `feature_flags` entries** — no value, leaves dead JSON keys harmlessly. Skip.
- **Reintroducing pin / agent-filter / 12-category UI in the deck** — separate product decisions, not blockers for this retirement.
- **Removing the `feature_flags` HashMap from `ExecutionSettings`** — kept for future UI flags; removing it is a backend churn we don't need now.

## Test plan checklist

- [ ] All six listed files deleted.
- [ ] `App.tsx` and `index.ts` updated; no broken imports across the project.
- [ ] `npm run build` (UI) clean.
- [ ] `npm test` (UI) green; `useFeatureFlag.test.ts` removed, no new test failures introduced.
- [ ] Manual smoke: `/memory` route renders the deck after a fresh `npm run dev`.
- [ ] `grep` audit per Verification step 4 returns zero matches.
- [ ] PR opens against `develop`, not `main`.
