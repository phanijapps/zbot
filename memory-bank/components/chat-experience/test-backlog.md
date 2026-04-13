# Chat Experience / UI — Unit Test Backlog

## Current State

| Category | Count | Files |
|----------|-------|-------|
| Unit tests | 7 | `types.test.ts`, `http.test.ts`, `ThinkingBlock.test.tsx`, `artifact-utils.test.tsx`, `trace-types.test.ts`, `SourceBadge.test.tsx`, `setup.test.ts` |
| Integration tests | 1 | `dashboard.test.tsx` |
| E2E tests (Playwright) | 11 | `tests/e2e/*.spec.ts` |
| Source files needing tests | ~95 | |

### Test Infrastructure

- **Runner:** Vitest 3.2 + jsdom
- **Mocking:** MSW 2.8 (Mock Service Worker) for API, `vi.mock()` for modules
- **Component testing:** `@testing-library/react` 16 + `@testing-library/user-event` 14
- **Custom render:** `src/test/utils.tsx` wraps `BrowserRouter`
- **Mock factories:** `createMockSession()`, `createMockStats()` in `src/test/mocks/handlers.ts`
- **Coverage:** `@vitest/coverage-v8` with `npm run test:coverage`

### Testing Conventions

1. Named imports from vitest: `import { describe, it, expect } from 'vitest'`
2. Component tests use `@/test/utils` render helper
3. API tests use MSW `server` + `http` overrides for error scenarios
4. Section headers with `// ============` separators for large test files
5. Colocation: test files live next to source (`Component.test.tsx`)
6. `it.each` for parameterized tests (see `SourceBadge.test.tsx`)
7. `data-testid` for element targeting

---

## Phase 1: Pure Functions & Utilities

**Status:** Not started
**Effort:** Low
**Est. Tests:** ~48

No DOM, no hooks, no API calls. Immediate coverage boost.

| # | File | Functions to Test | Tests |
|---|------|-------------------|-------|
| 1 | `features/chat/mission-hooks.ts` | `timeAgo()` — relative time formatting for recent/minutes/hours/days | 8 |
| 2 | `features/chat/SessionBar.tsx` | `formatDuration()`, `formatTokens()`, `sessionStatusClass()` | 12 |
| 3 | `features/chat/RecallBlock.tsx` | `parseRecall()` — JSON parsing with graceful fallback to raw text | 5 |
| 4 | `features/chat/ToolExecutionBlock.tsx` | `SHELL_TOOLS` set membership, shell tool detection logic | 3 |
| 5 | `features/chat/IntentAnalysisBlock.tsx` | `APPROACH_LABELS` mapping — all approach keys have labels | 4 |
| 6 | `features/chat/artifact-utils.tsx` | `getArtifactIcon()` mapping (extend existing tests) | 4 |
| 7 | `shared/utils/format.ts` | `formatContextWindow()` — bytes, KB, MB, GB, null | 5 |
| 8 | `features/settings/providerPresets.ts` | `PROVIDER_PRESETS` structure, `getAvailablePresets()` filtering | 4 |
| 9 | `features/setup/presets.ts` | `NAME_PRESETS` data — all presets have name + emoji, Custom has empty name | 3 |

---

## Phase 2: Component Rendering Tests

**Status:** Not started
**Effort:** Medium
**Est. Tests:** ~105

Test components render correctly with various props, states, and edge cases.

| # | File | Test Scenarios | Tests |
|---|------|---------------|-------|
| 10 | `features/chat/PhaseIndicators.test.tsx` | Renders all 4 phases; correct step status (done/active/pending/error) per Phase value; active step highlighting; subagent count during executing | 10 |
| 11 | `features/chat/ExecutionNarrative.test.tsx` | Renders user/response blocks; filters non-user/response blocks; PhaseIndicators on latest turn only; empty state; auto-scroll behavior | 6 |
| 12 | `features/chat/RecallBlock.test.tsx` | Renders corrections/episodes/domain facts from parsed JSON; collapse toggle when >6 items; empty recall; malformed JSON fallback | 6 |
| 13 | `features/chat/PlanBlock.test.tsx` | Renders steps with done/active/pending icons; empty plan; single step; many steps | 5 |
| 14 | `features/chat/DelegationBlock.test.tsx` | Renders active/completed/error states; tool call count display; duration; token display; missing result | 6 |
| 15 | `features/chat/AgentResponse.test.tsx` | Renders markdown content via ReactMarkdown; empty content; long content; special characters | 4 |
| 16 | `features/chat/UserMessage.test.tsx` | Renders text content; attachment chips when present; empty message; special characters | 4 |
| 17 | `features/chat/ToolExecutionBlock.test.tsx` | Shell tool (bash/shell/terminal/execute_command) gets terminal style; non-shell tools get collapsible format; expand/collapse; long output truncation | 6 |
| 18 | `features/chat/IntentAnalysisBlock.test.tsx` | Streaming skeleton when loading; complete data rendering; approach label mapping; hidden intents list; ward recommendation; execution strategy display | 7 |
| 19 | `features/chat/IntelligenceFeed.test.tsx` | Renders 5 collapsible sections; collapse/expand toggle; artifact loading via transport; subagent cards with tool details; empty sections gracefully hidden | 10 |
| 20 | `features/chat/SessionBar.test.tsx` | Title display; status dot color; metrics formatting (tokens/duration/model); stop button callback; new session button; history dropdown open/close; click-outside close | 10 |
| 21 | `components/Slideover.test.tsx` | Opens with content; escape key closes; backdrop click closes; body scroll lock; header/body/footer sections | 5 |
| 22 | `components/TabBar.test.tsx` | Tab switching; count badges; ARIA roles (tablist/tab/tabpanel); TabPanel conditional render; active tab styling | 5 |
| 23 | `components/ConnectionStatus.test.tsx` | Connected/disconnected/reconnecting display; reconnect button callback; retry button callback | 5 |
| 24 | `components/ActionBar.test.tsx` | Search input renders; filter chips display; action buttons render; onChange callbacks | 5 |
| 25 | `components/MetaChip.test.tsx` | All 16 variants render; className pass-through; icon + label display | 6 |
| 26 | `shared/ui/EmptyState.test.tsx` | Size variants (sm/md/lg); action button click; hint text; icon rendering; empty children | 5 |

---

## Phase 3: Hook Tests

**Status:** Not started
**Effort:** High
**Est. Tests:** ~84

Most complex modules. Use `renderHook` from `@testing-library/react` with mocked transport.

### 3A: `mission-hooks.ts` — Core Chat Hook (1369 lines)

| # | Test Suite | Scenarios | Tests |
|---|-----------|-----------|-------|
| 27 | Session lifecycle | Fresh session creates conv ID; localStorage management (`agentzero_web_conv_id`, `agentzero_web_session_id`); URL param `?new=1` triggers reset; session resumption from `getSessionState()` API | 8 |
| 28 | sendMessage | Appends user block; sets status=running; calls `executeAgent()` with correct params; double-submission guard (isSubmittingRef); attachment markdown table in message | 6 |
| 29 | Phase state machine | idle→intent→planning→executing→responding→completed; error transition at any phase; skipped intent (intent_analysis_skipped); phase only on latest turn | 8 |
| 30 | Event handler — token | RAF buffer accumulation; flush on turn_complete; token count tracking; streaming response assembly | 4 |
| 31 | Event handler — tool_call | Routing by tool name: `set_session_title` updates title, `memory/recall` creates recall block, `update_plan` creates plan block, `respond` creates response block, `delegate_to_agent` creates delegation block, generic creates tool block | 10 |
| 32 | Event handler — tool_result | Correlates via toolCallBlockMap to correct block; updates recall/tool/delegation/plan block data | 5 |
| 33 | Event handler — delegation | `delegation_started` adds subagent with running status; `delegation_completed` marks done with result; `delegation_error` sets error state | 5 |
| 34 | Event handler — session/title | `invoke_accepted` captures session ID to localStorage; `agent_started` starts duration timer + fallback title timer; `session_title_changed` updates title + clears fallback timer; `turn_complete` flushes all buffers | 6 |
| 35 | Event handler — ward/intent | `ward_changed` updates active ward; `intent_analysis_started/complete/skipped` manage intent analysis state | 4 |
| 36 | Event handler — completion/error | `agent_completed` sets completed + adds response if missing; `error` sets error state; `system_message`/`message` add response block | 4 |
| 37 | Duration timer | Starts on agent_started; updates every 500ms; stops on completed/error; cleanup on unmount | 3 |
| 38 | startNewSession | Resets all state; clears localStorage; generates fresh conversation ID; resets refs | 4 |
| 39 | switchToSession | Loads session state from API; restores blocks from snapshot; subscribes to WS if live | 4 |

### 3B: `fast-chat-hooks.ts` (682 lines)

| # | Test Suite | Scenarios | Tests |
|---|-----------|-----------|-------|
| 40 | Session init | POST /api/chat/init on mount; history loading from GET /api/sessions/{id}/messages?limit=100; message role mapping (user/assistant/tool) | 4 |
| 41 | Message handling | Send message via transport; dual streaming buffers (token + thinking); thinking/reasoning event handling | 6 |
| 42 | Artifact loading | Fetches artifacts on agent_completed; stable ref for session/conversation IDs avoiding stale closures | 2 |

### 3C: Other Hooks

| # | Hook | Scenarios | Tests |
|---|------|-----------|-------|
| 43 | `hooks/useTheme.test.ts` | Light/dark/system cycle; localStorage persistence (`agentzero-theme`); `matchMedia` listener for system preference; toggles `dark` class on `documentElement` | 6 |
| 44 | `hooks/useConnectionState.test.ts` | Subscribes to transport connection state; callback on disconnect/reconnect; cleanup on unmount | 3 |
| 45 | `hooks/useConversationEvents.test.ts` | Subscribe via `transport.subscribeConversation()`; unsubscribe cleanup; callback ref stability avoids re-subscribe | 3 |

---

## Phase 4: Transport Layer Unit Tests

**Status:** Not started
**Effort:** High
**Est. Tests:** ~42

### 4A: `http.ts` — HttpTransport (1642 lines)

| # | Test Suite | Scenarios | Tests |
|---|-----------|-----------|-------|
| 46 | HTTP methods | `get<T>()`, `post<T>()`, `put<T>()`, `delete()` with correct headers/body; 5s default timeout / 30s for long ops; AbortController on timeout; error wrapping into `TransportResult<T>` | 10 |
| 47 | WebSocket lifecycle | Connect with correct URL; heartbeat ping every 15s; pong timeout (30s) triggers reconnect; `visibilitychange` hidden→visible triggers reconnect; `online` browser event triggers reconnect; close cleanup | 8 |
| 48 | Subscription system | `subscribeConversation()` sends subscribe message; scope conversion (`"execution:exec-456"` → `{execution: "exec-456"}`); callback receives filtered events; resubscribe all on reconnect; unsubscribe removes callback + sends unsubscribe | 8 |
| 49 | Event dedup | `recentEvents` Set prevents duplicate processing; max 200 entries with FIFO eviction; sequence gap detection with console warnings for `all` scope | 4 |
| 50 | Reconnection logic | Exponential backoff between attempts; unlimited retries during `hasActiveExecution`; synthetic `reconnected` events emitted during active execution; active execution tracking (set on `agent_started`, clear on `agent_completed`/`turn_complete`) | 6 |

### 4B: `transport/index.ts` — Singleton Management (177 lines)

| # | Test Suite | Scenarios | Tests |
|---|-----------|-----------|-------|
| 51 | Factory & singleton | `createTransport()` creates HttpTransport with config; `getTransport()` returns singleton; `initializeTransport()` sets up singleton; `isTransportInitialized()` boolean; `resetTransport()` clears singleton; config resolution: `window.__ZERO_CONFIG__` → URL params → defaults (`http://localhost:18791`, `ws://localhost:18790`) | 6 |

---

## Phase 5: Feature Panel Integration Tests

**Status:** Not started
**Effort:** Medium-High
**Est. Tests:** ~56

Larger feature panels tested with MSW-mocked API calls.

| # | File | Test Scenarios | Tests |
|---|------|---------------|-------|
| 52 | `features/setup/SetupWizard.test.tsx` | Step navigation 1→2→3→4→5→6; `canNext()` validation per step (name required, verified provider required); Back button; Skip on skippable steps; hydration from API on re-run; delta detection | 10 |
| 53 | `features/setup/SetupGuard.test.tsx` | Redirects to `/setup` when `setupComplete=false` AND no providers; passes through when complete; sessionStorage caching avoids re-fetch; skips check when already on `/setup` route | 5 |
| 54 | `features/setup/steps/ReviewStep.test.tsx` | 7 sequential API operations on launch (load agents, update name, set default provider, update changed configs, create new MCPs, save about-me fact, mark setup complete); error handling mid-sequence; delta detection (only changed items updated) | 8 |
| 55 | `features/settings/WebSettingsPanel.test.tsx` | Tab switching via URL search params (`?tab=providers`); provider CRUD via slideover; logging settings save; execution settings save (orchestrator/multimodal/distillation); form validation | 8 |
| 56 | `features/memory/WebMemoryPanel.test.tsx` | List facts with agent filter; category/scope filters; search; pagination; add fact (policy/instruction/about-me); delete fact | 8 |
| 57 | `features/integrations/WebIntegrationsPanel.test.tsx` | MCP server CRUD via slideover; test MCP connection; env var conversion (`recordToEnvVars`/`envVarsToRecord`); plugin list rendering; bridge worker polling; search/filter | 8 |
| 58 | `features/logs/ObservabilityDashboard.test.tsx` | Session list rendering with filter params; session click opens detail; mini waterfall in session rows; running session polling | 5 |
| 59 | `features/observatory/ObservatoryPage.test.tsx` | Graph canvas renders entities; entity detail panel; learning health bar display | 4 |

---

## Mock Strategy

| Module | Mock Strategy |
|--------|--------------|
| Transport (`getTransport()`) | `vi.mock('@/services/transport', ...)` returning method stubs |
| `localStorage` | jsdom provides native impl; `vi.clearAllMocks()` or manual `localStorage.clear()` between tests |
| `WebSocket` | `vi.fn()` or `mock-socket` library |
| `requestAnimationFrame` | `vi.spyOn(window, 'requestAnimationFrame', cb => cb())` for synchronous flush |
| `matchMedia` | `Object.defineProperty(window, 'matchMedia', { value: vi.fn() })` |
| File upload (`POST /api/upload`) | MSW handler in `src/test/mocks/handlers.ts` |
| API endpoints | Extend existing MSW handlers in `src/test/mocks/handlers.ts` |
| `window.location` | `vi.spyOn(window, 'location', ...)` for URL param tests |
| Timers | `vi.useFakeTimers()` for duration timer (500ms interval), heartbeat (15s), reconnection backoff |

## Recommended Execution Order

1. **Phase 1** — Pure functions, validates test infra, immediate coverage
2. **Phase 2** — Component rendering, builds confidence
3. **Phase 3A** — mission-hooks (highest value, most critical logic)
4. **Phase 4A** — HttpTransport (second highest, WS complexity)
5. **Phase 3B-C** — Remaining hooks
6. **Phase 5** — Feature panels
7. **Phase 4B** — Transport singleton (small, quick)

## Summary

| Phase | Focus | Tests | Effort |
|-------|-------|-------|--------|
| 1 | Pure functions & utilities | ~48 | Low |
| 2 | Component rendering | ~105 | Medium |
| 3 | Hook logic | ~84 | High |
| 4 | Transport layer | ~42 | High |
| 5 | Feature panel integration | ~56 | Medium-High |
| **Total** | | **~335** | |
