// ============================================================================
// DATABASE MODULE
// SQLite database management for conversations and agent channels
// ============================================================================

pub mod schema;
pub mod schema_v2;
pub mod connection;

pub use connection::{
    init_database,
    get_database,
};

pub use schema_v2::{
    initialize_database_v2,
    get_schema_version_v2,
    needs_migration_v2,
};
