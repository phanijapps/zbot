#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
//! # Gateway Bridge
//!
//! WebSocket worker system for bidirectional agent-worker communication.
//!
//! Workers connect via WebSocket, self-describe their capabilities and resources,
//! and receive outbound pushes in real-time. The bridge provides:
//!
//! - **Protocol**: Typed messages for worker <-> server communication
//! - **Registry**: In-memory tracking of connected workers
//! - **Outbox**: SQLite-backed reliable delivery with ACK/replay
//! - **Push**: Outbox drain + retry loop for connected workers
//! - **Provider**: Bridge-aware `ConnectorResourceProvider` implementation
//! - **Handler**: Per-worker WebSocket session loop
//! - **Plugins**: STDIO plugin management for Node.js extensions

pub mod error;
pub mod handler;
pub mod outbox;
pub mod pending_requests;
pub mod plugin_config;
pub mod plugin_manager;
pub mod protocol;
pub mod provider;
pub mod push;
pub mod registry;
pub mod stdio_plugin;

// Re-export public types
pub use error::BridgeError;
pub use handler::handle_worker_connection;
pub use outbox::OutboxRepository;
pub use pending_requests::PendingRequests;
pub use plugin_config::{PluginConfig, PluginError, PluginState, PluginSummary, PluginUserConfig};
pub use plugin_manager::PluginManager;
pub use protocol::{BridgeServerMessage, WorkerCapability, WorkerMessage, WorkerResource};
pub use provider::BridgeResourceProvider;
pub use push::{enqueue_and_push, spawn_retry_loop};
pub use registry::{BridgeRegistry, WorkerSummary};
pub use stdio_plugin::StdioPlugin;
