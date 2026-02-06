//! # Gateway Database
//!
//! SQLite connection pool and schema management for the AgentZero gateway.
//!
//! Provides `DatabaseManager` with r2d2 connection pooling, WAL mode,
//! and performance pragmas applied to every connection.

mod connection;
pub mod repository;
mod schema;

pub use connection::DatabaseManager;
pub use repository::{ConversationRepository, Message};
