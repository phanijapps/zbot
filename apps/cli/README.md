# `zbot` — lightweight CLI for the z-Bot daemon

A streaming, Claude-Code-style terminal client. Talks to a running `zbotd` over
HTTP + WebSocket. ~1100 LOC, ~9 MB stripped release binary.

## Install

```bash
cargo build -p cli --release
# binary at target/release/zbot
```

## Daily usage

```bash
# interactive REPL (when stdin + stdout are a TTY)
zbot

# one-shot — sends, streams, exits
zbot "summarise yesterday's deploys"

# pipe stdin in; combine with optional prompt
cat README.md | zbot "what does this project do?"
cat error.log  | zbot

# point at a remote daemon (e.g. desktop from your Pi)
zbot --url http://desktop:18791 "list the open PRs"

# script-friendly — output redirects cleanly, no terminal codes
zbot "write a haiku about rust" > haiku.txt
```

## Modes

`zbot` picks a mode automatically:

| Signal | Mode |
|--|--|
| Prompt argument provided          | **one-shot** |
| Stdin is piped (not a TTY)        | **one-shot** |
| Stdout is redirected (not a TTY)  | **one-shot** |
| Otherwise                         | **interactive** |

One-shot mode streams the assistant's response to stdout, tool-call markers to stderr, and exits when the turn completes. Exit code is `0` on success, `1` on error.

## Slash commands (interactive mode only)

| Command | Action |
|--|--|
| `/help` (or `/?`, `/h`)      | Show command help |
| `/new`                       | Clear the chat session and start fresh |
| `/sessions` (or `/ls`)       | List recent conversations |
| `/wards`                     | List wards |
| `/memory <q>` (or `/m`, `/recall`) | Quick recall — no chat turn cost |
| `/quit` (or `/q`, `/exit`)   | Exit (Ctrl+C / Ctrl+D also work) |

In interactive mode, any input starting with `/` is treated as a slash command. Anything else is sent as a chat message.

## Configuration

Daemon URL resolution (highest precedence first):

1. `--url <URL>` flag
2. `ZBOT_URL` environment variable
3. `~/.config/zbot/cli.toml` (`daemon_url = "..."`)
4. Default: `http://localhost:18791`

Other:

- `--no-color` or `NO_COLOR=1` — disable ANSI colors (also auto-off when stdout isn't a TTY)
- `ZBOT_LOG` — tracing filter (`ZBOT_LOG=debug`, etc.); defaults to `warn`

Example `~/.config/zbot/cli.toml`:

```toml
daemon_url = "http://localhost:18791"
```

## Tool-call visualization

While the assistant is working, you'll see compact tool-call markers in the interactive view:

```
  ▶ shell · running · args={"command": "git status"}
  ✓ shell · On branch main…
```

In one-shot mode the same markers go to stderr (so they don't pollute stdout when you're piping the response).

## What this CLI is *not*

- A full-screen TUI. Use a normal terminal session; output behaves like any other streaming command.
- A standalone agent. All reasoning, tools, and memory live in the daemon. The CLI is a thin viewer + dispatcher.
- A permission gate. Tool safety lives daemon-side (forbidden commands, sudo guard, validate_command). The CLI just shows what the daemon decided.

## Architecture notes

| Layer | Crate / module |
|--|--|
| Component tree | `iocraft` |
| Transport      | `reqwest` + `tokio-tungstenite` |
| Protocol types | `gateway-ws-protocol` (shared with daemon — guaranteed in lock-step) |
| Slash parser   | `apps/cli/src/slash.rs` |
| Interactive REPL | `apps/cli/src/ui.rs` |
| One-shot mode  | `apps/cli/src/oneshot.rs` |
| URL discovery  | `apps/cli/src/config.rs` |

Future work (not yet shipped):

- Markdown rendering for assistant responses (pulldown-cmark is in deps, unused)
- File-diff visualization for `edit_file` tool calls
- `/resume <id>` and `/ward <name>` (need daemon-side support for clean switches)
- Multi-line input (Shift+Enter for newline)
