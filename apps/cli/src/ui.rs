//! Interactive chat REPL — iocraft component tree.
//!
//! Phase 4 adds:
//! - Slash command dispatch (`/help`, `/new`, `/sessions`, `/wards`,
//!   `/memory <q>`, `/quit`)
//! - Tool-call visualization (compact one-liner per call)
//! - System messages (slash-command output rendered inline)
//!
//! Phase 5+ adds permission prompts. Phase 6 adds markdown rendering
//! and one-shot mode.

use anyhow::{Context, Result};
use gateway_ws_protocol::ServerMessage;
use iocraft::prelude::*;
use serde_json::Value;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::client::{ChatInit, DaemonClient};
use crate::events::EventStream;
use crate::slash::{self, SlashCommand};

// =========================================================================
// Channel payloads
// =========================================================================

#[derive(Debug, Clone)]
enum UiUpdate {
    Token(String),
    ToolCallStarted { id: String, tool: String, args: String },
    ToolCallCompleted { id: String, ok: bool, summary: String },
    TurnComplete,
    Error(String),
    /// Slash-command output or other inline informational messages.
    System(String),
}

#[derive(Debug)]
enum UserAction {
    SendMessage(String),
    RunSlash(SlashCommand),
    Quit,
}

// =========================================================================
// Domain model
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

#[derive(Clone, Debug)]
struct ToolCallView {
    id: String,
    tool: String,
    args: String,
    /// `None` = running, `Some(Ok(summary))` = completed, `Some(Err(msg))` = failed.
    status: Option<Result<String, String>>,
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
    /// Cloned per-callback.
    action_tx: Option<UnboundedSender<UserAction>>,
}

#[component]
fn App(props: &mut AppProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut messages = hooks.use_state(Vec::<ChatMsg>::new);
    let mut active_assistant = hooks.use_state(String::new);
    let mut tool_calls = hooks.use_state(Vec::<ToolCallView>::new);
    let mut input = hooks.use_state(String::new);
    let mut streaming = hooks.use_state(|| false);
    let mut should_exit = hooks.use_state(|| false);
    let mut system = hooks.use_context_mut::<SystemContext>();

    // Always call use_future (rules of hooks). The future captures the
    // only rx on first render; re-render closures see None and bail.
    let rx_opt = props.update_rx.take();
    hooks.use_future(async move {
        let Some(mut rx) = rx_opt else { return; };
        while let Some(update) = rx.recv().await {
            match update {
                UiUpdate::Token(delta) => {
                    let mut current = active_assistant.to_string();
                    current.push_str(&delta);
                    active_assistant.set(current);
                    streaming.set(true);
                }
                UiUpdate::ToolCallStarted { id, tool, args } => {
                    let mut tcs = tool_calls.read().clone();
                    tcs.push(ToolCallView { id, tool, args, status: None });
                    tool_calls.set(tcs);
                }
                UiUpdate::ToolCallCompleted { id, ok, summary } => {
                    let mut tcs = tool_calls.read().clone();
                    if let Some(tc) = tcs.iter_mut().find(|t| t.id == id) {
                        tc.status = Some(if ok { Ok(summary) } else { Err(summary) });
                    }
                    tool_calls.set(tcs);
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
                UiUpdate::System(msg) => {
                    let mut msgs = messages.read().clone();
                    msgs.push(ChatMsg {
                        kind: MessageKind::System,
                        content: msg,
                    });
                    messages.set(msgs);
                }
            }
        }
    });

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
                        if trimmed.is_empty() {
                            return;
                        }
                        // Slash command?
                        if let Some(cmd) = slash::parse(trimmed) {
                            input.set(String::new());
                            // Echo the command into history so users can see what they ran.
                            let mut msgs = messages.read().clone();
                            msgs.push(ChatMsg {
                                kind: MessageKind::User,
                                content: trimmed.to_string(),
                            });
                            messages.set(msgs);

                            // /quit handled locally for snappiness.
                            if matches!(cmd, SlashCommand::Quit) {
                                should_exit.set(true);
                                return;
                            }
                            if let Some(tx) = action_tx.as_ref() {
                                let _ = tx.send(UserAction::RunSlash(cmd));
                            }
                            return;
                        }
                        // Streaming guard only blocks normal messages — slash
                        // commands always work.
                        if streaming.get() {
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

    // ---- Render ----

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

    let tool_views = tool_calls
        .read()
        .iter()
        .enumerate()
        .map(|(i, tc)| {
            let (icon, color) = match &tc.status {
                None => ("▶", Color::DarkYellow),
                Some(Ok(_)) => ("✓", Color::DarkGreen),
                Some(Err(_)) => ("✗", Color::DarkRed),
            };
            let detail = match &tc.status {
                None => format!("running · args={}", truncate(&tc.args, 50)),
                Some(Ok(s)) => truncate(s, 70),
                Some(Err(e)) => format!("error: {}", truncate(e, 60)),
            };
            let line = format!("  {icon} {} · {}", tc.tool, detail);
            element! {
                View(key: format!("tc-{i}")) {
                    Text(content: line, color)
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
            #(tool_views)
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
// WS + slash bridge + entry point
// =========================================================================

/// Run the interactive chat REPL. Blocks until the user quits.
pub async fn run_interactive(
    chat: ChatInit,
    daemon_url: String,
    mut events: EventStream,
    client: DaemonClient,
) -> Result<()> {
    events.subscribe(&chat.conversation_id)?;

    let (update_tx, update_rx) = mpsc::unbounded_channel::<UiUpdate>();
    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<UserAction>();

    let conv = chat.conversation_id.clone();
    let initial_session = chat.session_id.clone();

    let bridge = tokio::spawn(async move {
        let mut session_id = initial_session;
        loop {
            tokio::select! {
                action = action_rx.recv() => {
                    match action {
                        Some(UserAction::SendMessage(text)) => {
                            if let Err(e) = events.invoke("root", &conv, Some(session_id.clone()), text) {
                                let _ = update_tx.send(UiUpdate::Error(format!("send failed: {e}")));
                            }
                        }
                        Some(UserAction::RunSlash(cmd)) => {
                            handle_slash(cmd, &client, &update_tx, &mut session_id).await;
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

async fn handle_slash(
    cmd: SlashCommand,
    client: &DaemonClient,
    update_tx: &UnboundedSender<UiUpdate>,
    session_id: &mut String,
) {
    let push = |s: String| {
        let _ = update_tx.send(UiUpdate::System(s));
    };
    match cmd {
        SlashCommand::Help => push(slash::HELP_TEXT.to_string()),
        SlashCommand::Quit => { /* handled in App locally */ }
        SlashCommand::New => match client.clear_chat_session().await {
            Ok(()) => match client.init_chat_session().await {
                Ok(chat) => {
                    *session_id = chat.session_id.clone();
                    push(format!("session cleared. new session: {}", short_id(&chat.session_id)));
                }
                Err(e) => push(format!("session cleared, but init failed: {e}")),
            },
            Err(e) => push(format!("clear failed: {e}")),
        },
        SlashCommand::Sessions => match client.list_conversations().await {
            Ok(v) => push(format_conversations(&v)),
            Err(e) => push(format!("list failed: {e}")),
        },
        SlashCommand::Wards => match client.list_wards().await {
            Ok(v) => push(format_wards(&v)),
            Err(e) => push(format!("list failed: {e}")),
        },
        SlashCommand::Memory(q) => {
            if q.is_empty() {
                push("usage: /memory <query>".into());
            } else {
                match client.memory_search(&q, 8).await {
                    Ok(v) => push(format_memory(&v)),
                    Err(e) => push(format!("recall failed: {e}")),
                }
            }
        }
        SlashCommand::Unknown(name) => push(format!(
            "unknown command: /{name} — try /help"
        )),
    }
}

fn format_conversations(v: &Value) -> String {
    let arr = v.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if arr.is_empty() {
        return "no conversations".into();
    }
    let mut out = String::from("recent conversations:\n");
    for c in arr.iter().take(10) {
        let id = c
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("?");
        let title = c
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("(untitled)");
        let updated = c
            .get("updatedAt")
            .or_else(|| c.get("updated_at"))
            .and_then(Value::as_str)
            .unwrap_or("");
        out.push_str(&format!("  {}  {}  {}\n", short_id(id), title, updated));
    }
    out.trim_end().to_string()
}

fn format_wards(v: &Value) -> String {
    let arr = v.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if arr.is_empty() {
        return "no wards".into();
    }
    let mut out = String::from("wards:\n");
    for w in arr {
        let name = w
            .get("name")
            .or_else(|| w.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let desc = w
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        out.push_str(&format!("  {}  {}\n", name, truncate(desc, 60)));
    }
    out.trim_end().to_string()
}

fn format_memory(v: &Value) -> String {
    let items = if v.is_array() {
        v.as_array().unwrap().as_slice()
    } else if let Some(arr) = v.get("items").and_then(Value::as_array) {
        arr.as_slice()
    } else if let Some(arr) = v.get("results").and_then(Value::as_array) {
        arr.as_slice()
    } else {
        &[]
    };
    if items.is_empty() {
        return "no matches".into();
    }
    let mut out = String::from("recall:\n");
    for item in items.iter().take(8) {
        let content = item
            .get("content")
            .or_else(|| item.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let category = item
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("");
        let cat = if category.is_empty() {
            String::new()
        } else {
            format!("[{category}] ")
        };
        out.push_str(&format!("  {cat}{}\n", truncate(content, 80)));
    }
    out.trim_end().to_string()
}

fn map_server_msg(msg: ServerMessage) -> Option<UiUpdate> {
    match msg {
        ServerMessage::Token { delta, .. } => Some(UiUpdate::Token(delta)),
        ServerMessage::TurnComplete { .. } => Some(UiUpdate::TurnComplete),
        ServerMessage::AgentCompleted { .. } => Some(UiUpdate::TurnComplete),
        ServerMessage::Error { message, .. } => Some(UiUpdate::Error(message)),
        ServerMessage::ToolCall { tool_call_id, tool, args, .. } => {
            Some(UiUpdate::ToolCallStarted {
                id: tool_call_id,
                tool,
                args: args.to_string(),
            })
        }
        ServerMessage::ToolResult { tool_call_id, result, error, .. } => {
            let ok = error.is_none();
            let summary = error.unwrap_or_else(|| first_line(&result));
            Some(UiUpdate::ToolCallCompleted {
                id: tool_call_id,
                ok,
                summary,
            })
        }
        _ => None,
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(n).collect();
        format!("{truncated}…")
    }
}

fn short_id(id: &str) -> String {
    id.rsplit('-').next().unwrap_or(id).chars().take(8).collect()
}
