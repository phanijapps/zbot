# E2E Mock Harness — Full-Stack Design

**Date:** 2026-04-20
**Author:** Phani + Claude
**Status:** Approved
**Supersedes:** `2026-04-20-e2e-mock-llm-harness-requirements.md` (UI-only requirements doc — kept as historical reference).

---

## Summary

Deterministic e2e harness that runs the real `zerod` daemon and the real UI
against recorded session fixtures. Two test modes share the same fixture
bundles:

- **Mode UI** — UI against a mock gateway server that replays a recorded
  WS event stream. Catches UI-layer regressions in isolation.
- **Mode Full** — UI against a real `zerod` binary, with only the LLM and
  tool-call boundaries mocked. Catches backend emission bugs + full
  integration regressions.

Same fixture bundle drives both modes. Divergent failures between modes
are the diagnostic signal: green UI mode + red Full mode points at the
backend; red in both points at the UI.

Ships with 3 seed scenarios (AAPL peer valuation, Praying Mantis brief,
simple Q&A) and 10+ negative/regression specs. Entirely local — no real
LLM API keys, no network, hermetic temp vault per test.

## Problem

UI + backend regressions are currently caught only by manual smoke
against a live daemon with real provider keys. Each smoke cycle costs
30–300 seconds per prompt and burns quota. Event-timing bugs
(subscription-ack races, ping-timeout reconnects, scope-filter drops,
`delegation_started` delivery) are non-deterministic in manual testing
and cannot be asserted in CI.

Separately, the user has observed specific recurring failures:
- "Open existing session → click New Research → send a new prompt → URL
  doesn't change, WS goes silent; only a page refresh brings the new
  session into view." Clear backend/frontend interaction bug with no
  reliable repro path.
- The backend may be emitting different or reordered WS messages than
  the UI expects, but we have no way to prove which side is wrong.

## Goals

1. **Deterministic e2e tests** for `/research-v2` (and later `/chat-v2`)
   runnable locally and in CI in under 90 seconds per spec.
2. **Two-mode diagnostic signal** — Mode UI isolates UI regressions;
   Mode Full catches backend emission bugs. Divergent red/green between
   modes points directly at the failing layer.
3. **Zero real LLM spend** — every scenario runs without real provider
   keys, MCPs, or network.
4. **Real zerod binary** in Mode Full — catches gateway routing, WS
   protocol, runtime subagent spawning, artifact registration, snapshot
   correctness.
5. **Hermetic per-test vault** — temp directory created in setup, torn
   down in teardown; zero state leakage between scenarios.
6. **Extensible** — adding a new fixture or new mode requires only
   additive changes. Spec template + `record-fixture.py` keep the cost
   of new scenarios flat.

## Non-goals

- Simulating LLM reasoning quality. Mocks replay recorded decisions.
- Testing real MCP integrations. MCPs stay empty in test config.
- Database memory recall and knowledge graph correctness — out of scope
  (user explicit decision). Internal bge-small embeddings untouched.
- Cross-browser matrix. Chromium only.
- Visual regression / pixel diffs.

## Architecture

```
                    ┌─── Fixtures (source of truth) ──────────────────────┐
                    │ e2e/fixtures/<scenario>/                            │
                    │   session.json          metadata + executions       │
                    │   llm-responses.jsonl   LLM req/resp by iteration  │
                    │   tool-results.jsonl    tool args → canned result   │
                    │   ws-events.jsonl       ordered ServerMessage       │
                    └───────────┬───────────────────────────────┬─────────┘
                                │                                │
         ┌──────────────────────┴─────────┐     ┌───────────────┴──────────────────────┐
         │ Mode UI                         │     │ Mode Full                             │
         │                                 │     │                                        │
         │ mock-gateway (FastAPI ~300 LOC) │     │ mock-llm (FastAPI ~150 LOC)           │
         │   WS: replay ws-events.jsonl    │     │   /v1/chat/completions                │
         │   REST: serve snapshot state    │     │   replays llm-responses.jsonl         │
         │                                 │     │                                        │
         │   UI connects here directly     │     │ real zerod                            │
         │                                 │     │   ZBOT_VAULT=/tmp/e2e-<uuid>          │
         │ Catches UI regressions:         │     │   ZBOT_REPLAY_DIR=<fixture>/tools     │
         │   reducer, event-map,           │     │   settings.json → mock-llm URL        │
         │   React state, renders          │     │                                        │
         │                                 │     │   runtime/agent-tools wraps every     │
         │ ~5s per spec                    │     │   dispatch with ReplayLookup when     │
         │                                 │     │   ZBOT_REPLAY_DIR is set              │
         │                                 │     │                                        │
         │                                 │     │   UI connects to zerod                │
         │                                 │     │                                        │
         │                                 │     │ Catches UI + zerod emission bugs      │
         │                                 │     │ ~15–60s per spec                      │
         └─────────────────────────────────┘     └───────────────────────────────────────┘
                                │                                │
                                └────── both modes ──────────────┘
                                 Playwright (chromium) drives UI
                                 Assertions run against DOM + URL +
                                 injected WS-event hook
                                 Drift status from /__replay/status
                                 is captured per spec
```

### Why this shape

**Mock at the boundaries, not at the seams.** The LLM and tool-dispatch
surfaces are the only "lies" — every gateway routing decision, WS
framing, runtime subagent spawn, artifact registration, reducer path,
and React render runs real code. Bugs above the mock boundary are
catchable; bugs in the mock layer itself are easy to debug because the
mocks are small (~500 LOC total).

**Shared fixtures.** A single recording of a real session produces all
three jsonl files. Mode UI consumes one, Mode Full consumes the other
two, both modes share `session.json`. One recording, two test angles.

**Drift detection as first-class diagnostic.** Both mock servers expose
`/__replay/status` that returns `{expected, received, drift_count,
first_drift}`. A drifted test reports exactly where the sequence
diverged — making "test fails" into "backend emitted X at iteration 3
instead of the recorded Y".

## Components

### 1. `e2e/fixtures/<scenario>/` — fixture bundles

Four files per fixture. Same format across all scenarios.

**`session.json`** — metadata describing the session shape:

```json
{
  "session_id": "sess-f0e9b78c-...",
  "title": "AAPL Peer Valuation Analysis",
  "executions": [
    {
      "execution_id": "exec-aa77cc67-...",
      "agent_id": "root",
      "parent_execution_id": null,
      "started_at_offset_ms": 0,
      "ended_at_offset_ms": 1765488
    },
    { "...": "..." }
  ],
  "artifacts": [
    { "id": "art-...", "file_name": "peer_valuation.html", "file_type": "html", "file_size": 14092 }
  ]
}
```

**`llm-responses.jsonl`** — one line per LLM request. Keyed by
`(execution_id, iteration, messages_hash)`:

```json
{"execution_id":"exec-aa77cc67-...","iteration":0,"messages_hash":"sha256:...","response":{"choices":[{"message":{"role":"assistant","content":"","tool_calls":[{"id":"call_1","function":{"name":"set_session_title","arguments":"{\"title\":\"AAPL Peer Valuation Analysis\"}"}}]}}]}}
```

**`tool-results.jsonl`** — one line per tool call. Keyed by
`(execution_id, tool_index, tool_name, args_hash)`:

```json
{"execution_id":"exec-aa77cc67-...","tool_index":0,"tool_name":"set_session_title","args_hash":"sha256:...","result":"{\"__session_title_changed__\":true,\"title\":\"AAPL Peer Valuation Analysis\"}"}
```

**`ws-events.jsonl`** — ordered ServerMessage stream zerod emitted
during the original recording:

```json
{"t_offset_ms":42,"type":"invoke_accepted","session_id":"sess-f0e9b78c-...","conversation_id":"..."}
{"t_offset_ms":128,"type":"session_title_changed","session_id":"sess-f0e9b78c-...","title":"AAPL Peer Valuation Analysis"}
```

Hashes let us detect agent drift in strict mode (fail loud if zerod
sends a request that doesn't match the recorded sequence). Lenient mode
matches on `(execution_id, iteration)` only — tolerates minor wording
variance while authoring new fixtures.

Optional `fixture.checksum` file emitted by `record-fixture.py`; mock
servers assert it matches at load time. Catches silent hand-edits that
would break replay.

### 2. `e2e/mock-llm/` — OpenAI-compatible replay server (Mode Full)

~150 LOC FastAPI app. One file.

Endpoints:
- `POST /v1/chat/completions` — reads request body, computes
  `messages_hash`, looks up next response for the current execution,
  streams SSE chunks to match real provider behaviour.
- `GET /v1/models` — returns a stub model per provider the fixture
  declares.
- `GET /__replay/status` — `{ expected_requests, received_requests,
  drift_count, first_drift, per_execution_progress }`.
- `GET /__replay/diff?exec_id=…&iter=N` — returns `{recorded_messages,
  received_messages, diff_summary}` for the offending request.

### 3. `e2e/mock-gateway/` — WS + REST fidelity server (Mode UI)

~300 LOC FastAPI app. Speaks zerod's wire protocol back at the UI.

Endpoints:
- `GET /api/health` — 200 OK
- `GET /api/logs/sessions?limit=N` — returns rows derived from
  `session.json`. Wire-shape quirk preserved: `session_id` holds
  execution id, `conversation_id` holds the sess-* id.
- `GET /api/sessions/:id/messages?scope=all` — returns messages
  including `toolCalls` JSON column with the respond tool invocation.
- `GET /api/sessions/:id/artifacts` — returns artifact rows.
- `GET /api/artifacts/:id/content` — returns a recorded blob from the
  fixture.
- `POST /api/wards/:id/open` — returns 200 (no-op).
- `DELETE /api/sessions/:id` — 204 (no-op), test can assert it was called.
- `GET /api/sessions/:id/state` — 404 (mirrors real gateway).
- WS `/ws` — accepts `subscribe`/`unsubscribe` with scope filtering
  identical to `gateway/src/websocket/subscriptions.rs:should_send_to_scope`.
  Emits scenario events in order, respecting declared `t_offset_ms`
  (or fast-forwards in `compressed` cadence).
- `GET /__replay/status` — same shape as mock-llm.

Emit cadence, chosen per-test:
- `realtime` — honor `t_offset_ms`, full original duration (slow; good
  for time-sensitive behaviour like auto-collapse)
- `compressed` — 5 ms between events (fast default)
- `paced` — custom schedule, e.g. inject a delay or close mid-stream
  for reconnect scenarios

### 4. Tool replay — env-gated Rust wrapper (Mode Full)

~80 LOC new module inside `runtime/agent-tools`. Activated by
`ZBOT_REPLAY_DIR=<fixture>/tools`.

At tool-dispatch time the wrapper:
1. Reads `tool-results.jsonl` (or its indexed in-memory form) at startup.
2. On each tool call, computes `(execution_id, tool_index, tool_name,
   args_hash)` and looks up the canned result.
3. **Hit** → returns the recorded result without calling the underlying
   tool. No file I/O, no shell spawns, no network.
4. **Miss** in strict mode → panics loud with a descriptive message
   (`ZBOT_REPLAY_STRICT=1`, default).
5. **Miss** in lenient mode → falls through to real execution (useful
   while authoring fixtures; `ZBOT_REPLAY_STRICT=0`).

Rationale for replay-only (no real side effects): the harness tests the
UI + event flow. Artifact strip reads from `state.artifacts` which
populates from `/api/sessions/:id/artifacts`, which reads the DB rows
written by the artifact-registration tool — and that tool's result is
replayed. The DB gets correct rows without bytes actually being written
to the vault. Simpler and fully deterministic.

### 5. `e2e/scripts/` — lifecycle helpers

- `boot-ui-mode.sh <fixture>` — random port allocation, spawn
  mock-gateway, spawn UI dev server with env vars pointing at it, wait
  for both to be healthy, print URLs to stdout for the Playwright
  harness to consume.
- `boot-full-mode.sh <fixture>` — temp vault under `/tmp/zbot-e2e-<uuid>/`,
  generate minimal `settings.json` (providers → mock-llm URL; embeddings
  bge-small; MCPs empty), spawn mock-llm, spawn zerod with
  `ZBOT_VAULT=<tmp> ZBOT_REPLAY_DIR=<fixture>/tools ZBOT_REPLAY_STRICT=1`,
  spawn UI pointing at zerod, wait for health.
- `teardown.sh` — SIGTERM each spawned process, `rm -rf /tmp/zbot-e2e-<uuid>/`.
- `record-fixture.py <session-id> <fixture-name>` — one-shot capture:
  given a live session id, exports `session.json` + `llm-responses.jsonl`
  + `tool-results.jsonl` + `ws-events.jsonl` by reading the sqlite DB
  (execution logs, messages, artifacts tables). Recording WS events
  requires running a brief WS sniffer against the live daemon at the
  time of recording; the script prints instructions if the session is
  already completed (use offline-reconstructed ws-events from the DB
  instead).

### 6. `e2e/playwright/` — test runner

- `playwright.config.ts` — chromium only, 1 worker for stability,
  60-90 s timeout per spec, retries=0 (flakiness must be fixed, not
  tolerated).
- `lib/harness-ui.ts` — `bootUIMode(fixture, options)` fixture that
  runs `boot-ui-mode.sh`, exposes `uiUrl()`, `assertZeroDrift()`,
  `captureWsEvents()`, teardown wired to `afterAll`.
- `lib/harness-full.ts` — analogous wrapper for Mode Full.
- `lib/ws-inspector.ts` — page.evaluate snippet that hooks
  `window.WebSocket` before page load and exposes received events for
  assertion.
- `ui-mode/` — specs that only need mock-gateway.
- `full-mode/` — specs that need real zerod.

### Component boundaries

Each component has one responsibility and can be tested independently:

- **mock-llm** ships with a pytest suite validating request matcher,
  streaming, drift detection, schema validation — without zerod.
- **mock-gateway** ships with a pytest suite validating WS subscribe/
  unsubscribe, scope filtering, event cadence — without the UI.
- **tool-replay** has Rust unit tests: `ZBOT_REPLAY_DIR` activates
  wrapper, hit returns canned, strict miss panics, args_hash mismatch
  in strict mode drifts.
- **boot-*.sh** scripts can be run manually; human verifies services
  come up and log their ports.
- **record-fixture.py** has its own pytest: given a seeded DB snapshot,
  produces bit-identical jsonl outputs.
- **Playwright specs** layer on top: if a spec fails, the component
  suite that does *not* fail pinpoints the broken layer.

## Data flow

### Mode UI — event-by-event

Setup:
1. `boot-ui-mode.sh aapl-peer-valuation` starts mock-gateway on random
   port + UI dev server pointing at it, waits for `/api/health` green.
2. Playwright launches chromium, navigates to `/research-v2`.

During the test:
1. UI calls `/api/logs/sessions` — mock-gateway returns the fixture's
   session row + any pre-existing rows declared in the fixture.
2. Test clicks a session in the drawer — UI calls
   `/api/sessions/:id/messages?scope=all` and `/artifacts` — mock
   returns the fixture's derived snapshot. UI renders full state.
3. Test clicks "New Research" — UI clears state, URL → `/research-v2`.
4. Test types prompt + clicks Send — UI opens WS, sends `invoke`.
5. mock-gateway receives the invoke, **starts replaying**
   `ws-events.jsonl` — one event per tick at the selected cadence.
6. UI reducer processes each event, state updates, DOM re-renders.
7. Playwright assertions fire progressively: URL flip within 500 ms,
   title flip, subagent cards appear, respond body, artifact chips.
8. `assertZeroDrift()` hits `/__replay/status`, fails the test if
   the UI sent unexpected requests to the mock.

Teardown: SIGTERM mock-gateway + UI dev server.

### Mode Full — event-by-event

Setup:
1. `boot-full-mode.sh aapl-peer-valuation` creates
   `/tmp/zbot-e2e-<uuid>/vault/`, writes `settings.json` with
   provider `base_url = http://127.0.0.1:$MOCK_LLM_PORT/v1`, starts
   mock-llm, starts zerod with `ZBOT_VAULT=<tmp>` +
   `ZBOT_REPLAY_DIR=<fixture>/tools` + `ZBOT_REPLAY_STRICT=1`, starts
   UI pointing at zerod.

During the test:
1. UI sends `invoke` over WS to real zerod.
2. zerod mints server `session_id` + execution_id, emits
   `invoke_accepted` over WS.
3. zerod's runtime builds messages[] and POSTs to mock-llm.
4. mock-llm matches by `(execution_id, iteration=0, messages_hash)`,
   streams the recorded response.
5. zerod receives tool_calls, schedules them. Each call hits
   tool-replay wrapper → canned result returned without real execution.
6. zerod emits the corresponding WS events (tool_call, tool_result,
   thinking, etc.) to the UI.
7. On `delegate_to_agent`, zerod spawns a child execution; its own
   LLM loop begins, hitting mock-llm with the child's `execution_id`.
8. Loop continues through the delegation tree until `agent_completed`
   for root.
9. UI's reducer processes the stream zerod produced LIVE.

Playwright asserts same DOM state as Mode UI, plus:
- `/_replay/status` on mock-llm shows zero drift.
- `zerod.log` in the temp vault has zero WARN entries.
- Vault DB `artifacts` table has expected rows (queried via
  `/api/sessions/:id/artifacts`).

Teardown: SIGTERM zerod + mock-llm + UI, `rm -rf /tmp/zbot-e2e-<uuid>/`.

### Signal matrix

| Mode UI | Mode Full | Meaning |
|---|---|---|
| ✅ | ✅ | System healthy |
| ❌ | ✅ | UI regression — zerod still emits correctly, reducer/render broke |
| ✅ | ❌ | Backend regression — zerod now emits differently than recorded |
| ❌ | ❌ | UI bug that also manifests with the recorded stream; fix UI first, re-run |

## Error handling

### Mock server failures

| Failure | Detection | Response |
|---|---|---|
| zerod sends an LLM request whose messages don't match the recorded hash | strict-mode `messages_hash` mismatch | HTTP 409 `{"error":"drift","expected":"<hash>","received":"<hash>","exec_id":"…"}` + entry in `drift.log`. Test fails loud. |
| Tool call with no matching fixture entry | `(exec_id, tool_index, tool_name)` miss | Strict: Rust wrapper panics → zerod crashes → Playwright sees WS close → test fails with log dump. Lenient: fall through to real tool (only used while authoring new fixtures). |
| Fixture exhausted (more LLM requests than recorded) | matcher returns None past the last entry | HTTP 410 Gone + drift log. The agent has gone off-script. |
| Malformed fixture | JSON parse failure at startup | Mock process exits non-zero with field path. `boot-*.sh` surfaces it; test suite aborts before first spec. |

### zerod failures (Mode Full only)

| Failure | Detection | Response |
|---|---|---|
| zerod panics | Playwright sees WS close + HTTP 5xx | Dump last 200 lines of `zerod.log` from the vault, fail test. |
| zerod hangs (no events for 30 s) | Per-test timeout | Dump `/__replay/status` + `ps aux | grep zerod` snapshot, fail. |
| Port conflict at boot | `boot-full-mode.sh` health check times out | Retry port allocation up to 3 times, then fail with the actual port. |
| Vault write failure | zerod logs ERROR | Surfaced via log dump; often tmpfs full or replay-dir permissions wrong. |

### UI failures (both modes)

| Failure | Detection | Response |
|---|---|---|
| Expected text/element never appears | Playwright `waitFor` timeout | Screenshot + DOM snapshot saved to `test-results/`. |
| WS connection never established | `page.evaluate(() => window.__wsHook.connected)` returns false | Skip remaining assertions; fail with "WS never connected — check mock-gateway / zerod is up". |
| UI crashed (React error boundary) | Console error assertion | Any `[error]` during the test fails it unless explicitly whitelisted. |
| Console noise (warnings, 404s) | Soft assertion at test end | Logged but don't fail unless the spec explicitly requires a clean console. |

### Setup/teardown failures

| Failure | Detection | Response |
|---|---|---|
| `boot-*.sh` exits non-zero | Exit code check in global-setup | Suite fails before first spec. Error identifies the failing sub-step. |
| `teardown.sh` exits non-zero | Exit code check in global-teardown | Warn but don't fail suite — test results already captured. Stray `/tmp/zbot-e2e-*` swept by scheduled cleanup: `find /tmp -name 'zbot-e2e-*' -mtime +1 -exec rm -rf {} +`. |
| Mid-test process death | Fixture's `afterEach` detects orphan | SIGKILL remaining processes, mark test flaky, continue. |

### Debuggability surface

- `/__replay/status` — consumed index, drift count per execution.
- `/__replay/diff?exec_id=…&iter=N` — recorded vs received messages[]
  for the offending request. Pinpoints where zerod diverged.
- `drift.log` in each test's `test-results/<spec>/` folder with
  timestamp + exec_id + structured diff.
- `zerod.log` captured and attached on failure in Mode Full.

## Testing strategy

### Positive scenarios (3 seeds)

**AAPL Peer Valuation** (`sess-f0e9b78c-...`) — 7 executions, 165+ tool
calls across root + planner + 3 builders + research + writing.
Exercises the deepest delegation tree the system produces today. Each
mode validates URL flip, title flip, subagent card order and content,
artifact chip count, copy buttons, drift=0.

**Operation Praying Mantis 1988 Brief** (`sess-fe5fe944-...`) — 3
executions (root + research + writing). Medium complexity covering the
two-subagent research-then-write shape with artifacts.

**Simple Q&A** (e.g. `sess-96e22dd9-...`) — root-only execution, no
delegation. Validates the trivial-case happy path: user prompt → tokens
→ respond → complete, no subagent cards should ever render.

### Negative / regression scenarios

| Scenario | Base fixture | Mode | Setup | Assertion |
|---|---|---|---|---|
| `new-research-url-silent` | simple-qa | Both | Open existing session → click New Research → send | URL flips to new `/research-v2/:id` within 500 ms + WS events arrive for new session. The bug reported by the user. |
| `ws-disconnect-mid-run` | praying-mantis | UI | mock-gateway closes WS at 30% through stream | R14h reconnect + R14g catch-up snapshot produces full state; subagent card still appears within 2 s of reconnect. |
| `delegation-before-subscribe-ack` | aapl | UI | paced cadence: delegation_started emitted before subscribe ack | R14g catch-up snapshot backfills the subagent card within 2 s. |
| `dropped-session-title` | aapl | UI | mock silently drops `session_title_changed` | Reload / snapshot path recovers title from `/api/logs/sessions`. |
| `subagent-error-banner` | praying-mantis | UI | fixture: one child's status → "error", errorMessage populated | SubagentCard shows ErrorBanner when expanded. |
| `silent-crash-inference` | simple-qa | UI | fixture ends with agent_completed but no respond + empty timeline | Turn's status flips to "error", errorMessage contains "no output". |
| `stale-tool-error-pill` | aapl | UI | inject `tool_result` with error field early, then normal events | Pill turns red initially, clears on next `agent_started`. |
| `second-tab-opens-running` | aapl | UI | two pages on same `/research-v2/:id`, fixture paced as "still running" | Both tabs show the same rendered state; second tab subscribes by session_id. |
| `backend-emits-delegation-started` | aapl | Full only | Real zerod + real delegation | Asserts zerod emitted `delegation_started` at the right position. Tests the backend-emission concern directly. |
| `drift-on-backend-change` | aapl | Full | intentional zerod change during test (e.g. env var that alters tool ordering) | Drift report points at the exact iteration where zerod diverged. Demonstrates the diagnostic surface works. |

### Spec template

Every spec is ~50 LOC. Adding scenario #4 = drop a fixture folder +
copy a template.

```ts
// e2e/playwright/ui-mode/<scenario>.ui.spec.ts
import { test, expect } from "@playwright/test";
import { bootUIMode } from "../lib/harness-ui";

test.describe("<scenario> (UI mode)", () => {
  const h = bootUIMode({
    fixture: "<scenario>",
    cadence: "compressed",
  });

  test("happy path", async ({ page }) => {
    await page.goto(h.uiUrl("/research-v2"));
    await page.fill('textarea', '<prompt from fixture>');
    await page.click('button[title="Send message"]');

    await expect.poll(() => page.url()).toMatch(/\/research-v2\/sess-/, { timeout: 500 });
    await expect(page.locator(".research-page__title"))
      .toHaveText("<expected title>", { timeout: 3000 });
    await expect(page.locator(".subagent-card")).toHaveCount(<n>);

    await h.assertZeroDrift();
  });
});
```

### CI integration

**Phase 1** (this plan): Mode UI specs run on every PR. Mode Full
specs run nightly and on `release/*`. Both in the same GitHub Actions
matrix slot; chromium is the only browser.

**Phase 2**: Mode UI graduates to "must pass before merge"; Mode Full
stays as a nightly smoke gate.

**Phase 3** (opt-in): flake-report aggregation — `drift.log` +
screenshots + `zerod.log` become PR artifacts for failed runs. Drift
reports are the first thing reviewers see when a backend change breaks
fixtures.

### Extension path

- **New fixture**: record live → `record-fixture.py <session-id> <name>`
  produces all four jsonl files. Drop a spec from the template. No
  infra changes.
- **New mode** (e.g. Rust contract tests): new folder consuming the
  same `fixtures/` + `mock-llm/` + `tool-replay/`. mock-gateway is
  UI-only; contract tests wouldn't need it.
- **New regression kind**: extend mock-gateway with an event-injection
  hook (mid-stream timing, synthetic errors); add a spec that uses it.
  No other infra touched.

## Layout

```
e2e/
├── fixtures/
│   ├── aapl-peer-valuation/
│   ├── praying-mantis-1988-brief/
│   └── simple-qa/
├── mock-gateway/
│   ├── server.py
│   ├── replay.py
│   ├── requirements.txt
│   └── tests/
├── mock-llm/
│   ├── server.py
│   ├── replay.py
│   ├── requirements.txt
│   └── tests/
├── scripts/
│   ├── boot-ui-mode.sh
│   ├── boot-full-mode.sh
│   ├── teardown.sh
│   └── record-fixture.py
├── playwright/
│   ├── playwright.config.ts
│   ├── lib/
│   │   ├── harness-ui.ts
│   │   ├── harness-full.ts
│   │   └── ws-inspector.ts
│   ├── ui-mode/
│   │   ├── aapl-peer-valuation.ui.spec.ts
│   │   ├── praying-mantis-happy-path.ui.spec.ts
│   │   ├── simple-qa-happy-path.ui.spec.ts
│   │   ├── regressions/
│   │   │   └── new-research-url-silent.ui.spec.ts
│   │   └── negative/
│   │       ├── ws-disconnect-mid-run.ui.spec.ts
│   │       ├── delegation-before-subscribe-ack.ui.spec.ts
│   │       ├── dropped-session-title.ui.spec.ts
│   │       ├── subagent-error-banner.ui.spec.ts
│   │       ├── silent-crash-inference.ui.spec.ts
│   │       ├── stale-tool-error-pill.ui.spec.ts
│   │       └── second-tab-opens-running.ui.spec.ts
│   └── full-mode/
│       ├── aapl-peer-valuation.full.spec.ts
│       ├── praying-mantis-happy-path.full.spec.ts
│       ├── simple-qa-happy-path.full.spec.ts
│       └── regressions/
│           ├── new-research-url-silent.full.spec.ts
│           └── backend-emits-delegation-started.full.spec.ts
└── .gitignore

runtime/agent-tools/src/replay.rs   ← new module for tool replay wrapper
```

`.gitignore` entries: `tmp-vault-*/`, `recorded-runs/`, `mock-*/__pycache__/`,
`node_modules/`, `test-results/`, `playwright-report/`, `coverage/`.
Fixture bundles under `fixtures/` ARE committed (they are the test
data).

## Success criteria

- [ ] All 3 positive scenarios run green in both modes on first attempt.
- [ ] Each spec completes in under 90 s on CI (p95).
- [ ] Adding a 4th scenario requires only a new fixture folder + a new
      spec file from the template — no changes to mock servers, scripts,
      or harness.
- [ ] The `new-research-url-silent` regression spec fails in Mode Full
      until the underlying bug is fixed, and passes in Mode UI
      (demonstrating the two-mode diagnostic split works).
- [ ] Zero flakes over 100 consecutive CI runs for the 3 positive
      scenarios.
- [ ] Drift-log output is human-readable and points at the diverging
      iteration in under a paragraph.
- [ ] Existing live-testing workflow unchanged — harness is additive.
- [ ] No real LLM API keys required by any test in the suite.

## Out of scope (deferred)

- Fuzz-style LLM simulation (harness replays canned responses only).
- Performance / load testing (separate harness).
- Visual regression (pixel diffs).
- Database memory + knowledge graph correctness (user explicit).
- MCP integration testing (stays stubbed empty in test config).

## Phasing recommendation

**Phase 1** (this plan — ~2 weeks):
1. Mock-llm + mock-gateway + tool-replay wrapper + record-fixture.py
2. Three positive scenarios (AAPL, Praying Mantis, Simple Q&A) green
   in both modes.
3. `new-research-url-silent` regression wired in both modes.
4. CI runs Mode UI on every PR; Mode Full nightly.

**Phase 2** (follow-up):
- Remaining 7 negative scenarios.
- Flake-report aggregation + drift artifacts on failed PRs.
- Graduate Mode UI to pre-merge gate.

**Phase 3** (as needed):
- Rust contract tests reusing fixtures + mock-llm.
- New fixtures recorded from production incidents as they occur.

## Open items

None blocking. Recording ws-events for already-completed sessions will
need a small helper — either replay DB logs into a mock event stream,
or require a live-daemon WS sniffer for new recordings. Decision
deferred to Phase 1 implementation.
