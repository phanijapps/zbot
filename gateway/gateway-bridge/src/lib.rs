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

pub mod error;
pub mod handler;
pub mod outbox;
pub mod pending_requests;
pub mod protocol;
pub mod provider;
pub mod push;
pub mod registry;

// Re-export public types
pub use error::BridgeError;
pub use handler::handle_worker_connection;
pub use outbox::OutboxRepository;
pub use pending_requests::PendingRequests;
pub use protocol::{BridgeServerMessage, WorkerCapability, WorkerMessage, WorkerResource};
pub use provider::BridgeResourceProvider;
pub use push::{enqueue_and_push, spawn_retry_loop};
pub use registry::{BridgeRegistry, WorkerSummary};
