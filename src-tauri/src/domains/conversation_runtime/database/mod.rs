// ============================================================================
// DATABASE MODULE
// SQLite database management for conversations
// ============================================================================

pub mod schema;
pub mod connection;

pub use connection::{
    init_database,
    get_database,
};
