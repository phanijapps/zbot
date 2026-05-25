//! Interactive chat REPL — iocraft component tree (Direction B).
//!
//! Aesthetic: Cool Terminal — slate neutrals + violet primary + sky
//! secondary. All-caps role labels in the speaker's color, sharp
//! single-line borders around each block, animated braille spinner
//! while a turn is in flight, status footer pinned at the bottom.
//!
//! All palette / glyph constants live in `crate::theme` so future theme
//! swaps are a single-file change.

use anyhow::{Context, Result};
use gateway_ws_protocol::ServerMessage;
use iocraft::prelude::*;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::client::{ChatInit, DaemonClient};
use crate::events::EventStream;
use crate::slash::{self, SlashCommand};
use crate::theme;

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
    System(String),
    Tokens { tokens_in: u64, tokens_out: u64 },
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
    update_rx: Option<UnboundedReceiver<UiUpdate>>,
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
    let mut spinner_frame = hooks.use_state(|| 0_usize);
    let mut tokens_total = hooks.use_state(|| 0_u64);
    let mut system = hooks.use_context_mut::<SystemContext>();

    // ── update channel pump (always called; first render owns the rx) ──────
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
                UiUpdate::Tokens { tokens_in, tokens_out } => {
                    tokens_total.set(tokens_in + tokens_out);
                }
            }
        }
    });

    // ── spinner tick ───────────────────────────────────────────────────────
    hooks.use_future(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(theme::SPINNER_TICK_MS)).await;
            let next = (spinner_frame.get() + 1) % theme::SPINNER_FRAMES.len();
            spinner_frame.set(next);
        }
    });

    // ── keyboard ───────────────────────────────────────────────────────────
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
                        if let Some(cmd) = slash::parse(trimmed) {
                            input.set(String::new());
                            let mut msgs = messages.read().clone();
                            msgs.push(ChatMsg {
                                kind: MessageKind::User,
                                content: trimmed.to_string(),
                            });
                            messages.set(msgs);
                            if matches!(cmd, SlashCommand::Quit) {
                                should_exit.set(true);
                                return;
                            }
                            if let Some(tx) = action_tx.as_ref() {
                                let _ = tx.send(UserAction::RunSlash(cmd));
                            }
                            return;
                        }
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

    // ─────────────────────────────────────────────────────────────────────
    // Render
    // ─────────────────────────────────────────────────────────────────────

    let chat_empty = messages.read().is_empty() && active_assistant.read().is_empty();

    let welcome_view: AnyElement<'static> = if chat_empty {
        render_welcome(&props.daemon_url, &props.session_id)
    } else {
        element!(View).into()
    };

    let msg_views = messages
        .read()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let (border_color, label) = match m.kind {
                MessageKind::User => (theme::BORDER_USER, "YOU"),
                MessageKind::Assistant => (theme::BORDER_ASSISTANT, "ASSISTANT"),
                MessageKind::System => (theme::BORDER_SYSTEM, "SYSTEM"),
            };
            let lines = match m.kind {
                MessageKind::Assistant => crate::render::parse_markdown(&m.content),
                _ => crate::render::plain_lines(&m.content),
            };
            render_message_block(i, label, border_color, &lines)
        })
        .collect::<Vec<_>>();

    let tool_views = tool_calls
        .read()
        .iter()
        .enumerate()
        .map(|(i, tc)| render_tool_call_block(i, tc))
        .collect::<Vec<_>>();

    let streaming_view: AnyElement<'static> = if active_assistant.read().is_empty() {
        element!(View).into()
    } else {
        let lines = crate::render::plain_lines(&active_assistant.to_string());
        render_message_block(usize::MAX, "ASSISTANT", theme::BORDER_ASSISTANT, &lines)
    };

    let spinner_view: AnyElement<'static> = if streaming.get() {
        let frame = theme::SPINNER_FRAMES[spinner_frame.get() % theme::SPINNER_FRAMES.len()];
        element! {
            View(margin_top: 1, flex_direction: FlexDirection::Row) {
                Text(content: format!("{frame} "), color: theme::ACCENT)
                Text(content: "thinking".to_string(), color: theme::MUTED)
            }
        }
        .into()
    } else {
        element!(View).into()
    };

    let footer_left = format!("── {}", props.daemon_url.to_uppercase());
    let footer_right = format!(
        "── {} ── {} TOKS ── /HELP ──",
        short_id(&props.session_id).to_uppercase(),
        format_tokens(tokens_total.get()),
    );

    element! {
        View(flex_direction: FlexDirection::Column, padding: 0) {
            #(welcome_view)
            #(msg_views)
            #(tool_views)
            #(streaming_view)
            #(spinner_view)

            // Input row
            View(margin_top: 1, flex_direction: FlexDirection::Row) {
                Text(content: "> ".to_string(), color: theme::ACCENT, weight: Weight::Bold)
                TextInput(
                    has_focus: !streaming.get(),
                    value: input.to_string(),
                    on_change: move |v| input.set(v),
                )
            }

            // Status footer
            View(
                margin_top: 1,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
            ) {
                Text(content: footer_left, color: theme::MUTED_DIM)
                Text(content: footer_right, color: theme::MUTED_DIM)
            }
        }
    }
}

// =========================================================================
// Render helpers
// =========================================================================

fn render_welcome(daemon_url: &str, session_id: &str) -> AnyElement<'static> {
    let title = format!("ZBOT v{}", env!("CARGO_PKG_VERSION"));
    let daemon_line = format!("daemon   {daemon_url}");
    let session_line = format!("session  {}", short_id(session_id));
    let hint = "↵ to send · /help · ⌃C to quit".to_string();

    element! {
        View(
            border_style: BorderStyle::Single,
            border_color: theme::ACCENT,
            padding_left: 2,
            padding_right: 2,
            padding_top: 1,
            padding_bottom: 1,
            margin_bottom: 1,
            flex_direction: FlexDirection::Column,
        ) {
            Text(content: title, color: theme::ACCENT, weight: Weight::Bold)
            View(margin_top: 1, flex_direction: FlexDirection::Column) {
                Text(content: daemon_line, color: theme::MUTED)
                Text(content: session_line, color: theme::MUTED)
            }
            View(margin_top: 1) {
                Text(content: hint, color: theme::MUTED_DIM)
            }
        }
    }
    .into()
}

fn render_message_block(
    key: usize,
    role_label: &str,
    border_color: Color,
    lines: &[crate::render::Line],
) -> AnyElement<'static> {
    let label = role_label.to_string();
    let lines_owned: Vec<crate::render::Line> = lines.to_vec();

    element! {
        View(
            key: key.to_string(),
            flex_direction: FlexDirection::Column,
            margin_bottom: 1,
        ) {
            // All-caps role label outside the border, in the speaker's color
            Text(content: label, color: border_color, weight: Weight::Bold)
            // Bordered content
            View(
                border_style: BorderStyle::Single,
                border_color,
                padding_left: 1,
                padding_right: 1,
                flex_direction: FlexDirection::Column,
            ) {
                #(lines_owned.iter().enumerate().map(|(li, line)| {
                    render_content_row(li, line)
                }).collect::<Vec<_>>())
            }
        }
    }
    .into()
}

fn render_content_row(idx: usize, line: &crate::render::Line) -> AnyElement<'static> {
    use crate::render::{LineKind, SpanStyle};

    if matches!(line.kind, LineKind::Blank) || line.spans.is_empty() {
        return element! {
            View(key: format!("c-{idx}")) {
                Text(content: " ".to_string())
            }
        }
        .into();
    }

    let has_bold = line.spans.iter().any(|s| s.style == SpanStyle::Bold);
    let has_code = line.spans.iter().any(|s| s.style == SpanStyle::Code);

    let (text_color, text_weight, prefix): (Color, Weight, &'static str) = match (&line.kind, has_bold, has_code) {
        (LineKind::Heading { .. }, _, _) => (Color::White, Weight::Bold, ""),
        (LineKind::CodeBlock, _, _) => (theme::SECONDARY, Weight::Normal, "  "),
        (LineKind::Bullet, _, _) => (theme::TEXT, Weight::Normal, "▪ "),
        (_, true, _) => (Color::White, Weight::Bold, ""),
        (_, _, true) => (theme::SECONDARY, Weight::Normal, ""),
        _ => (theme::TEXT, Weight::Normal, ""),
    };

    let content: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    let full = format!("{prefix}{content}");

    element! {
        View(key: format!("c-{idx}")) {
            Text(content: full, color: text_color, weight: text_weight)
        }
    }
    .into()
}

fn render_tool_call_block(key: usize, tc: &ToolCallView) -> AnyElement<'static> {
    let (status_label, status_color) = match &tc.status {
        None => ("running", theme::MUTED),
        Some(Ok(_)) => ("✓", theme::SUCCESS),
        Some(Err(_)) => ("✗", theme::ERROR),
    };
    let head = format!("{} · {}", tc.tool, status_label);
    let detail = match &tc.status {
        None => format!("args {}", truncate(&tc.args, 60)),
        Some(Ok(s)) => truncate(s, 80),
        Some(Err(e)) => format!("error: {}", truncate(e, 70)),
    };

    element! {
        View(
            key: format!("tc-{key}"),
            flex_direction: FlexDirection::Column,
            margin_bottom: 1,
            padding_left: 2,
        ) {
            View(
                border_style: BorderStyle::Single,
                border_color: theme::BORDER_TOOL,
                padding_left: 1,
                padding_right: 1,
                flex_direction: FlexDirection::Column,
            ) {
                Text(content: head, color: status_color, weight: Weight::Bold)
                Text(content: detail, color: theme::MUTED)
            }
        }
    }
    .into()
}

// =========================================================================
// WS + slash bridge + entry point
// =========================================================================

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
        SlashCommand::Quit => {}
        SlashCommand::New => match client.clear_chat_session().await {
            Ok(()) => match client.init_chat_session().await {
                Ok(chat) => {
                    *session_id = chat.session_id.clone();
                    push(format!(
                        "session cleared. new session: {}",
                        short_id(&chat.session_id)
                    ));
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
        SlashCommand::Unknown(name) => push(format!("unknown command: /{name} — try /help")),
    }
}

fn format_conversations(v: &Value) -> String {
    let arr = v.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if arr.is_empty() {
        return "no conversations".into();
    }
    let mut out = String::from("recent conversations:\n");
    for c in arr.iter().take(10) {
        let id = c.get("id").and_then(Value::as_str).unwrap_or("?");
        let title = c.get("title").and_then(Value::as_str).unwrap_or("(untitled)");
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
        let category = item.get("category").and_then(Value::as_str).unwrap_or("");
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
        ServerMessage::TokenUsage { tokens_in, tokens_out, .. } => {
            Some(UiUpdate::Tokens { tokens_in, tokens_out })
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

fn format_tokens(n: u64) -> String {
    if n < 1_000 {
        format!("{n}")
    } else if n < 100_000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        format!("{}K", n / 1000)
    }
}
