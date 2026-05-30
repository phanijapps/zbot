# `zbot` CLI Rewrite — Implementation Plan

**Date:** 2026-05-24
**Scope:** Replace the existing 1751-LOC ratatui TUI in `apps/cli/` with a streaming, component-driven Claude-Code-style CLI built on `iocraft`. Same binary name (`zbot`), same crate path (`apps/cli/`), fresh source.

**Out of scope:** SSH backends, Dockerize work, embedded mode, MCP config from CLI, file diff display, background/detach mode, hooks.

---

## Goal

A daily-driver terminal experience that feels like Claude Code:

- Streaming chat with the daemon's root agent
- Slash commands for session / ward / memory operations
- Permission prompts for non-read tool calls
- Component-driven UI that re-renders reactively on event stream
- Lightweight (~1000-1500 LOC), single static binary, no full-screen TUI takeover

## Architecture

- **Front-end framework:** `iocraft` (React/JSX-style components, Rust analogue of the `ink` library that Claude Code itself uses).
- **Transport:** HTTP (`reqwest`) + WebSocket (`tokio-tungstenite`) to the existing daemon at `http://localhost:18791` by default.
- **State model:** iocraft hooks (`use_state`, `use_future`, `use_context`) — no manual redraw loop.
- **All work happens in the daemon.** The CLI is a streaming-I/O shell with reactive rendering on top of the WebSocket event stream.

### Why `iocraft` and not the alternatives

| Considered | Verdict |
|--|--|
| `crossterm` only | Too bare — we'd hand-roll the component layer anyway |
| `ratatui` (immediate-mode) | Immediate-mode means writing the redraw loop ourselves; component-driven fits chat UIs better |
| `tui-realm` (Elm on ratatui) | Elm-style verbose for chat; the message-list-of-components mental model fits React better |
| `cursive` | Dated ergonomics, community moved on |
| **`iocraft`** | **Chosen** — component-driven, JSX-like, deliberate analogue to `ink`, reactive state model fits chat naturally |

### Known risks with iocraft

- Newer ecosystem (~1-2 years of life). Fewer Stack Overflow answers than ratatui.
- Possible API churn between minor versions. Pin to a specific minor.
- Smaller prior-art catalogue — may need to read source for edge cases.

Mitigation: keep components small and replaceable. If iocraft proves painful, the render layer is the only throwaway — the slash-command / transport / config layers carry over to any UI framework.

---

## File structure

```
apps/cli/
├── Cargo.toml                  (rewrite: drop ratatui, add iocraft)
├── README.md                   (new: usage + slash commands table)
└── src/
    ├── main.rs                 (clap entry + mode dispatch)
    ├── client.rs               (HTTP + WS transport to daemon)
    ├── config.rs               (daemon URL discovery, auth resolution)
    ├── slash.rs                (slash command parser + dispatcher)
    ├── events.rs               (WS event stream → typed state updates)
    ├── permission.rs           (tool-call permission policy + smart defaults)
    └── components/
        ├── mod.rs
        ├── app.rs              (<App> — root, holds session state)
        ├── header.rs           (<Header> — daemon URL, active ward, session id)
        ├── message_list.rs     (<MessageList> — scrolling history)
        ├── message.rs          (<UserMessage>, <AssistantMessage> — markdown content)
        ├── tool_call.rs        (<ToolCallCard> — one-line + collapsible output)
        ├── permission_dialog.rs (<PermissionDialog> — y/n/a/s prompt)
        └── prompt.rs           (<PromptInput> — text input + slash hints)
```

**Total target:** ~1000-1500 LOC. The existing 1751-LOC ratatui code is archived (moved to a `_archive/` directory or simply replaced and recovered from git if needed).

---

## Daemon endpoints to consume

**Task 0** before any code: verify these exist with the shapes assumed below. Adjust plan if any are missing or differently shaped.

| Method | Path | Use |
|--|--|--|
| GET | `/api/health` | Smoke test on startup |
| POST | `/api/chat/init` | Reserve a session id |
| POST | `/api/chat/send` | Send a user message |
| WS | `/ws` | Stream events: `token`, `tool_call`, `tool_call_pending`, `tool_result`, `response_complete`, `session_end` |
| GET | `/api/sessions` | List sessions for `/sessions` picker |
| GET | `/api/sessions/:id/state` | Snapshot for `/resume` |
| GET | `/api/wards` | List for `/wards` |
| POST | `/api/wards/active` | Switch for `/ward <name>` |
| POST | `/api/memory/recall` | `/memory <q>` quick query without chat turn |

The verification step is a 20-minute grep through `gateway/src/http/` against this list.

---

## Modes

- `zbot` — interactive REPL, streaming chat
- `zbot "do X"` — one-shot, exits on `session_end`
- `zbot --session <id>` — resume specific session
- `cat file.md | zbot "summarise"` — read stdin if not a TTY, prepend to message
- `zbot --url http://desktop:18791` — connect to a remote daemon (e.g. from Pi to desktop)

## Daemon URL discovery (precedence)

1. `--url <url>` flag
2. `ZBOT_URL` env var
3. `~/.config/zbot/cli.toml`
4. Default: `http://localhost:18791`

## Slash commands (v1)

| Command | Behavior |
|--|--|
| `/help` (or `/?`) | Show command table |
| `/new` | Clear current session, start fresh |
| `/sessions` | Recent sessions, numbered picker |
| `/resume <id>` | Resume specific session |
| `/wards` | List wards, mark active |
| `/ward <name>` | Switch active ward |
| `/memory <query>` | Quick recall, no chat turn cost |
| `/quit` (or `/q`, Ctrl+D) | Exit |

## Permission prompts

| Tool category | Default |
|--|--|
| Reads (`recall`, `list`, `get_fact`, `grep`, `read_file`) | auto-allow |
| Writes (`save_fact`, `write_file`, `edit_file`) | prompt: `y / n / a (allow all this session) / s (skip session)` |
| Execute (`shell`, `run_procedure`) | prompt, same options |

Override: `ZBOT_AUTO_APPROVE=all|reads|none`.

Default-deny after **30 seconds** if user walks away mid-prompt (covers headless / SSH timeout scenarios).

---

## Task breakdown — ~16-20 hours total

### Phase 0 — Verify endpoints (20 min)

- Grep `gateway/src/http/mod.rs` and `gateway/src/http/*.rs` against the 9 endpoints listed above
- Confirm WebSocket event types match assumptions in `gateway/src/ws/` or `gateway-ws-protocol/`
- Adjust this plan inline if any endpoint shape differs

### Phase 1 — Scaffold (~3 hours)

- Move existing `apps/cli/src/*.rs` to `apps/cli/_archive/` (or rely on git history; pick one)
- Rewrite `apps/cli/Cargo.toml`:
  - Drop: `ratatui`
  - Add: `iocraft`, `pulldown-cmark` (for markdown), `inquire` (only if iocraft's input doesn't cover us)
  - Keep: `clap`, `tokio`, `reqwest`, `tokio-tungstenite`, `serde`, `serde_json`, `crossterm` (iocraft uses it under the hood), `chrono`, `uuid`, `anyhow`, `tracing`, `tracing-subscriber`
- Create new `src/main.rs` with clap args (`--url`, `--session`, optional prompt arg)
- Create `src/config.rs` with URL discovery precedence
- Create `src/client.rs` with HTTP client + `/api/health` smoke test on startup
- Verify build: `cargo build -p cli`

### Phase 2 — Transport + event stream (~4 hours)

- `src/client.rs`: wrap HTTP for `chat/init`, `chat/send`
- `src/events.rs`: WebSocket client + typed event enum (`Token`, `ToolCall`, `ToolCallPending`, `ToolResult`, `ResponseComplete`, `SessionEnd`)
- mpsc channel from WS event loop to UI thread
- Iocraft `use_future` hook in `<App>` to consume the channel and update state

### Phase 3 — Component tree (~5 hours)

- `<App>` — root; holds session state, message history, active tool call, permission dialog state
- `<Header>` — top status line: daemon URL · active ward · session id (short)
- `<MessageList>` — scrolling history of `<UserMessage>` and `<AssistantMessage>` components
- `<UserMessage>` — right-aligned (or distinguished) prefix + content
- `<AssistantMessage>` — streaming content, minimal markdown via `pulldown-cmark` rendered to terminal SGR
- `<ToolCallCard>` — compact: `▶ shell · git status`, expandable to show result (collapse after N lines)
- `<PromptInput>` — text field at bottom with slash command hint dropdown
- `<PermissionDialog>` — modal-ish overlay for tool-call confirmation

### Phase 4 — Slash commands (~3 hours)

- `src/slash.rs`: parser + dispatcher
- Wire `/help`, `/new`, `/sessions`, `/resume`, `/wards`, `/ward`, `/memory`, `/quit`
- Picker UI for `/sessions` and `/wards` (iocraft list component)

### Phase 5 — Permission prompts (~2 hours)

- `src/permission.rs`: policy table (read/write/execute defaults)
- `<PermissionDialog>` component with y/n/a/s keybindings
- `ZBOT_AUTO_APPROVE` env var override
- 30-second default-deny timer

### Phase 6 — Polish (~2 hours)

- `$NO_COLOR` support (disable ANSI)
- TTY detection (skip iocraft, use plain stream output when piped)
- Error messages on daemon down (clear, with the URL we tried)
- One-shot mode: connect → send → stream → exit on `session_end`
- Stdin pipe handling: read until EOF if `!stdin.is_tty()`, prepend to message
- `README.md` with usage + slash command table
- Manual smoke test against running daemon

---

## Open decisions (resolve in PR review, not blockers)

1. **Markdown library** — `pulldown-cmark` (mature, 0 deps beyond unicode) vs `termimad` (heavier, prettier defaults). Lean `pulldown-cmark` + minimal hand-rolled SGR.
2. **Picker UX for `/sessions`** — iocraft list or `inquire::Select`? Lean iocraft for consistency with the rest of the UI.
3. **Tool call output collapsing** — v1: show all. v2: collapse beyond N lines (default 20) with `[...N more lines, hit space]`.
4. **Session resume picker shows what?** — id (short hash) + first user message + timestamp + token count?
5. **Multi-line input** — Shift+Enter for newline, Enter to send? Match Claude Code's behavior.

## What I'd do before writing the PR

1. **Phase 0 endpoint verification** (~20 min) — single sweep through `gateway/src/http/`
2. **Skim 2-3 iocraft examples** (~20 min) — get the JSX-macro syntax in muscle memory before starting
3. **Sketch the component tree on paper** (~15 min) — confirms data flow before code

## Success criteria

- `cargo build -p cli` clean
- `zbot` opens REPL, streams a response
- `zbot "what time is it"` runs one-shot, prints result, exits
- `cat README.md | zbot "summarise"` works
- `/sessions` shows a picker, selecting one resumes that session
- `/memory rust` returns recall results inline without consuming a chat turn
- Permission prompt fires on `shell`, allows `recall` silently
- `Ctrl+D` exits cleanly
- Binary is < 30 MB stripped (sanity check; not a hard constraint)

---

## Sequence to PR

1. PR #1 (this rewrite) — entire CLI in one PR. Don't split: the components are co-designed.
2. Manual smoke test on dev box + Pi (just `scp` the ARM build)
3. Merge → start using as daily driver → file follow-up issues for v2 wishes (file diff display, collapsing, etc.)
