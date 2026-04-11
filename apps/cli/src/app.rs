// ============================================================================
// APP STATE & TUI LOOP
// Main application state and event loop
// ============================================================================

use anyhow::Result;
use crossterm::{
    event::KeyCode,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::client::{GatewayClient, GatewayEvent};
use crate::events::{AppEvent, EventHandler};
use crate::ui;

// ============================================================================
// App State
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_name: Option<String>,
}

pub struct AppState {
    pub agent_id: String,
    pub conversation_id: String,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub input_mode: InputMode,
    pub connected: bool,
    pub is_processing: bool,
    pub current_tool: Option<String>,
    pub iteration: Option<(u32, u32)>,
    pub scroll_offset: usize,
    pub should_quit: bool,

    // Streaming state
    current_assistant_message: String,
}

impl AppState {
    pub fn new(agent_id: &str, conversation_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            conversation_id: conversation_id.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: format!("Chat with agent '{}'. Press 'i' to start typing.", agent_id),
                tool_name: None,
            }],
            input: String::new(),
            input_mode: InputMode::Normal,
            connected: false,
            is_processing: false,
            current_tool: None,
            iteration: None,
            scroll_offset: 0,
            should_quit: false,
            current_assistant_message: String::new(),
        }
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: content.to_string(),
            tool_name: None,
        });
    }

    pub fn append_assistant_token(&mut self, token: &str) {
        self.current_assistant_message.push_str(token);

        // Update the last message if it's from assistant, otherwise create new
        if let Some(last) = self.messages.last_mut() {
            if matches!(last.role, MessageRole::Assistant) && last.tool_name.is_none() {
                last.content = self.current_assistant_message.clone();
                return;
            }
        }

        // Create new assistant message
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: self.current_assistant_message.clone(),
            tool_name: None,
        });
    }

    pub fn add_tool_message(&mut self, tool_name: &str, result: &str) {
        self.messages.push(ChatMessage {
            role: MessageRole::Tool,
            content: result.to_string(),
            tool_name: Some(tool_name.to_string()),
        });
    }

    pub fn finish_assistant_message(&mut self) {
        self.current_assistant_message.clear();
    }

    pub fn add_error_message(&mut self, error: &str) {
        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: format!("Error: {}", error),
            tool_name: None,
        });
    }
}

// ============================================================================
// TUI Application
// ============================================================================

pub async fn run_chat_tui(
    gateway_url: &str,
    ws_url: &str,
    agent_id: &str,
    conversation_id: &str,
) -> Result<()> {
    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut state = AppState::new(agent_id, conversation_id);

    // Create gateway client
    let client = GatewayClient::new(gateway_url, ws_url);

    // Check connection
    state.connected = client.is_running().await;
    if !state.connected {
        state.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Warning: Gateway daemon not running. Start with: cargo run -p daemon"
                .to_string(),
            tool_name: None,
        });
    }

    // Create event handler
    let mut events = EventHandler::new(Duration::from_millis(100));
    let gateway_tx = events.gateway_sender();

    // Main loop
    loop {
        // Draw UI
        terminal.draw(|f| ui::render(f, &state))?;

        // Handle events
        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => {
                    handle_key_event(&mut state, key, &client, gateway_tx.clone()).await?;
                }
                AppEvent::Gateway(gw_event) => {
                    handle_gateway_event(&mut state, gw_event);
                }
                AppEvent::Tick => {
                    // UI refresh
                }
                AppEvent::Resize(_, _) => {
                    // Terminal handles resize
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn handle_key_event(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    client: &GatewayClient,
    gateway_tx: mpsc::Sender<AppEvent>,
) -> Result<()> {
    use crossterm::event::KeyModifiers;

    // Global quit with Ctrl+C or Ctrl+Q
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q')) {
            state.should_quit = true;
            return Ok(());
        }

    match state.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('i') | KeyCode::Char('I') => {
                state.input_mode = InputMode::Editing;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                state.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.scroll_offset = state.scroll_offset.saturating_add(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.scroll_offset = state.scroll_offset.saturating_sub(1);
            }
            _ => {}
        },
        InputMode::Editing => {
            match key.code {
                KeyCode::Enter => {
                    if !state.input.is_empty() && !state.is_processing {
                        let message = state.input.clone();
                        state.input.clear();
                        state.add_user_message(&message);
                        state.is_processing = true;
                        state.input_mode = InputMode::Normal;

                        // Send to gateway
                        if state.connected {
                            let mut rx = client
                                .invoke(&state.agent_id, &state.conversation_id, &message)
                                .await?;

                            // Spawn task to forward gateway events
                            let tx = gateway_tx.clone();
                            tokio::spawn(async move {
                                while let Some(event) = rx.recv().await {
                                    if tx.send(AppEvent::Gateway(event)).await.is_err() {
                                        break;
                                    }
                                }
                            });
                        } else {
                            state.add_error_message("Not connected to gateway");
                            state.is_processing = false;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    state.input.push(c);
                }
                KeyCode::Backspace => {
                    state.input.pop();
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn handle_gateway_event(state: &mut AppState, event: GatewayEvent) {
    match event {
        GatewayEvent::Connected { .. } => {
            state.connected = true;
        }
        GatewayEvent::Token { content } => {
            state.append_assistant_token(&content);
        }
        GatewayEvent::Thinking { content } => {
            // Could show thinking indicator
            state.current_tool = Some(format!("Thinking: {}", content));
        }
        GatewayEvent::ToolCall { tool, .. } => {
            state.current_tool = Some(tool);
        }
        GatewayEvent::ToolResult { result, error, .. } => {
            let tool_name = state
                .current_tool
                .take()
                .unwrap_or_else(|| "tool".to_string());
            if let Some(err) = error {
                state.add_tool_message(&tool_name, &format!("Error: {}", err));
            } else if let Some(res) = result {
                // Truncate long results
                let display = if res.len() > 500 {
                    format!("{}...", &res[..res.floor_char_boundary(500)])
                } else {
                    res
                };
                state.add_tool_message(&tool_name, &display);
            }
        }
        GatewayEvent::Iteration { current, max } => {
            state.iteration = Some((current, max));
        }
        GatewayEvent::Done { .. } => {
            state.finish_assistant_message();
            state.is_processing = false;
            state.current_tool = None;
            state.iteration = None;
        }
        GatewayEvent::Error { message, .. } => {
            state.add_error_message(&message);
            state.is_processing = false;
            state.current_tool = None;
        }
    }
}
