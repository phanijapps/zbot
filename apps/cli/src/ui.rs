// ============================================================================
// UI RENDERING
// TUI rendering using ratatui
// ============================================================================

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

use crate::app::{AppState, ChatMessage, InputMode, MessageRole};

// ============================================================================
// Color Scheme
// ============================================================================

const COLOR_BG: Color = Color::Rgb(26, 26, 26);
const COLOR_FG: Color = Color::Rgb(230, 230, 230);
const COLOR_ACCENT: Color = Color::Rgb(138, 43, 226); // Purple
const COLOR_ACCENT_DIM: Color = Color::Rgb(88, 28, 135);
const COLOR_SUCCESS: Color = Color::Rgb(34, 197, 94);
const COLOR_WARNING: Color = Color::Rgb(234, 179, 8);
const COLOR_ERROR: Color = Color::Rgb(239, 68, 68);
const COLOR_MUTED: Color = Color::Rgb(115, 115, 115);
const COLOR_USER: Color = Color::Rgb(96, 165, 250);
const COLOR_ASSISTANT: Color = Color::Rgb(167, 139, 250);
const COLOR_TOOL: Color = Color::Rgb(251, 191, 36);

// ============================================================================
// Main UI Render
// ============================================================================

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Main layout: header, content, input, status
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Chat messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    render_header(frame, main_layout[0], state);
    render_messages(frame, main_layout[1], state);
    render_input(frame, main_layout[2], state);
    render_status_bar(frame, main_layout[3], state);
}

// ============================================================================
// Header
// ============================================================================

fn render_header(frame: &mut Frame, area: Rect, state: &AppState) {
    let status_indicator = if state.connected {
        Span::styled(" ● ", Style::default().fg(COLOR_SUCCESS))
    } else {
        Span::styled(" ○ ", Style::default().fg(COLOR_ERROR))
    };

    let title = Line::from(vec![
        Span::styled(
            "  ZERO",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(COLOR_MUTED)),
        Span::styled(&state.agent_id, Style::default().fg(COLOR_FG)),
        status_indicator,
    ]);

    let iteration_info = if state.is_processing {
        if let Some((current, max)) = state.iteration {
            format!("Iteration {}/{}", current, max)
        } else {
            "Processing...".to_string()
        }
    } else {
        String::new()
    };

    let right_info = Line::from(vec![Span::styled(
        &iteration_info,
        Style::default().fg(COLOR_MUTED),
    )]);

    let header = Paragraph::new(vec![title, right_info]).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(COLOR_ACCENT_DIM)),
    );

    frame.render_widget(header, area);
}

// ============================================================================
// Messages
// ============================================================================

fn render_messages(frame: &mut Frame, area: Rect, state: &AppState) {
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    let messages: Vec<ListItem> = state
        .messages
        .iter()
        .flat_map(|msg| message_to_list_items(msg, inner_area.width as usize))
        .collect();

    let messages_widget = List::new(messages).block(Block::default());

    // Calculate scroll state
    let total_lines: usize = state
        .messages
        .iter()
        .map(|m| estimate_lines(m, inner_area.width as usize))
        .sum();

    let visible_lines = area.height as usize;
    let scroll_offset = if total_lines > visible_lines {
        total_lines.saturating_sub(visible_lines)
    } else {
        0
    };

    // Render with scroll
    frame.render_widget(messages_widget, inner_area);

    // Scrollbar
    if total_lines > visible_lines {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_offset);

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn message_to_list_items<'a>(msg: &'a ChatMessage, width: usize) -> Vec<ListItem<'a>> {
    let mut items = Vec::new();

    // Role indicator
    let (role_str, role_color) = match msg.role {
        MessageRole::User => ("You", COLOR_USER),
        MessageRole::Assistant => ("Assistant", COLOR_ASSISTANT),
        MessageRole::System => ("System", COLOR_MUTED),
        MessageRole::Tool => ("Tool", COLOR_TOOL),
    };

    // Header line
    let header_text = format!(
        "─── {} {}",
        role_str,
        "─".repeat(width.saturating_sub(role_str.len() + 5))
    );
    items.push(ListItem::new(Line::styled(
        header_text,
        Style::default().fg(role_color).add_modifier(Modifier::BOLD),
    )));

    // Content
    let content_style = Style::default().fg(match msg.role {
        MessageRole::User => COLOR_FG,
        MessageRole::Assistant => COLOR_FG,
        MessageRole::System => COLOR_MUTED,
        MessageRole::Tool => COLOR_MUTED,
    });

    for line in msg.content.lines() {
        let line_text = format!("  {}", line);
        items.push(ListItem::new(Line::styled(line_text, content_style)));
    }

    // Tool info if present
    if let Some(tool) = &msg.tool_name {
        let tool_text = format!("  [{}]", tool);
        items.push(ListItem::new(Line::styled(
            tool_text,
            Style::default().fg(COLOR_TOOL),
        )));
    }

    // Empty line after message
    items.push(ListItem::new(Line::raw("")));

    items
}

fn estimate_lines(msg: &ChatMessage, _width: usize) -> usize {
    let content_lines = msg.content.lines().count();
    let header_lines = 1;
    let tool_lines = if msg.tool_name.is_some() { 1 } else { 0 };
    let spacing = 1;

    header_lines + content_lines + tool_lines + spacing
}

// ============================================================================
// Input
// ============================================================================

fn render_input(frame: &mut Frame, area: Rect, state: &AppState) {
    let input_style = match state.input_mode {
        InputMode::Normal => Style::default().fg(COLOR_MUTED),
        InputMode::Editing => Style::default().fg(COLOR_FG),
    };

    let prompt = match state.input_mode {
        InputMode::Normal => "Press 'i' to type, 'q' to quit",
        InputMode::Editing => "",
    };

    let display_text = if state.input.is_empty() && matches!(state.input_mode, InputMode::Normal) {
        prompt.to_string()
    } else {
        state.input.clone()
    };

    let input = Paragraph::new(display_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(match state.input_mode {
                InputMode::Normal => COLOR_MUTED,
                InputMode::Editing => COLOR_ACCENT,
            }))
            .title(match state.input_mode {
                InputMode::Normal => " Input ",
                InputMode::Editing => " Type message (Enter to send, Esc to cancel) ",
            })
            .title_style(Style::default().fg(COLOR_ACCENT)),
    );

    frame.render_widget(input, area);

    // Show cursor in editing mode
    if matches!(state.input_mode, InputMode::Editing) {
        frame.set_cursor_position((area.x + state.input.len() as u16 + 1, area.y + 1));
    }
}

// ============================================================================
// Status Bar
// ============================================================================

fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status_text = if state.is_processing {
        vec![
            Span::styled("⏳ ", Style::default().fg(COLOR_WARNING)),
            Span::styled("Processing", Style::default().fg(COLOR_WARNING)),
            if let Some(tool) = &state.current_tool {
                Span::styled(
                    format!(" │ Tool: {}", tool),
                    Style::default().fg(COLOR_TOOL),
                )
            } else {
                Span::raw("")
            },
        ]
    } else {
        vec![
            Span::styled(
                if state.connected {
                    "● Connected"
                } else {
                    "○ Disconnected"
                },
                Style::default().fg(if state.connected {
                    COLOR_SUCCESS
                } else {
                    COLOR_ERROR
                }),
            ),
            Span::styled(" │ ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                format!(
                    "Conv: {}...",
                    &state.conversation_id[..8.min(state.conversation_id.len())]
                ),
                Style::default().fg(COLOR_MUTED),
            ),
        ]
    };

    let keybinds_text = " Ctrl+C quit  i input  Esc cancel ";
    let keybinds_width = keybinds_text.len() as u16;

    let left = Line::from(status_text);

    // Render left-aligned status
    let status = Paragraph::new(left).style(Style::default().bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(status, area);

    // Render right-aligned keybinds
    let right_width = keybinds_width;
    let right_area = Rect {
        x: area.x + area.width.saturating_sub(right_width + 1),
        y: area.y,
        width: right_width,
        height: 1,
    };
    let keybinds_widget = Paragraph::new(keybinds_text)
        .style(Style::default().fg(COLOR_MUTED).bg(Color::Rgb(30, 30, 30)));
    frame.render_widget(keybinds_widget, right_area);
}
