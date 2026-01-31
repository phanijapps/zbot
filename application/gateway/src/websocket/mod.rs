//! # WebSocket Module
//!
//! WebSocket server for real-time agent communication.

mod handler;
mod messages;
mod session;

pub use handler::WebSocketHandler;
pub use messages::{ClientMessage, ServerMessage};
pub use session::{SessionRegistry, WsSession};
