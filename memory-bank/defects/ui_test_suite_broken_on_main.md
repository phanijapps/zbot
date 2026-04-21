# Defect — 24 UI tests fail on main; Sonar coverage step needs continue-on-error

## Symptom

`npm run test:coverage` exits 1 on every run of the SonarQube workflow
on `main`. The failure was hidden until the `feature/sonar` workflow
wired UI coverage generation into CI — previously the step didn't exist,
so the breakage never surfaced as a pipeline failure. 24 tests fail
across 3 files; 418 tests pass.

## Reproduction

```
cd apps/ui
npm ci
npm run test:coverage
# → "Test Files 3 failed | 38 passed (41)"
# → "Tests 24 failed | 418 passed (442)"
# → exit code 1
```

## Failure breakdown

### tests/integration/dashboard.test.tsx — 18 failures

Root-level symptom: `TypeError: transport.listLogSessions is not a
function`, followed by cascading `TypeError: Cannot read properties of
undefined (reading 'color')` and ~18 `TestingLibraryElementError: Unable
to find an element with the text: Dashboard / root / Active Sessions
/ ...`.

The tests expect a `transport.listLogSessions()` method that no longer
exists on the transport API (renamed or removed during a past refactor).
Every test in the file breaks because `useRecentSessions` throws on
mount, which unmounts the rest of the dashboard before any assertion
can find its target element.

### src/services/transport/http.test.ts — 4 failures

`TypeError: fetch failed` with `Caused by: Error: connect ECONNREFUSED
127.0.0.1:18791` at lines 400, 409, 422, 431. These tests fall through
the MSW handler chain and hit the real gateway port, which is not
running in CI. MSW passthrough config needs handlers for the four
endpoints being exercised (or the tests should mock them explicitly).

### src/features/research-v2/SessionsList.test.tsx — 2 failures

`groupSessions — groups into Running / Today / Yesterday / Last week /
Older` and `sorts each bucket newest-first`. Both are clock-dependent:
the test seeds sessions with dates relative to "today" and expects them
in specific buckets. When the system clock crosses midnight (or the
sessions were seeded on a different day than the assertion runs), the
bucketing drifts. Fix: mock `Date.now()` / use `vi.setSystemTime(...)`.

## Root cause

This is three separate regressions that accumulated:

1. A transport-layer refactor dropped `listLogSessions` without
   updating the dashboard tests.
2. An MSW upgrade or handler config change silently shifted four
   `http.test.ts` cases from mocked to passthrough.
3. `groupSessions` was never hardened against clock drift.

## Interim mitigation

`feature/sonar` adds:

- `reportOnFailure: true` in `apps/ui/vitest.config.ts` so vitest still
  emits `coverage/lcov.info` when tests fail (default behaviour is to
  skip the report entirely).
- `continue-on-error: true` on the `Generate UI coverage (LCOV)` step
  in `.github/workflows/sonarqube.yml` so the exit-1 from vitest does
  not abort the subsequent LCOV path rewrite + Sonar scan.

Net effect: CI stays green, SonarCloud receives coverage data for the
~95% of tests that pass, but the 24 broken tests go unnoticed at the
pipeline level. **Remove the `continue-on-error` the moment this defect
is resolved**, otherwise a future regression that breaks the entire
suite will not be caught.

## Proper fixes to consider

Each sub-failure wants its own PR:

- **dashboard.test.tsx** — find the renamed/removed transport method,
  update the mock to match the current API, re-exercise every assertion.
  ~2-4 hours.
- **http.test.ts** — audit MSW handlers for the four failing endpoints,
  either add explicit mocks or convert the tests to use a spy instead
  of passthrough. ~1-2 hours.
- **SessionsList.test.tsx** — wrap the test in `vi.useFakeTimers()` +
  `vi.setSystemTime('2026-04-20T12:00:00Z')` so bucketing is
  deterministic. ~30 min.

Minimum bar before removing `continue-on-error`: `npm run test:coverage`
must exit 0 on `main` for at least one full release cycle.

## Files involved

- `apps/ui/tests/integration/dashboard.test.tsx`
- `apps/ui/src/services/transport/http.test.ts`
- `apps/ui/src/features/research-v2/SessionsList.test.tsx`
- `apps/ui/vitest.config.ts` — added `reportOnFailure: true`
- `.github/workflows/sonarqube.yml` — added `continue-on-error: true`

## Discovered

2026-04-21, while chasing a "SonarQube isn't syncing coverage" report.
The CI logs (`sonar.logs`, 5260 lines) showed the npm step exiting 1
before the Sonar scan action ever ran. Surfaced by the workflow change
in commit `4fc27d8` (Phase 1 of the Sonar cleanup plan) which wired UI
coverage into the pipeline for the first time.
