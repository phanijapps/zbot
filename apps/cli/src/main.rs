//! `zbot` — lightweight streaming Claude-Code-style CLI for the z-Bot daemon.
//!
//! Architecture: this CLI is a thin front-end. All real work happens in the
//! daemon. The CLI opens an HTTP + WebSocket connection and renders the
//! event stream via `iocraft` components.
//!
//! Modes
//! -----
//! - `zbot`                                — interactive REPL
//! - `zbot "do X"`                         — one-shot, exits on completion
//! - `zbot --session <id>`                 — resume a specific session
//! - `cat file.md | zbot "summarise"`      — read stdin if not a TTY
//! - `zbot --url http://desktop:18791`     — connect to a remote daemon
//!
//! Configuration precedence: `--url` > `ZBOT_URL` > `~/.config/zbot/cli.toml` > default.

mod client;
mod config;
mod events;

use anyhow::{Context, Result};
use clap::Parser;
use std::io::IsTerminal;

use crate::client::DaemonClient;
use crate::config::Config;
use crate::events::EventStream;
use gateway_ws_protocol::ServerMessage;

#[derive(Parser, Debug)]
#[command(
    name = "zbot",
    version,
    about = "Streaming chat client for the z-Bot daemon",
    long_about = None,
)]
struct Args {
    /// Daemon base URL (overrides ZBOT_URL and config file).
    #[arg(long, value_name = "URL")]
    url: Option<String>,

    /// Resume a specific session by id.
    #[arg(long, value_name = "ID")]
    session: Option<String>,

    /// One-shot prompt. When provided, sends and exits on completion.
    /// If stdin is not a TTY, its contents are prepended to this message.
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let args = Args::parse();
    let cfg = Config::resolve(args.url).context("resolve daemon URL")?;
    let client = DaemonClient::new(cfg.clone());

    // Phase 1 smoke test: hit /api/health and report.
    let health = client
        .health()
        .await
        .with_context(|| format!("daemon unreachable at {}", cfg.daemon_url))?;
    eprintln!(
        "zbot · daemon {} · status={} · version={}",
        cfg.daemon_url, health.status, health.version
    );

    // Determine mode (stub for Phase 2+).
    let stdin_piped = !std::io::stdin().is_terminal();
    let mode = match (args.prompt.as_deref(), stdin_piped) {
        (Some(_), _) | (_, true) => Mode::OneShot,
        _ => Mode::Interactive,
    };
    eprintln!("zbot · mode={:?} · session={:?}", mode, args.session);

    // Phase 2 transport smoke test:
    //   1. POST /api/chat/init → get sessionId + conversationId
    //   2. Open WS to /ws
    //   3. Subscribe to the conversation
    //   4. Wait for the `Subscribed` ack and report it
    //   5. Disconnect
    //
    // Phase 3+ will turn this into a full chat REPL with iocraft components.
    let chat = client
        .init_chat_session()
        .await
        .context("init chat session")?;
    eprintln!(
        "zbot · chat session={} conv={} created={}",
        chat.session_id, chat.conversation_id, chat.created
    );

    let mut events = EventStream::connect(&cfg.websocket_url())
        .await
        .with_context(|| format!("ws connect to {}", cfg.websocket_url()))?;
    events.subscribe(&chat.conversation_id)?;

    // Drain events until we see the Subscribed ack (or 5s elapses).
    // The daemon may emit `Connected` on initial handshake before the ack.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            eprintln!("zbot · timeout waiting for Subscribed ack");
            break;
        }
        match tokio::time::timeout(remaining, events.recv()).await {
            Ok(Some(ServerMessage::Subscribed { conversation_id, current_sequence, .. })) => {
                eprintln!(
                    "zbot · ws subscribed conv={} seq={}",
                    conversation_id, current_sequence
                );
                break;
            }
            Ok(Some(other)) => {
                tracing::debug!(event = ?other, "waiting for Subscribed; got intermediate event");
            }
            Ok(None) => {
                eprintln!("zbot · ws closed before ack");
                break;
            }
            Err(_) => {
                eprintln!("zbot · timeout waiting for Subscribed ack");
                break;
            }
        }
    }

    // Phase 2 ends here. Phase 3 wires this transport into the iocraft UI.
    Ok(())
}

#[derive(Debug)]
enum Mode {
    Interactive,
    OneShot,
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_env("ZBOT_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    fmt().with_env_filter(filter).with_writer(std::io::stderr).init();
}
