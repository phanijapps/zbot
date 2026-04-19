# Chat + Research UI Redesign — Design

**Date:** 2026-04-19
**Status:** design, pre-implementation
**Branch:** `feature/ui-chat-research-redesign`

## Vision

Two new pages replacing the current `/chat` (FastChat) and `/` (WebChatPanel / MissionControl). Each ships alongside the old one until the new one is green; then the old retires.

- **Chat** — ephemeral, single-session, memory-aware quick interface. Delegates to at most one subagent for bounded tasks. No planner. Claude-minimal aesthetic.
- **Research** — durable, multi-session, full agent-chain workbench. Drawer-toggle layout today, designed so the sessions list can migrate to a topbar later without a rewrite.
- **Shared** — a rolling status-pill component that sits in a top strip across both pages. Shows the agent's live narration with a muted tool suffix. Swaps content as actions change. Visible only while a session is running.

## Principles

1. **The ward is an app-level concept.** Chat inherits the active ward for memory-recall scoping; Research infers it per session. Neither page owns ward selection.
2. **Session state is authoritative on the server.** The UI subscribes to a WebSocket event stream and holds a derived view. On reconnect, the UI re-derives from a snapshot.
3. **One block per agent turn.** All internal events (`Thinking`, `ToolCall`, `ToolResult`) between `AgentStarted` and `AgentCompleted` collapse into a single expandable "Thinking" block, anchored by the final `Respond` message. No event ping-pong in the conversation view.
4. **Feature-flag new pages alongside old ones.** Routes `/chat-v2` and `/research-v2` during shakeout; retire `/chat` (FastChat) and `/` (WebChatPanel) in a later cleanup when the new ones are trusted.
5. **Seams for later evolution.** The sessions list is a presentation-agnostic component; a future topbar migration swaps its render mount without touching its data layer.

## Chat — Spec

### Role

The user's quick-response surface. Memory-based Q&A, skill calls, single-step delegation. Not a research workbench — when a question would need multi-step orchestration, Chat refers the user to Research.

### Agent

A dedicated **`quick-chat`** agent, new in this redesign.

- **Prompt surface.** System prompt optimized for conversational responses that use memory recall first, skills second, single-delegation third. Explicit guardrail: "Do not invoke the planner. If the task needs multi-step orchestration, tell the user to move to Research."
- **Tool allowlist.** `memory` (recall / get_fact / save_fact), `load_skill`, `delegate_to_agent` (at most once per turn, soft-enforced via prompt), `ward` (use / info), `grep` (read-only file probes), `respond`, `multimodal_analyze`, `graph_query`, `ingest`. **Not allowed:** `update_plan`, `set_session_title` (root-only elsewhere), and in practice the agent simply will not invoke `planner-agent`.
- **Single-delegation enforcement.** Soft via prompt for v1. If a future version needs hard enforcement, the tool registry can remove `delegate_to_agent` from the allowlist after the first call in a turn; not in scope now.

### Session lifecycle

- **Roll trigger.** User-visible "New chat" button in the top-right of the Chat page. Clicking it discards the current session (server-side archive, not deleted from DB — accessible via Logs page if needed) and creates a fresh one.
- **Default state.** When the user first lands on `/chat-v2` with no active chat session, the app auto-creates one tagged `mode=quick-chat` bound to the active ward.
- **Ephemerality.** No visible history panel. "New chat" is the only way to end-of-life the current session from within Chat. Past chats are not surfaced in the Chat page itself.
- **Tab close.** Session persists server-side. Reopening `/chat-v2` restores the same session (state snapshot via `GET /api/sessions/:id/state`), resubscribes to its event stream, and shows the conversation as it stands.

### View

- **Header.** Left: active-ward chip (read-only; mirrors whatever the app-shell ward switcher has set). Right: `⊕ New chat` button.
- **Status pill strip.** Full-width slim strip directly under the header. Invisible when no agent is active. Visible and animating when the quick-chat agent (or the one subagent it delegated to) is working. See "Shared — Status pill" section for content/behavior.
- **Conversation area.** Centered single column, max-width ≈ 720px. Messages rendered top-to-bottom in chronological order. When the page first loads (or the user navigates to `/chat-v2` from elsewhere), the viewport opens **anchored at the last user message** — older turns are collapsed behind a `↑ Show N earlier turns` affordance at the top of the column. Clicking the affordance lazy-loads the next batch (default 10) by calling `GET /api/sessions/:id/messages?before=<cursor>&limit=10`.
- **Composer.** Anchored at the bottom of the column. Multi-line textarea, `Enter` to send, `Shift+Enter` for newline. Attach-file button to the left of the textarea. Send button to the right. Paste images supported (inserted inline as an attachment card above the textarea).
- **Assistant replies.** Rendered in a single bubble per turn. Inline activity chips appear at the end of the reply, color-coded:
  - `🧠 recalled X` — blue — click expands to show the recalled fact key + score
  - `📚 loaded <skill>` — purple — click expands to show which skill file was loaded
  - `→ <agent-id>` — orange — click expands to show the delegated task + returned respond() message
- **Empty state** (after "New chat" or on first ever visit): centered "Quick chat" title, one-line subtext (`memory-aware · single-step delegation · bound to <ward-name>`), composer below. No suggestion chips.
- **Artifact slide-out.** If the quick-chat agent (or a subagent it delegated to) produces an artifact — an HTML report, a markdown memo, a JSON file — the artifact card appears inline in the assistant reply with a "View" button. Clicking opens a right-edge slide-out panel using the existing `ArtifactSlideOut` component. The conversation column shrinks to make room; no overlay.

### Out of scope

- Citations, sources panel (Chat is not for research).
- Session search or history sidebar (ephemeral by design).
- Voice input (future).

---

## Research — Spec

### Role

The durable, multi-session research workbench. Runs the full agent chain (`planner → solution → builder → writer`) against ward-scoped problems, produces artifacts, and supports long-running work that survives tab close.

### Session model

**Unchanged from current behavior** — a user can start a new research session or continue an existing one. Each research session is a first-class server object with a `session_id`, a `ward_id` (inferred by the agent), a status (running / complete / crashed / paused), and associated artifacts.

### View

- **Header.** Left: `☰` drawer-toggle button. Center: `zbot`. Right: `⊕ New research` button, active-ward chip, user menu. (Ward chip here surfaces the **current research session's ward**, not necessarily the app-shell ward.)
- **Status pill strip.** Directly under the header, same component as Chat. Visible only while the current research session has a running agent.
- **Sessions drawer (`☰`).** Default closed. When opened (click `☰`), slides in from the left as an overlay, 280px wide. Contents:
  - `⊕ New research` button (mirrors the header button for convenience).
  - Grouped sessions list: **Running** (at top, dot status-colored), **Today**, **Yesterday**, **Last week**, **Older**.
  - Each row: session title (from `SessionTitleChanged` event or auto-generated from intent), ward chip, status dot (🟢 running / ⚪ complete / 🔴 crashed / 🟡 paused), relative time.
  - Clicking a row jumps to that session (`/research-v2/:session_id`), closes the drawer.
  - Clicking the currently-active row closes the drawer without navigation.
- **Main content column.** Full width when the drawer is closed (minus normal page padding). Max-width ~880px, horizontally centered.
  - **Session header.** Session title + ward chip + status dot. Clicking the title edits it (debounced save).
  - **User query bubble.** The verbatim user prompt that started this session. Styled distinctly from agent output.
  - **Per-agent-turn blocks.** One block per agent turn — see "Per-agent-turn block" below.
  - **Follow-up composer.** Anchored at the bottom. For continuations and follow-up queries within the same session.
- **Artifact slide-out.** Same pattern as Chat. Any file link in the conversation (plan.md, step_*.md, report.html, etc.) opens the slide-out. Files are fetched via existing artifact endpoints.

### Per-agent-turn block

The heart of the Research-page rendering change. Fixes the ping-pong bug.

Between each `AgentStarted` event and the corresponding `AgentCompleted` event, all intermediate events collapse into ONE block:

```
┌ <agent-id> · <wall-clock duration> ────────────────────────┐
│ ▸ Thinking (N actions · M tokens)    [click to expand]    │
│                                                            │
│ <final Respond message rendered as regular markdown>       │
└────────────────────────────────────────────────────────────┘
```

- **Thinking chevron.** Collapsed by default. When expanded: chronological list of every `Thinking` chunk, `ToolCall` (with arg preview), `ToolResult` (with result preview or "offloaded — Nk tokens"), and any `Error` events. Rendered as a vertical timeline with timestamps, not a ping-pong chat.
- **Final Respond.** Whatever the agent returned via `respond()` — the user-facing output for this turn. Always visible, markdown-rendered.
- **Block-level metadata line.** Shows agent-id (color-coded: green=planner, purple=solution, orange=builder, cyan=writer), total duration, token count, and status icon.
- **Running block.** If the agent is still active, show a muted "· running" badge in the header and a pulse indicator. Thinking chevron still expandable; content streams in.
- **Delegation blocks.** When one agent delegates to another, the child agent's turn block appears INDENTED under the parent's thinking chevron — preserving the tree structure. Expanding the parent's thinking reveals the child's block in place.

### Bug fixes rolled into this design

- **Ward goes "unknown" after intent analysis → root.** The Research page maintains a client-side `ward_id` that is only overwritten by an explicit `WardChanged` event. `AgentStarted` events that omit ward are ignored for ward-chip purposes — the last-known-good persists. Bug closed at the UI layer.
- **Thinking + system messages ping-pong.** Eliminated by the per-agent-turn block — all events between `AgentStarted` and `AgentCompleted` live inside one collapsed "Thinking" chevron.
- **New-session behavior buggy.** The new-research flow is a single API call to `POST /api/sessions?mode=research` that returns a `session_id`; the UI navigates to `/research-v2/:session_id` immediately. Event subscription for that session begins on navigation. No race conditions with the old event stream from a previous session.
- **Response not rendered when session ends.** The `Respond` event is the authoritative source for the final-reply bubble. The UI subscribes to it regardless of whether `AgentCompleted` has also arrived. If the socket drops between `Respond` and `AgentCompleted`, the snapshot API still carries the Respond in session state — reconstructed on reconnect.

### Out of scope

- Parallel session tabs (sessions are serial per-user view for v1).
- Branch-from-past-session "new session referencing old" — simple "continue" stays as the existing behavior.
- Read-only-vs-continue distinction on past sessions (all sessions are re-enterable in whatever state they're in).

---

## Shared — Status pill

### Purpose

A persistent glance-able "what is happening right now" surface. Reassures the user that the agent chain is alive and moving, without them having to expand thinking blocks.

### Placement

Full-width slim strip directly under the app header, above the page content. Same component rendered on Chat and Research. On pages where no session is active (e.g., settings, memory), the strip is absent.

### Content

Single-line, hybrid source:

- **Primary:** the latest assistant narration delta from the currently-active agent. Sourced from `Thinking` event `content` field, truncated to ~80 chars with an ellipsis. Preserved across multiple `Thinking` events within the same turn (collected, collapsed to the latest meaningful sentence).
- **Suffix (muted):** the current `ToolCall`'s derived label from the dictionary — e.g. `· yf_fundamentals.py`, `· builder-agent`, `· recall`. Dictionary:

| Tool | Derived suffix |
|---|---|
| `write_file(path=X)` | `· <basename(X)>` |
| `edit_file(path=X)` | `· editing <basename(X)>` |
| `shell(command=C)` | `· <first 30 chars of C>` |
| `load_skill(skill=S)` | `· loading <S>` |
| `delegate_to_agent(agent_id=A)` | `· delegating to <A>` |
| `memory(action=recall)` | `· recall` |
| `graph_query(…)` | `· graph search` |
| `ingest(…)` | `· ingest` |
| `ward(action=use, name=N)` | `· entering <N>` |
| `respond(…)` | `· responding` |

If no narration is available (agent didn't emit a `Thinking` event before the tool call), the pill falls back to the dictionary phrase alone (e.g., `Creating yf_fundamentals.py` from the dictionary).

### Color coding

Left-edge indicator + background tint, keyed to the current `ToolCall`'s category:

- **Blue** — reads: `memory(recall)`, `graph_query`, `grep`, `shell(cat|ls|…)`
- **Cyan** — writes / edits: `write_file`, `edit_file`, `ingest`, `ward(create)`
- **Purple** — delegation: `delegate_to_agent`
- **Green** — responding: `respond`
- **Neutral gray** — startup / unclassified

### Lifecycle

- **Hidden** when no session is active on the current page.
- **Fade-in (150ms)** when the first `Thinking` / `ToolCall` / `AgentStarted` arrives for the active session.
- **Swap animation (150ms slide-out-left + slide-in-right)** when the pill content changes. Content changes on every new `ToolCall` and on `Thinking` deltas that replace the current narration.
- **Parallel agents:** in Research, if multiple subagents are running in parallel, the pill follows the most recently-emitted event (first-fires-wins per new event). No split-screen.
- **Fade-out (300ms)** when the session's last `AgentCompleted` arrives AND there are no pending delegations (`SessionContinuationReady` received or no delegations in flight).
- **Idle state.** Between a fade-out and the next session start, the strip is hidden (not dimmed). Zero visual noise when nothing is happening.

### Interaction

Clicking the pill opens a small dropdown showing the last 10 events in chronological order — raw form (timestamp, agent-id, event-type, content preview). Useful for a "what was the agent doing just now?" glance without expanding a full Thinking block. Click outside to dismiss. **Optional for v1** — can ship as read-only if interaction adds complexity.

---

## Tab-close + resume mechanics

**Goal:** user closes tab mid-session; backend keeps running; user reopens tab; UI reconstructs and rejoins the live stream. Keep simple — inherit current backend behavior, don't introduce new resume primitives.

### On page mount

1. UI reads the URL's `session_id` (or queries "which session is the active quick-chat?" for `/chat-v2`).
2. `GET /api/sessions/:id/state` — returns a snapshot: session metadata (ward, mode, status, title), full message history, last-known agent activity, artifact manifest.
3. UI reconstructs the page from the snapshot: sessions drawer (Research), conversation column, status pill (hidden if session complete, visible if running with last-known narration).
4. UI opens a WebSocket subscription to the session's event stream.

### During session

- WebSocket pushes events in real time; UI appends.
- If WebSocket drops mid-session, the UI displays a subtle "Reconnecting…" indicator (like a small pulse on the status pill). Backend keeps running (current runtime.resume / continuation semantics already support this — nothing new to build).
- On reconnect, the UI re-fetches the snapshot, diffs against local state, and resubscribes. Events that streamed during the disconnect are recovered from the snapshot's message history.

### On session end

- `Respond` event arrives → UI appends the final reply bubble.
- `AgentCompleted` arrives for the last-active agent → status pill fades out.
- `SessionContinuationReady` (if applicable) keeps the session open for follow-up.
- Session's sidebar badge flips from 🟢 running to ⚪ complete.

### Session status surfacing

In the Research sessions drawer, running sessions are sorted to the top with a 🟢 dot. Clicking reconnects. This makes the "oh my research is still going" state explicitly visible — user can resume without hunting.

---

## Event → UI mapping

The canonical mapping from `GatewayEvent` to UI element. Every event has exactly one home.

| GatewayEvent | Chat page | Research page | Status pill |
|---|---|---|---|
| `AgentStarted` | Opens a new thinking-block accumulator (invisible v1) | Opens a new per-agent-turn block | Fades in if first of session |
| `AgentCompleted` | Closes the thinking accumulator; renders inline chips | Closes the turn block; status flips from "running" | Fades out if last agent |
| `AgentStopped` | Same as AgentCompleted but with a stopped badge | Same + stopped badge | Fades out |
| `Thinking` | Feeds inline activity chip (recall/skill detail) | Appends to current turn's Thinking content | **Updates pill primary text** |
| `Token` | Streams into current assistant bubble | Streams into current turn's respond buffer | (ignored) |
| `ToolCall` | Inline chip in assistant reply (purple for delegate) | Appends to current turn's Thinking content | **Updates pill suffix + color** |
| `ToolResult` | (ignored in reply UI; available on chip expand) | Appends to current turn's Thinking content | (ignored) |
| `TurnComplete` | Finalizes bubble for this turn | (mostly informational; turn block uses AgentCompleted) | (ignored) |
| `Respond` | Renders assistant reply bubble | Renders final Respond message below Thinking chevron | Pill color flips green briefly |
| `Error` | Error chip in bubble | Error block inside turn Thinking | Pill color flips red briefly |
| `Heartbeat` | (ignored) | (ignored) | Keeps pill "alive" indicator pulsing |
| `WardChanged` | Updates ward chip (if this is quick-chat's ward) | **Updates ward chip — the only event that changes it** | (ignored) |
| `IterationsExtended` | (ignored) | Muted line in turn Thinking | (ignored) |
| `PlanUpdate` | (ignored — chat doesn't plan) | Renders inline plan.md link | (ignored) |
| `SessionTitleChanged` | (ignored — chat has no title) | Updates session title + drawer entry | (ignored) |
| `DelegationStarted` | Chip in parent reply | Indented child-agent turn block opens inside parent | Pill color flips purple |
| `DelegationCompleted` | Chip expand-data populated | Indented child block closes; parent resumes | (ignored; AgentStarted/Completed handle transitions) |
| `MessageAdded` | New user bubble or system message | Same | (ignored) |
| `TokenUsage` | Footer badge (future) | Session metadata | (ignored) |
| `SessionContinuationReady` | (ignored — quick-chat doesn't continue this way) | Keeps sessions drawer status "running" | Prevents pill from fading immediately |
| `IntentAnalysisStarted` | (ignored — chat skips intent analysis per mode) | Muted "analyzing intent…" line above user bubble | **Updates pill: "Analyzing intent…"** |
| `IntentAnalysisComplete` | (ignored) | Replaces the muted line with ward chip + classification | Pill fades to next event |
| `IntentAnalysisSkipped` | (ignored) | Removes the muted line if it was showing | (ignored) |

---

## Backend gap table

| # | Gap | Location | Effort | Notes |
|---|---|---|---|---|
| G1 | Create `quick-chat` agent | `~/Documents/zbot/agents/quick-chat/{AGENTS.md, config.yaml}` + template in `gateway/templates/agents/` | small | One system prompt + one config file. |
| G2 | Quick-chat tool allowlist | `quick-chat/config.yaml` → `tools: [memory, load_skill, delegate_to_agent, respond, ward, grep, multimodal_analyze, graph_query, ingest]` | small | Excludes `update_plan`, `set_session_title` (root-only) and the planner itself. |
| G3 | Single-delegation soft rule | Prompt-level: "You may delegate to at most one subagent per turn. You may not invoke planner-agent — if the task needs planning, tell the user to switch to Research." | negligible | Prompt-only enforcement for v1. |
| G4 | Session mode tag | Already present — `sessions.mode` column exists (per schema audit). Confirm `quick-chat` and `research` values honored. | verify | Audit the session-creation paths. |
| G5 | Session-creation endpoint | `POST /api/sessions` with `{mode, agent_id, ward_id}` body. Verify exists; add params if missing. | small | Likely exists. |
| G6 | Ward-stickiness bug fix | Ensure `AgentStarted` events carry `ward_id` from session state, not agent context. Or fix at UI layer (U6). Both work; UI-layer is simpler. | tiny | Preferred: UI-layer fix. |
| G7 | Message-history pagination | `GET /api/sessions/:id/messages?before=<cursor>&limit=<n>`. | small | May already exist unpaginated; add params. |
| G8 | Session snapshot includes active-agent narration | `GET /api/sessions/:id/state` returns the last `Thinking` delta + last `ToolCall` for the active agent so the status pill restores on reload. | medium | If snapshot already carries message history, this is an additional top-level field. |
| G9 | Resume semantics — keep current | No change. If `runtime.resume()` is pause-based, user accepts that. If a session is "paused" when the tab closed, clicking it in the drawer calls resume; UI reflects the running state. | none | Inherit current. |
| G10 | Running-session surfacing | Sessions-list API returns status (`running` / `complete` / `crashed` / `paused`). Verify `GET /api/sessions?…` returns status. | verify | Likely already does. |
| G11 | Final-Respond reliability | When `Respond` arrives but `AgentCompleted` doesn't (socket drop), snapshot reload must still surface the Respond in the message history so the UI renders it. Verify. | verify | Tied to the "response not rendered when session ends" bug. |

---

## UI gap table

| # | Gap | Location | Effort |
|---|---|---|---|
| U1 | `StatusPill` component | `apps/ui/src/features/shared/StatusPill.tsx` (new) + its hook. Consumes `Thinking` + `ToolCall` + `Respond` + `AgentCompleted` events. Hybrid narration + suffix + color. | medium |
| U2 | `AgentTurnBlock` component | `apps/ui/src/features/research/AgentTurnBlock.tsx` (new). Groups events per `execution_id` between `AgentStarted` / `AgentCompleted`. Collapsed Thinking chevron + visible Respond. | medium |
| U3 | `QuickChat` page | `apps/ui/src/features/chat/QuickChat.tsx` (new, not FastChat). Centered column, composer, inline chips, lazy-loaded older turns. | medium |
| U4 | `ResearchPage` | `apps/ui/src/features/research/ResearchPage.tsx` (new). `☰` toggle + sessions drawer + main column + artifact slide-out wiring. | medium |
| U5 | `SessionsDrawer` + `SessionsList` | `SessionsList.tsx` — presentation-agnostic. Rendered inside `SessionsDrawer.tsx` for v1; future topbar uses the same `SessionsList`. | small-medium |
| U6 | Ward-sticky state | Both new pages: client-side state that tracks last-known `ward_id` and only updates on explicit `WardChanged`. Fixes bug #1. | tiny |
| U7 | Route additions | `App.tsx` — add `/chat-v2` → `QuickChat`, `/research-v2` → `ResearchPage`. Keep old routes. | tiny |
| U8 | Hooks: `useQuickChat`, `useResearchSession` | New hooks consuming WebSocket events per session. Mirror the structure of `useFastChat` and `useMissionControl` but subscribe to the new event-to-UI mapping above. | medium |
| U9 | Artifact slide-out wiring | Reuse `ArtifactSlideOut.tsx` + `ArtifactsPanel.tsx` components; wire into both new pages. | small |
| U10 | `/chat-v2` session auto-resolve | On mount, if no `session_id` in URL, call `GET /api/sessions/active?mode=quick-chat` (new or extended endpoint — pin in plan) to find the current quick-chat session; if none, `POST /api/sessions {mode: "quick-chat"}` to create. Navigate to the resolved id. | small |
| U11 | "New chat" + "New research" flows | Button → API call → redirect to new session URL. Old session archived server-side. | small |
| U12 | WebSocket reconnect UX | Subtle "reconnecting…" indicator on the status pill area when WS disconnects. On reconnect: re-fetch snapshot, resubscribe, resume rendering. | small |

---

## Migration strategy — new alongside old

Phase 1 (this PR):
- Add `/chat-v2` and `/research-v2` routes wired to new components.
- Old `/chat` (FastChat) and `/` (WebChatPanel) remain untouched and functional.
- Both entry points coexist in nav (or `/chat-v2` gated behind a settings toggle if the user prefers no surface-level exposure yet).

Phase 2 (this PR or follow-up):
- User runs both. Reports bugs against new pages.
- Fixes land in `feature/ui-chat-research-redesign` iteratively.

Phase 3 (separate cleanup PR, once new pages are trusted):
- Swap routes: `/chat` → `QuickChat`, `/` (or `/research`) → `ResearchPage`.
- Delete old files: `FastChat.tsx`, `MissionControl.tsx`, `WebChatPanel.tsx`, `fast-chat-hooks.ts`, `mission-hooks.ts`, and any components uniquely used by them.
- Update memory-bank docs.

No database migration needed — session records carry `mode` today; old and new pages coexist on the same data.

---

## Future evolution — topbar migration

Designed into the components so the topbar swap is additive, not a rewrite:

- **`SessionsList` is presentation-agnostic.** Its props: `sessions`, `currentId`, `onSelect`, `onNew`, `renderDensity` ("expanded" / "condensed"). It renders a list; whoever mounts it decides the container.
- **`SessionsDrawer`** for v1 is a thin wrapper that slides `<SessionsList renderDensity="expanded" />` from the left on `☰` click.
- **Future `SessionsTopbarDropdown`** will be a thin wrapper that mounts `<SessionsList renderDensity="condensed" />` in a header dropdown.
- **Ward chip and "New research" button** are already header-native; the topbar evolution just reorganizes what sits next to them.

The tangible rule: when the v1 ships, nothing in `SessionsList.tsx` should assume "I am rendered in a drawer." All drawer-specific behavior (slide animation, overlay, backdrop) lives in the `SessionsDrawer` wrapper.

---

## Risks and tradeoffs

- **Ephemeral Chat with no history panel.** If a user wants to look back at a past chat, they'll need to go to Logs. Accepted per the Q3 decision. Mitigation: consider a follow-up PR that adds a minimal "recent chats" popover if users complain.
- **Soft single-delegation enforcement.** Prompt-based. A misbehaving quick-chat agent could still delegate more than once. Mitigation: monitor; escalate to hard enforcement if abuse shows up in sessions.
- **Status pill can flicker on rapid tool-call churn.** Dictionary of animations is 150ms in + 150ms out; if tool calls fire faster than 300ms/each, pill visually "strobes." Mitigation: debounce the pill swap — if a new event lands during an animation, queue to the end; only the most recent queued event actually renders. Implement in U1.
- **Per-agent-turn blocks can grow tall when thinking is verbose.** By default collapsed, so not an issue until a user expands. Mitigation: inside the expanded Thinking, virtualize the timeline (only render what's visible) if perf becomes an issue.
- **WebSocket reconnect during a critical `Respond`.** If the socket drops between `Respond` event emission and the client receiving it, the snapshot-reload path must still show the Respond. Covered by G11; must verify during implementation.

---

## Acceptance criteria

Consider this design implemented when:

1. Navigating to `/chat-v2` opens a Claude-minimal chat page with a composer, ward chip, and "New chat" button. Entering a memory-based question returns a response using the quick-chat agent and shows inline recall/skill/delegation chips.
2. Clicking "New chat" discards the current chat session (server-side archived) and opens a fresh one. Prior chat is no longer visible.
3. Closing the tab and reopening `/chat-v2` restores the same ongoing session (if not "New chat"-ed) with the conversation intact.
4. Navigating to `/research-v2` shows a centered conversation column with `☰` drawer toggle. Starting a new research triggers the full agent chain; each agent's turn appears as a collapsed Thinking block with its Respond message visible below.
5. The rolling status pill appears on top of both pages while an agent is active, updates as new `Thinking` / `ToolCall` events arrive, and fades when the session is complete.
6. Closing the Research tab mid-session and reopening it shows the session still running (per running-dot in drawer), the live status pill resumes, and the Thinking blocks continue to stream.
7. The ward chip in Research displays the session's ward correctly throughout — never flipping to "unknown" on agent transitions.
8. A per-agent-turn block's Thinking chevron expands to show a chronological timeline of all inner events — no ping-pong between thinking and system messages.
9. The `Respond` event's content is rendered as the final-reply bubble even when `AgentCompleted` arrives late or is lost.
10. Old `/chat` (FastChat) and `/` (WebChatPanel) still work unchanged.
