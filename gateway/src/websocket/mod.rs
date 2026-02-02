//! # WebSocket Module
//!
//! WebSocket server for real-time agent communication.

mod handler;
mod messages;
mod session;
mod subscriptions;

pub use handler::WebSocketHandler;
pub use messages::{ClientMessage, ServerMessage, SubscriptionErrorCode};
pub use session::{SessionRegistry, WsSession};
pub use subscriptions::{SubscriptionManager, SubscribeError, SubscribeResult, RoutingResult};
