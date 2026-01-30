//! # Gateway
//!
//! HTTP and WebSocket gateway for the AgentZero daemon.
//!
//! This crate provides the network interface for the agent runtime,
//! enabling multiple clients (Tauri, CLI, web) to interact with agents.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              Gateway                     │
//! ├─────────────────────────────────────────┤
//! │  WebSocket :18790  │  HTTP :18791       │
//! ├─────────────────────────────────────────┤
//! │           Event Bus (broadcast)         │
//! └─────────────────────────────────────────┘
//!              │
//!              ▼
//!     ┌─────────────────┐
//!     │  Agent Runtime  │
//!     └─────────────────┘
//! ```
//!
//! ## Features
//!
//! - **WebSocket API** - Real-time streaming for agent conversations
//! - **HTTP API** - RESTful endpoints for agents, conversations, tools
//! - **Event Bus** - Broadcast events to all connected clients

pub mod config;
pub mod database;
pub mod error;
pub mod events;
pub mod execution;
pub mod http;
pub mod server;
pub mod services;
pub mod state;
pub mod templates;
pub mod websocket;

pub use config::GatewayConfig;
pub use error::{GatewayError, Result};
pub use execution::{ExecutionRunner, ExecutionConfig, ExecutionHandle};
pub use server::GatewayServer;
pub use services::{AgentService, RuntimeService};
pub use state::AppState;

/// Default WebSocket port
pub const DEFAULT_WS_PORT: u16 = 18790;

/// Default HTTP port
pub const DEFAULT_HTTP_PORT: u16 = 18791;
