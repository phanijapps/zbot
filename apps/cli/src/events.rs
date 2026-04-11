// ============================================================================
// EVENT HANDLING
// Keyboard input and gateway event processing
// ============================================================================

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::client::GatewayEvent;

// ============================================================================
// App Events
// ============================================================================

#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input
    Key(KeyEvent),

    /// Gateway event received
    Gateway(GatewayEvent),

    /// Tick for UI updates
    Tick,

    /// Resize event
    #[allow(dead_code)]
    Resize(u16, u16),
}

// ============================================================================
// Event Handler
// ============================================================================

pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    _tx: mpsc::Sender<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(100);

        let event_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                // Poll for keyboard events
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key)) => {
                            if event_tx.send(AppEvent::Key(key)).await.is_err() {
                                break;
                            }
                        }
                        Ok(Event::Resize(w, h)) => {
                            if event_tx.send(AppEvent::Resize(w, h)).await.is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Send tick event
                    if event_tx.send(AppEvent::Tick).await.is_err() {
                        break;
                    }
                }
            }
        });

        Self { rx, _tx: tx }
    }

    /// Get a sender for gateway events
    pub fn gateway_sender(&self) -> mpsc::Sender<AppEvent> {
        self._tx.clone()
    }

    /// Receive the next event
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

// ============================================================================
// Key Helpers
// ============================================================================

#[allow(dead_code)]
pub fn is_quit(key: &KeyEvent) -> bool {
    matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } | KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    )
}

#[allow(dead_code)]
pub fn is_enter(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Enter)
}

#[allow(dead_code)]
pub fn is_backspace(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Backspace)
}

#[allow(dead_code)]
pub fn is_escape(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc)
}

#[allow(dead_code)]
pub fn is_up(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Up)
}

#[allow(dead_code)]
pub fn is_down(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Down)
}

#[allow(dead_code)]
pub fn is_page_up(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::PageUp)
}

#[allow(dead_code)]
pub fn is_page_down(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::PageDown)
}
