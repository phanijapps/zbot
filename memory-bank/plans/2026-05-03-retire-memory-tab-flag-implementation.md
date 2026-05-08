# Retire `memory_tab_command_deck` flag — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the legacy memory UI (`MemoryTabLegacy`, `WebMemoryPanel`, `MemoryTabGate`, `MemoryPage`, `useFeatureFlag`, `MemoryFactCard`) and mount the Command Deck unconditionally at `/memory`.

**Architecture:** UI-only deletion. The backend memory store is already trait-routed (Phase E6c) — no changes there. Edit two consumer files (`App.tsx`, `features/memory/index.ts`) to point at the Command Deck directly, then delete eight orphaned files. The `feature_flags` HashMap on `ExecutionSettings` stays (flag-agnostic, useful for future flags); stale `memory_tab_command_deck` entries in user `settings.json` become harmless dead keys.

**Tech Stack:** TypeScript, React, Vite, Vitest. UI lives in `apps/ui/`.

**Spec:** `memory-bank/future-state/2026-05-03-retire-memory-tab-flag-design.md`

---

## File map

**Edit (2):**
- `apps/ui/src/App.tsx` — replace `MemoryPage` import + route element with `MemoryTab` from the deck.
- `apps/ui/src/features/memory/index.ts` — drop dead exports; re-alias `MemoryTab` to the deck.

**Delete (8):**
- `apps/ui/src/features/memory/MemoryTabLegacy.tsx`
- `apps/ui/src/features/memory/WebMemoryPanel.tsx`
- `apps/ui/src/features/memory/MemoryTabGate.tsx`
- `apps/ui/src/features/memory/MemoryPage.tsx`
- `apps/ui/src/features/memory/useFeatureFlag.ts`
- `apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts`
- `apps/ui/src/features/memory/MemoryFactCard.tsx`
- `apps/ui/src/features/memory/MemoryFactCard.test.tsx`

**Untouched:** Everything in `apps/ui/src/features/memory/command-deck/`, the entire backend, all other UI features.

---

## Task 1 — Capture baseline: build + test green before any change

**Files:** none (verification only).

- [ ] **Step 1.1: Confirm we're on the right branch with a clean tree apart from this plan**

  Run: `git status --short`

  Expected: only the new `memory-bank/plans/2026-05-03-retire-memory-tab-flag-implementation.md` showing as `??`. Branch: `docs/release-on-main-workflow-plan`.

- [ ] **Step 1.2: Establish UI build baseline**

  Run: `cd /home/videogamer/projects/agentzero/apps/ui && npm run build`

  Expected: exits 0, no TypeScript errors.

- [ ] **Step 1.3: Establish UI test baseline**

  Run: `cd /home/videogamer/projects/agentzero/apps/ui && npm test -- --run 2>&1 | tail -10`

  Expected: all suites pass. Note the total test count; we'll compare after deletion.

---

## Task 2 — Re-point the `/memory` route at the Command Deck

This must happen **before** any deletion so the route is never left referencing a removed module.

**Files:**
- Modify: `apps/ui/src/App.tsx:28` (import) and `apps/ui/src/App.tsx:208` (route element)

- [ ] **Step 2.1: Update the import on line 28**

  ```diff
  -import { MemoryPage } from "./features/memory";
  +import { MemoryTab as MemoryPanel } from "./features/memory";
  ```

  Why the alias: keeps a single distinct local name. The new `MemoryTab` from the deck takes an `agentId` prop, unlike the old `MemoryPage` which took none — so the route element on line 208 must change too.

- [ ] **Step 2.2: Update the route element on line 208**

  ```diff
  -                  <Route path="/memory" element={<MemoryPage />} />
  +                  <Route path="/memory" element={<MemoryPanel agentId="root" />} />
  ```

  The `agentId="root"` value matches today's behavior — `MemoryPage.tsx:11` passes the same string.

- [ ] **Step 2.3: Run TypeScript build**

  Run: `cd /home/videogamer/projects/agentzero/apps/ui && npm run build 2>&1 | tail -20`

  Expected: clean exit. The build may flag `MemoryPage` (in index.ts) as still-exporting-but-unused — that's fine; we delete it next task.

- [ ] **Step 2.4: Spot-check via the running dev server**

  Pre-condition: `npm run dev` already running on port 3000 (it is — see prior session context).

  Open `http://localhost:3000/memory` in the browser and confirm the Command Deck mounts (`WardRail` on the left, `ContentDeck` middle, `WriteRail` right). The flag in user settings is currently `false` so this is a real behavior change to verify.

  Use Chrome DevTools MCP `navigate_page` + `take_snapshot` to confirm the deck UI is visible.

---

## Task 3 — Update `features/memory/index.ts` to drop dead exports

**Files:**
- Modify: `apps/ui/src/features/memory/index.ts`

- [ ] **Step 3.1: Replace the entire file**

  New full contents of `apps/ui/src/features/memory/index.ts`:

  ```ts
  export { MemoryTab } from "./command-deck/MemoryTab";
  ```

  That single export covers what `App.tsx` needs after Task 2. Every other previous export targeted a file we are about to delete.

- [ ] **Step 3.2: Build to verify nothing else imported the removed exports**

  Run: `cd /home/videogamer/projects/agentzero/apps/ui && npm run build 2>&1 | tail -20`

  Expected: clean. If TypeScript flags an unresolved import for `MemoryTabGate`, `MemoryTabLegacy`, `WebMemoryPanel`, `MemoryPage`, `MemoryFactCard`, or `useFeatureFlag` somewhere in `apps/ui/src`, fix the offending file. Per the audit, no such consumers exist outside the files being deleted, but verify.

---

## Task 4 — Delete the eight legacy files

Order matters only insofar as we delete in one batch and immediately rebuild. Files are listed alphabetically for review-friendliness.

**Files:**
- Delete: `apps/ui/src/features/memory/MemoryFactCard.test.tsx`
- Delete: `apps/ui/src/features/memory/MemoryFactCard.tsx`
- Delete: `apps/ui/src/features/memory/MemoryPage.tsx`
- Delete: `apps/ui/src/features/memory/MemoryTabGate.tsx`
- Delete: `apps/ui/src/features/memory/MemoryTabLegacy.tsx`
- Delete: `apps/ui/src/features/memory/WebMemoryPanel.tsx`
- Delete: `apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts`
- Delete: `apps/ui/src/features/memory/useFeatureFlag.ts`

- [ ] **Step 4.1: Delete in one git command**

  Run:

  ```bash
  cd /home/videogamer/projects/agentzero && git rm \
    apps/ui/src/features/memory/MemoryFactCard.test.tsx \
    apps/ui/src/features/memory/MemoryFactCard.tsx \
    apps/ui/src/features/memory/MemoryPage.tsx \
    apps/ui/src/features/memory/MemoryTabGate.tsx \
    apps/ui/src/features/memory/MemoryTabLegacy.tsx \
    apps/ui/src/features/memory/WebMemoryPanel.tsx \
    apps/ui/src/features/memory/__tests__/useFeatureFlag.test.ts \
    apps/ui/src/features/memory/useFeatureFlag.ts
  ```

  Expected: `git status` shows 8 files staged for deletion plus the modified `App.tsx` and `index.ts`.

- [ ] **Step 4.2: Check the `__tests__/` directory isn't now empty**

  Run: `ls /home/videogamer/projects/agentzero/apps/ui/src/features/memory/__tests__/`

  Expected: directory is empty after `useFeatureFlag.test.ts` is removed (no other tests live there). If empty, also remove the directory:

  ```bash
  rmdir /home/videogamer/projects/agentzero/apps/ui/src/features/memory/__tests__
  ```

  Otherwise leave it alone.

---

## Task 5 — Verify build + tests after deletion

**Files:** none (verification only).

- [ ] **Step 5.1: TypeScript build**

  Run: `cd /home/videogamer/projects/agentzero/apps/ui && npm run build 2>&1 | tail -20`

  Expected: exits 0. No "cannot find module" errors. Same warnings (if any) as the baseline from Step 1.2.

- [ ] **Step 5.2: Test suite**

  Run: `cd /home/videogamer/projects/agentzero/apps/ui && npm test -- --run 2>&1 | tail -10`

  Expected: all suites pass. Total test count drops by exactly the count from `useFeatureFlag.test.ts` (typically 1–3 tests) plus `MemoryFactCard.test.tsx` (5 tests in the file). Confirm no test references the deleted files.

  If a test fails because it imported one of the deleted symbols: the audit missed it. Fix the test file or delete it as appropriate, then rerun.

- [ ] **Step 5.3: Final grep audit**

  Run:

  ```bash
  cd /home/videogamer/projects/agentzero && grep -rn "memory_tab_command_deck\|useFeatureFlag\|MemoryTabGate\|MemoryTabLegacy\|WebMemoryPanel\|MemoryPage\|MemoryFactCard" apps/ui/src 2>/dev/null
  ```

  Expected: zero matches. If there are matches, every line must be inspected — likely a comment or a stale import we missed.

---

## Task 6 — Manual smoke test with Chrome DevTools

The unit test suite covers the deck components but does not actually mount `/memory`. Confirm in a real browser before committing.

**Files:** none.

- [ ] **Step 6.1: Reload `/memory` in the running dev server**

  The Vite dev server (port 3000) auto-reloads on file changes. Use Chrome DevTools MCP:

  1. `navigate_page` to `http://localhost:3000/memory`
  2. `take_snapshot` — verify the snapshot includes deck-specific UI (`WardRail`, ward chips, search bar, content deck list).
  3. `list_console_messages` — confirm no React errors or unresolved imports.

- [ ] **Step 6.2: Verify the route does not depend on the feature flag**

  Even with no `featureFlags` set in `settings.json` (or with `memory_tab_command_deck: false` set explicitly), the deck must render. Quick check via DevTools console:

  ```js
  fetch('/api/settings/execution').then(r => r.json()).then(s => console.log(s.data.featureFlags))
  ```

  The output is irrelevant — what matters is that `/memory` rendered the deck regardless. Note in the PR description that the flag is now ignored.

---

## Task 7 — Commit

**Files:** none (commit operation).

- [ ] **Step 7.1: Stage and commit**

  Run from `/home/videogamer/projects/agentzero`:

  ```bash
  git add apps/ui/src/App.tsx apps/ui/src/features/memory/index.ts
  git commit -m "$(cat <<'EOF'
  refactor(ui): retire memory_tab_command_deck flag and legacy memory UI

  The Command Deck has been the intended successor for several releases
  but stayed gated behind a feature flag with no UI control, defaulting
  to false — so most users never saw it. Storage is already unified
  behind the MemoryFactStore trait (Phase E6c), so this cutover is
  UI-only. Mount the Command Deck unconditionally at /memory and delete
  the legacy stack.

  Deleted:
  - MemoryTabLegacy.tsx, WebMemoryPanel.tsx, MemoryTabGate.tsx
  - MemoryPage.tsx, useFeatureFlag.ts, MemoryFactCard.tsx
  - useFeatureFlag.test.ts, MemoryFactCard.test.tsx

  Edited:
  - App.tsx — /memory route now mounts MemoryTab (Command Deck) directly
    with agentId="root", matching today's behavior
  - features/memory/index.ts — single export, points at the deck

  Backend untouched. The feature_flags HashMap on ExecutionSettings stays
  (flag-agnostic, useful for future flags). Stale memory_tab_command_deck
  keys in user settings.json are silently ignored.

  Acknowledged parity gaps with WebMemoryPanel (12-category taxonomy,
  agent dropdown filter, explicit pinning UI, "About Me" type, per-category
  stats) are documented in the design doc and tracked as follow-ups if
  needed. No data is lost — facts persist in SQLite regardless.

  Spec: memory-bank/future-state/2026-05-03-retire-memory-tab-flag-design.md

  Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

  Expected: single commit, ~10 files changed (2 modified, 8 deleted).

- [ ] **Step 7.2: Verify the commit**

  Run: `git show --stat HEAD`

  Expected: 8 files with `delete mode 100644`, 2 with insertion/deletion counts. No surprises.

---

## Task 8 — Push and open PR to `develop`

**Files:** none.

- [ ] **Step 8.1: Push the branch**

  Run: `git push -u origin docs/release-on-main-workflow-plan`

  Expected: push succeeds; if the branch already tracks `origin`, drop the `-u`.

- [ ] **Step 8.2: Open the PR**

  Run:

  ```bash
  gh pr create --base develop --title "refactor(ui): retire memory_tab_command_deck flag and legacy memory UI" --body "$(cat <<'EOF'
  ## Summary

  - Mount the Memory Command Deck unconditionally at `/memory`.
  - Delete the legacy memory UI stack (`WebMemoryPanel`, `MemoryTabLegacy`, `MemoryTabGate`, `MemoryPage`, `useFeatureFlag`, `MemoryFactCard`) and their tests.
  - Backend is untouched — storage is already unified via the `MemoryFactStore` trait (Phase E6c).
  - The `feature_flags` HashMap stays for future flags. Stale `memory_tab_command_deck` keys in user `settings.json` are silently ignored — no migration needed.

  See design doc: `memory-bank/future-state/2026-05-03-retire-memory-tab-flag-design.md`.

  Also bundles a previously-uncommitted fix on this branch: `fix(research-v2): render plain-text assistant responses on session reload` — this lets reopened sessions display final answers that the model emitted as plain text instead of via a `respond()` tool call.

  ## Test plan

  - [x] `npm run build` (UI) clean
  - [x] `npm test` (UI) green; deleted-test count matches expectation
  - [x] `grep -r "memory_tab_command_deck|useFeatureFlag|MemoryTabGate|MemoryTabLegacy|WebMemoryPanel|MemoryPage|MemoryFactCard" apps/ui/src` returns 0 matches
  - [x] Live: `/memory` in a fresh browser session renders the Command Deck regardless of `featureFlags` value
  - [x] research-v2 fix verified end-to-end via Chrome DevTools (session reload now shows the response)

  🤖 Generated with [Claude Code](https://claude.com/claude-code)
  EOF
  )"
  ```

  Expected: PR URL printed. The PR targets `develop`, not `main`, per the user's instruction and the project's PR workflow rule.

- [ ] **Step 8.3: Report the PR URL back to the user**

  Print the URL returned by `gh pr create`.

---

## Self-review checklist

- [ ] Every spec section maps to a task: deletion list (Task 4), edits (Tasks 2 + 3), backend untouched (no task — explicitly noted), verification (Tasks 1, 5, 6), rollout (Tasks 7, 8). ✅
- [ ] No placeholders (no TBD/TODO, every code block is complete). ✅
- [ ] Type/symbol consistency: every reference to `MemoryTab`, `MemoryPanel`, `MemoryPage` traces to a defined import or alias. ✅
- [ ] Commands are runnable verbatim (paths absolute, no shell variables left undefined). ✅
- [ ] Order is safe: edit consumers before deleting providers. ✅
