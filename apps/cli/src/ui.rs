//! Interactive chat REPL — iocraft component tree.
//!
//! Phase 3 scope:
//! - <App>: streaming chat with user + assistant messages
//! - Text input at the bottom; Enter sends, Ctrl+C / Ctrl+D quits
//! - Background WS bridge maps `ServerMessage` to UI state updates
//!
//! Out of scope (later phases): slash commands, tool-call viz,
//! permission prompts, markdown rendering.

use anyhow::{Context, Result};
use gateway_ws_protocol::ServerMessage;
use iocraft::prelude::*;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::client::ChatInit;
use crate::events::EventStream;

// =========================================================================
// Channel payloads
// =========================================================================

/// Updates pushed from the WS bridge into the iocraft App state.
#[derive(Debug, Clone)]
enum UiUpdate {
    Token(String),
    TurnComplete,
    Error(String),
}

/// User-initiated actions pushed from the App component into the WS bridge.
#[derive(Debug)]
enum UserAction {
    SendMessage(String),
    Quit,
}

// =========================================================================
// Domain model — what gets rendered
// =========================================================================

#[derive(Clone, Debug)]
enum MessageKind {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug)]
struct ChatMsg {
    kind: MessageKind,
    content: String,
}

// =========================================================================
// <App> component
// =========================================================================

#[derive(Default, Props)]
struct AppProps {
    daemon_url: String,
    session_id: String,
    /// Drained once on first render.
    update_rx: Option<UnboundedReceiver<UiUpdate>>,
    /// Cloned per-callback (mpsc Sender is Clone + Send).
    action_tx: Option<UnboundedSender<UserAction>>,
}

#[component]
fn App(props: &mut AppProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut messages = hooks.use_state(Vec::<ChatMsg>::new);
    let mut active_assistant = hooks.use_state(String::new);
    let mut input = hooks.use_state(String::new);
    let mut streaming = hooks.use_state(|| false);
    let mut should_exit = hooks.use_state(|| false);
    let mut system = hooks.use_context_mut::<SystemContext>();

    // Drain the update channel into reactive state.
    if let Some(mut rx) = props.update_rx.take() {
        hooks.use_future(async move {
            while let Some(update) = rx.recv().await {
                match update {
                    UiUpdate::Token(delta) => {
                        let mut current = active_assistant.to_string();
                        current.push_str(&delta);
                        active_assistant.set(current);
                        streaming.set(true);
                    }
                    UiUpdate::TurnComplete => {
                        let final_text = active_assistant.to_string();
                        if !final_text.is_empty() {
                            let mut msgs = messages.read().clone();
                            msgs.push(ChatMsg {
                                kind: MessageKind::Assistant,
                                content: final_text,
                            });
                            messages.set(msgs);
                            active_assistant.set(String::new());
                        }
                        streaming.set(false);
                    }
                    UiUpdate::Error(msg) => {
                        let mut msgs = messages.read().clone();
                        msgs.push(ChatMsg {
                            kind: MessageKind::System,
                            content: format!("⚠ {msg}"),
                        });
                        messages.set(msgs);
                        streaming.set(false);
                    }
                }
            }
        });
    }

    // Keyboard: Enter sends, Ctrl+C / Ctrl+D quits.
    let action_tx = props.action_tx.clone();
    hooks.use_terminal_events({
        move |event| {
            if let TerminalEvent::Key(key) = event {
                if key.kind == KeyEventKind::Release {
                    return;
                }
                match key.code {
                    KeyCode::Char('c') | KeyCode::Char('d')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        should_exit.set(true);
                    }
                    KeyCode::Enter => {
                        let text = input.to_string();
                        let trimmed = text.trim();
                        if trimmed.is_empty() || streaming.get() {
                            return;
                        }
                        let mut msgs = messages.read().clone();
                        msgs.push(ChatMsg {
                            kind: MessageKind::User,
                            content: trimmed.to_string(),
                        });
                        messages.set(msgs);
                        input.set(String::new());
                        streaming.set(true);
                        if let Some(tx) = action_tx.as_ref() {
                            let _ = tx.send(UserAction::SendMessage(trimmed.to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    if should_exit.get() {
        if let Some(tx) = props.action_tx.as_ref() {
            let _ = tx.send(UserAction::Quit);
        }
        system.exit();
    }

    let header = format!(
        "zbot · {} · session {}",
        props.daemon_url,
        short_id(&props.session_id)
    );
    let prompt_marker = if streaming.get() { "… " } else { "▸ " };
    let prompt_color = if streaming.get() { Color::DarkGrey } else { Color::Green };

    let msg_views = messages
        .read()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let (color, prefix) = match m.kind {
                MessageKind::User => (Color::Cyan, "› "),
                MessageKind::Assistant => (Color::Reset, "◂ "),
                MessageKind::System => (Color::Yellow, "‼ "),
            };
            let content = format!("{prefix}{}", m.content);
            element! {
                View(key: i.to_string(), margin_bottom: 1) {
                    Text(content, color)
                }
            }
        })
        .collect::<Vec<_>>();

    let streaming_view: AnyElement<'static> = if active_assistant.read().is_empty() {
        element!(View).into()
    } else {
        let content = format!("◂ {}", active_assistant.to_string());
        element! {
            View(margin_bottom: 1) {
                Text(content, color: Color::Grey)
            }
        }
        .into()
    };

    element! {
        View(flex_direction: FlexDirection::Column, padding: 0) {
            View(margin_bottom: 1) {
                Text(content: header, color: Color::DarkGrey)
            }
            #(msg_views)
            #(streaming_view)
            View(margin_top: 1, flex_direction: FlexDirection::Row) {
                Text(content: prompt_marker.to_string(), color: prompt_color)
                TextInput(
                    has_focus: !streaming.get(),
                    value: input.to_string(),
                    on_change: move |v| input.set(v),
                )
            }
        }
    }
}

// =========================================================================
// WS bridge + entry point
// =========================================================================

/// Run the interactive chat REPL. Blocks until the user quits.
pub async fn run_interactive(
    chat: ChatInit,
    daemon_url: String,
    mut events: EventStream,
) -> Result<()> {
    events.subscribe(&chat.conversation_id)?;

    let (update_tx, update_rx) = mpsc::unbounded_channel::<UiUpdate>();
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<UserAction>();

    let conv = chat.conversation_id.clone();
    let initial_session = chat.session_id.clone();

    let bridge = tokio::spawn(async move {
        loop {
            tokio::select! {
                action = action_rx.recv() => {
                    match action {
                        Some(UserAction::SendMessage(text)) => {
                            let session = Some(initial_session.clone());
                            if let Err(e) = events.invoke("root", &conv, session, text) {
                                let _ = update_tx.send(UiUpdate::Error(format!("send failed: {e}")));
                            }
                        }
                        Some(UserAction::Quit) | None => break,
                    }
                }
                msg = events.recv() => {
                    let Some(msg) = msg else { break; };
                    if let Some(update) = map_server_msg(msg) {
                        if update_tx.send(update).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    element!(App(
        daemon_url,
        session_id: chat.session_id,
        update_rx: Some(update_rx),
        action_tx: Some(action_tx),
    ))
    .render_loop()
    .await
    .context("iocraft render loop")?;

    bridge.abort();
    Ok(())
}

fn map_server_msg(msg: ServerMessage) -> Option<UiUpdate> {
    match msg {
        ServerMessage::Token { delta, .. } => Some(UiUpdate::Token(delta)),
        ServerMessage::TurnComplete { .. } => Some(UiUpdate::TurnComplete),
        ServerMessage::AgentCompleted { .. } => Some(UiUpdate::TurnComplete),
        ServerMessage::Error { message, .. } => Some(UiUpdate::Error(message)),
        // Phase 4+ adds: ToolCall, ToolResult, Thinking, etc.
        _ => None,
    }
}

fn short_id(id: &str) -> String {
    id.rsplit('-').next().unwrap_or(id).chars().take(8).collect()
}
