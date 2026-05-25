//! One-shot mode — non-interactive streaming.
//!
//! `zbot "do X"` or `cat file | zbot "summarise"`. Connects to the
//! daemon, sends one message, streams tokens to stdout as they arrive,
//! exits when the turn completes. No iocraft, no full-screen render,
//! no terminal raw mode. Plays nice with pipes, redirects, scripts.
//!
//! Exit codes:
//! - 0 — turn completed successfully
//! - 1 — daemon returned an error or the WS dropped before the turn ended

use anyhow::{Context, Result};
use gateway_ws_protocol::ServerMessage;
use std::io::{self, IsTerminal, Read, Write};
use std::time::Duration;

use crate::client::ChatInit;
use crate::events::EventStream;

/// Compose the user message from CLI prompt + optional stdin.
///
/// - `zbot "msg"`           → returns `"msg"`
/// - `cat f | zbot`         → returns stdin
/// - `cat f | zbot "msg"`   → returns `"<stdin>\n\n<msg>"`
pub fn compose_message(prompt: Option<String>) -> Result<String> {
    let piped = !io::stdin().is_terminal();
    let stdin_text = if piped {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("read stdin")?;
        Some(buf.trim_end().to_string()).filter(|s| !s.is_empty())
    } else {
        None
    };
    Ok(match (stdin_text, prompt) {
        (Some(s), Some(p)) => format!("{s}\n\n{p}"),
        (Some(s), None) => s,
        (None, Some(p)) => p,
        (None, None) => String::new(),
    })
}

/// Run one turn in non-interactive mode. Streams tokens to stdout,
/// tool calls to stderr, returns when the turn completes or errors.
pub async fn run_oneshot(
    chat: ChatInit,
    mut events: EventStream,
    message: String,
    color: bool,
) -> Result<()> {
    if message.trim().is_empty() {
        anyhow::bail!("no message — provide a prompt or pipe content via stdin");
    }

    events.subscribe(&chat.conversation_id)?;

    // Drain intermediate events until Subscribed, then send Invoke.
    let subscribe_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = subscribe_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("timeout waiting for ws Subscribed ack");
        }
        match tokio::time::timeout(remaining, events.recv()).await {
            Ok(Some(ServerMessage::Subscribed { .. })) => break,
            Ok(Some(_)) => continue, // ignore Connected etc.
            Ok(None) => anyhow::bail!("ws closed before Subscribed"),
            Err(_) => anyhow::bail!("timeout waiting for ws Subscribed ack"),
        }
    }

    events.invoke(
        "root",
        &chat.conversation_id,
        Some(chat.session_id.clone()),
        message,
    )?;

    let mut had_token = false;
    let mut exit_err: Option<String> = None;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    loop {
        let Some(msg) = events.recv().await else {
            if exit_err.is_none() {
                exit_err = Some("ws closed before turn completed".into());
            }
            break;
        };
        match msg {
            ServerMessage::Token { delta, .. } => {
                had_token = true;
                let _ = out.write_all(delta.as_bytes());
                let _ = out.flush();
            }
            ServerMessage::ToolCall { tool, .. } => {
                let line = if color {
                    format!("\x1b[2m  ▶ {tool}\x1b[0m\n")
                } else {
                    format!("  ▶ {tool}\n")
                };
                let _ = io::stderr().write_all(line.as_bytes());
            }
            ServerMessage::ToolResult { tool_call_id: _, error, .. } => {
                if let Some(err) = error {
                    let line = if color {
                        format!("\x1b[31m  ✗ {err}\x1b[0m\n")
                    } else {
                        format!("  ✗ {err}\n")
                    };
                    let _ = io::stderr().write_all(line.as_bytes());
                }
            }
            ServerMessage::TurnComplete { .. } | ServerMessage::AgentCompleted { .. } => {
                break;
            }
            ServerMessage::Error { message, .. } => {
                exit_err = Some(message);
                break;
            }
            _ => { /* ignore Thinking, Heartbeat, Iteration, TokenUsage, etc. */ }
        }
    }

    // Make sure we end on a newline if any output came through.
    if had_token {
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    }

    if let Some(err) = exit_err {
        anyhow::bail!("{err}");
    }
    Ok(())
}
