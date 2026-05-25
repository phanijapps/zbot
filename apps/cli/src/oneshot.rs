//! One-shot mode — non-interactive streaming.
//!
//! `zbot "do X"` or `cat file | zbot "summarise"`. Sends one message,
//! streams tokens to stdout, exits when the turn completes. No iocraft,
//! no readline, no terminal raw mode.

use anyhow::{Context, Result};
use gateway_ws_protocol::ServerMessage;
use std::io::{self, IsTerminal, Read};
use std::time::Duration;

use crate::client::ChatInit;
use crate::events::EventStream;
use crate::stream::{run_turn, StreamConfig};

/// Compose the user message from CLI prompt + optional stdin.
pub fn compose_message(prompt: Option<String>) -> Result<String> {
    let piped = !io::stdin().is_terminal();
    let stdin_text = if piped {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).context("read stdin")?;
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

    // Drain intermediate events until Subscribed.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("timeout waiting for ws Subscribed ack");
        }
        match tokio::time::timeout(remaining, events.recv()).await {
            Ok(Some(ServerMessage::Subscribed { .. })) => break,
            Ok(Some(_)) => continue,
            Ok(None) => anyhow::bail!("ws closed before Subscribed"),
            Err(_) => anyhow::bail!("timeout waiting for ws Subscribed ack"),
        }
    }

    let cfg = StreamConfig { color, indent: 0 };
    let _summary = run_turn(
        &mut events,
        &chat.conversation_id,
        &chat.session_id,
        &message,
        cfg,
    )
    .await
    .context("one-shot turn")?;

    Ok(())
}
