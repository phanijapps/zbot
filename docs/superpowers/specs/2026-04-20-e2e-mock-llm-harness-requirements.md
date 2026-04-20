# E2E Mock-LLM Test Harness — High-Level Requirements

**Date:** 2026-04-20
**Author:** Phani + Claude
**Status:** Draft

---

## Problem

UI regressions in `/chat-v2` and `/research-v2` have been caught only by
live manual testing against a real backend + real LLM. Each cycle takes
30–300 seconds per prompt and burns provider quota. Bugs tied to event
timing (sequence gaps, WS reconnects, subagent deltas, title updates) are
non-deterministic — we cannot reproduce them on demand, and we cannot
assert on them in CI.

We need a **hermetic, scripted test harness** that lets Playwright drive
the UI against a mock LLM + mock gateway, producing fully deterministic
scenarios for happy paths, edge cases, and failure modes.

## Goals

1. **Deterministic e2e tests** for `/chat-v2` and `/research-v2` that run
   in CI in under 60 seconds each without network egress.
2. **Scriptable scenarios** — a scenario file declares the sequence of
   gateway events and tool responses; the harness replays them to the UI
   over a real WebSocket.
3. **Round-trip fidelity** — events emitted match the real gateway's wire
   format verbatim (type, field names, routing metadata, seq numbers).
4. **Failure-mode coverage** — WS ping-timeout disconnects, sequence gaps,
   partial deliveries, dropped `delegation_started`, missing
   `session_title_changed`, tool errors, agent stall.
5. **No code changes to the UI under test** — the UI connects to the same
   `ws://` and HTTP endpoints it uses in production; only the backing
   server differs.

## Non-goals

- Simulating actual LLM reasoning quality. The mock returns canned text.
- Validating the real gateway's behavior. That's covered by gateway's own
  Rust tests.
- Cross-browser matrix. Chromium only.
- Mobile viewport testing.

## Users

1. **UI engineers** writing features in `apps/ui/`. Run `npm run test:e2e`
   and get a green/red signal before commit.
2. **CI** running on every PR touching UI code.
3. **Future AI agents** (coding assistants) iterating on UI features with
   a reliable feedback loop.

## Architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│                          Playwright runner                             │
│  - Loads scenario.json for the test                                    │
│  - Starts mock-gateway server on 127.0.0.1:<random>                    │
│  - Launches headless Chromium                                          │
│  - Points UI at the mock via VITE_WS_URL / VITE_HTTP_URL               │
│  - Drives UI interactions, asserts DOM state                           │
└───────────────────────┬───────────────────────────────────────────────┘
                        │
                        ▼
┌───────────────────────────────────────────────────────────────────────┐
│   Mock Gateway Server (Node/TS, reuses gateway-events type defs)      │
│  - HTTP: /api/logs/sessions, /api/sessions/:id/messages, /artifacts,  │
│          /api/chat/init, /api/wards/:id/open, etc.                    │
│  - WS:   subscribe/unsubscribe, ping/pong, invoke, stop               │
│  - Scenario engine: reads scripted event timeline, emits over WS      │
│  - State store: in-memory records of sessions, messages, artifacts    │
└───────────────────────────────────────────────────────────────────────┘
                        ▲
                        │
┌───────────────────────────────────────────────────────────────────────┐
│                          UI under test                                 │
│  - Unmodified production build                                        │
│  - Connects to mock via env-configured URLs                           │
└───────────────────────────────────────────────────────────────────────┘
```

## Mock Gateway requirements

### HTTP surface (subset sufficient for UI)

- `GET  /api/health` — always 200
- `POST /api/chat/init` — returns reserved chat session/conv IDs
- `DELETE /api/chat/session` — clears reserved session (noop ok)
- `GET  /api/logs/sessions?limit=N` — returns in-memory session rows
- `GET  /api/sessions/:id/messages?scope=all|root|delegates` — returns
  messages tagged with execution_id, including `toolCalls` JSON
- `GET  /api/sessions/:id/artifacts` — returns artifact records
- `GET  /api/artifacts/:id/content` — serves static fixture bytes
- `POST /api/wards/:id/open` — returns 200 (does not actually launch)
- `GET  /api/sessions/:id/state` — returns 404 (mirrors real behavior)

Responses match the wire format documented in
`apps/ui/src/services/transport/types.ts` (`LogSession`,
`SessionMessage`, `Artifact`). Camel/snake-case quirks preserved.

### WebSocket surface

- Accept `subscribe` / `unsubscribe` with `conversation_id` + optional
  `scope` ("all" | "session" | "execution")
- Assign monotonic `seq` per conversation subscription, exactly like
  `gateway/src/websocket/subscriptions.rs:route_event_scoped`
- Apply scope filter identically: session scope passes delegation events
  and session-level events + root execution events only
- Send `pong` in response to `ping`; supports optional pong-drop mode
  to trigger client ping-timeout reconnect
- Emit scenario events in order, respecting declared delays

### Events supported

Full `ServerMessage` variant coverage from
`gateway/gateway-ws-protocol/src/messages.rs`:
`agent_started`, `agent_completed`, `agent_stopped`, `thinking`,
`token`, `tool_call`, `tool_result`, `turn_complete`, `error`,
`respond`, `delegation_started`, `delegation_completed`,
`session_title_changed`, `ward_changed`, `intent_analysis_started`,
`intent_analysis_complete`, `invoke_accepted`, `heartbeat`.

### Scenario file format (TypeScript)

```ts
type Scenario = {
  name: string;
  // Initial REST state seeded before the UI connects.
  initialSessions: LogSession[];
  initialMessages: Record<string /*sessionId*/, SessionMessage[]>;
  initialArtifacts: Record<string, Artifact[]>;
  // WS timeline: what to emit when, to whom.
  timeline: TimelineStep[];
  // Defaults: ping behavior, invoke ack latency, disconnect plans.
  transport?: {
    pingPongMode?: "normal" | "drop-after" | "flap";
    invokeAckDelayMs?: number;
    disconnectAfterMs?: number;
  };
};

type TimelineStep =
  | { at: number; emit: ServerMessage; scope?: "all" | "session" | "execution" }
  | { at: number; drop: true;        reason: "scope-filter" | "disconnect" }
  | { at: number; action: "close-ws" | "reopen-ws" | "stop-pong" | "resume-pong" };
```

A scenario is a plain TS module that exports `default` as a `Scenario`.

## Required scenarios

The harness ships with a baseline library. Each is one Playwright
spec file under `apps/ui/tests/e2e/scenarios/`.

### Quick Chat (`/chat-v2`)

1. **Happy path** — user prompts, tokens stream, respond arrives, final
   bubble renders with copy + artifact (if any).
2. **Multi-turn** — two consecutive user prompts, each gets its own
   streaming assistant bubble.
3. **Respond via `respond` tool** — `turn_complete.final_message` is
   empty; `tool_call.args.message` carries the answer; UI still renders
   it as the final bubble.
4. **Tool error** — `tool_result.error` fires; pill turns red with error
   text; sticky until next `agent_started`.
5. **Chat-init race** — two parallel `/api/chat/init` calls return the
   same session; UI does not duplicate messages.
6. **Reload mid-run** — simulate a reload while streaming; hydrate pulls
   the partial tail from the message log and renders cleanly.

### Research (`/research-v2`)

1. **Happy path, single root** — user prompt → streaming tokens → respond
   tool → final markdown + artifact chip. No subagent.
2. **Single subagent delegation** — `delegate_to_agent` tool_call →
   `delegation_started` → child `agent_started` → child tool_calls →
   child `delegation_completed` → root `agent_completed` + final
   respond. UI shows 1 subagent card with Request + Response, auto-
   collapsed at end.
3. **Multiple serial subagents** — planner → writer. Two cards,
   chronologically ordered. Second card mounted only after first
   completes.
4. **Subagent fails** — delegation_completed carries an error; child
   card shows Error banner; root still produces final respond.
5. **Title flip** — agent calls `set_session_title` at t=3s; header
   title and browser tab flip to the AI-generated title; sessions
   drawer row renames.
6. **Ward chip** — agent calls `ward` tool; chip appears; click opens
   ward folder (POST to `/api/wards/:id/open`).
7. **Artifact strip** — agent writes a file; artifact chip appears
   above composer; click opens slide-out.
8. **WS ping timeout mid-run** — `stop-pong` at t=5s; transport
   reconnects; events 2-4 dropped; `delegation_started` was in the
   dropped range. R14g + R14i catch-up snapshot populates the subagent
   card after reconnect. Assert card appears within 2s of reconnect.
9. **Dropped `session_title_changed`** — event scope-filtered; UI still
   recovers title via the reconnect-hydrate or the post-completion
   re-snapshot.
10. **Second tab opens running session** — tab A starts session; tab B
    opens `/research-v2/:sessionId`; tab B must show the current state
    and continue receiving live deltas via its own session-scope sub.
11. **Sequence gap at subscribe ack** — session-scope subscribe acks at
    seq 4, events 2 and 3 were first delegation's lifecycle; catch-up
    snapshot fills the child card.
12. **Hydration with only partial respond** — agent never called
    `respond` tool; hydrate shows "waiting…" with no fake content.
13. **`[tool calls]` placeholder with respond** — assistant row's
    `toolCalls` column contains a respond entry; hydrate upgrades
    content from `args.message`.
14. **Subagent never emits on its own conv_id** — child execution
    runs, root stays subscribed on session_id; child's events arrive
    via session scope only.

### Shared failure scenarios

15. **Server-side 500 on invoke** — UI dispatches ERROR, session does
    not get wedged in "running".
16. **WS refuses subscribe (not_found error)** — UI surfaces error
    without crashing.
17. **Many concurrent scenarios running** (browser-level isolation test)
    — N=5 parallel pages, each with its own scenario file, no state
    bleed.

## Success criteria

- [ ] Each scenario above runs in <15s on CI (p95), <5s (p50).
- [ ] Zero flakes over 100 consecutive runs for scenarios 1-7 of each
      page. (Scenarios 8+ are failure-mode — still deterministic given
      fixed-seed scheduling.)
- [ ] New UI features can be added with a new scenario file + one spec
      file, no harness changes.
- [ ] Chromium only. No external network. No LLM tokens consumed.
- [ ] Existing live-testing workflow still works unchanged (dev can run
      `npm run dev` + `zerod` as today; harness is additive).

## Out of scope (future work)

- Mock real LLM reasoning (fuzz-style). We pre-canned responses only.
- Performance/load testing. Use a separate harness.
- Visual regression (pixel diffs). Optional follow-up.
- Multi-user / multi-session isolation on a shared server.

## Implementation sketch (non-binding)

Suggested layout:

```
apps/ui/tests/e2e/
├── harness/
│   ├── mock-gateway.ts          — HTTP + WS server
│   ├── scenario-engine.ts       — timeline player
│   ├── fixtures/                — static artifact payloads, etc.
│   └── wire-types.ts            — re-exports from transport/types
├── scenarios/
│   ├── chat-v2/
│   │   ├── 01-happy-path.scenario.ts
│   │   └── ...
│   └── research-v2/
│       ├── 01-happy-path.scenario.ts
│       ├── 08-ping-timeout.scenario.ts
│       └── ...
└── specs/
    ├── chat-v2.spec.ts          — one describe per scenario
    └── research-v2.spec.ts
```

Mock server: ~400 LOC Node/TS. Scenario engine: ~200 LOC. Scenarios:
~80 LOC each. Playwright specs: ~50 LOC each (load scenario, drive,
assert).

Wire-type fidelity is maintained by generating `wire-types.ts` from
`gateway/gateway-ws-protocol/src/messages.rs` via a one-shot
`ts-rs` or by hand-mirroring (acceptable — the Rust side is stable).

## Open questions

1. Where does the mock server live — inside `apps/ui/tests/` or a new
   top-level `tools/mock-gateway/` package? Recommendation: keep inside
   `apps/ui/tests/` for now to minimize surface. Promote if other
   consumers appear.
2. How to seed ward state for ward-chip / open-folder scenarios?
   Recommendation: no real disk; mock `POST /api/wards/:id/open` with a
   200 + logged spy assertion.
3. How to simulate artifact content bytes? Static fixtures under
   `fixtures/` shipped with the scenario referencing them by id.

## Done when

The three existing live-smoke bugs (subagent panel missing,
title-not-flipping, session-stuck-on-running) all have a failing
scenario that turns green with the current code. Any regression in
those areas would then fail CI before reaching manual testing.
