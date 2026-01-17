// ============================================================================
// CONVERSATION RUNTIME DOMAIN
// Manages conversations, messages, and memory
// ============================================================================

pub mod database;
pub mod repository;
pub mod memory;

pub use database::{
    init_database,
    get_database,
};
