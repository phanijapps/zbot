//! # Bridge Errors

use thiserror::Error;

/// Errors from the bridge system.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// Worker not connected.
    #[error("Worker '{0}' is not connected")]
    NotConnected(String),

    /// Worker already connected.
    #[error("Worker '{0}' is already connected")]
    AlreadyConnected(String),

    /// Hello handshake timed out.
    #[error("Hello handshake timed out after {0}s")]
    HelloTimeout(u64),

    /// Request timed out waiting for worker response.
    #[error("Request '{0}' timed out after {1}s")]
    RequestTimeout(String, u64),

    /// Worker sent an invalid message.
    #[error("Invalid message from worker: {0}")]
    InvalidMessage(String),

    /// Internal channel error.
    #[error("Channel error: {0}")]
    Channel(String),

    /// Database error.
    #[error("Database error: {0}")]
    Database(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}
