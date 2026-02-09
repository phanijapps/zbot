//! # Gateway WebSocket Protocol
//!
//! Message types for WebSocket communication between the gateway and clients.
//!
//! This crate provides the protocol definition (message enums, scopes, error codes)
//! without any runtime dependencies, making it usable by both server and client code.

mod messages;

pub use messages::*;
