// ============================================================================
// DATABASE CONNECTION
// Manages SQLite database connection and initialization
// ============================================================================

use api_logs::DbProvider;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::schema::initialize_database;

/// Database connection manager
pub struct DatabaseManager {
    db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseManager {
    /// Create a new database manager at the specified path
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let db_path = config_dir.join("conversations.db");

        // Ensure the config directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create database directory: {}", e))?;
        }

        // Open/create the database
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // Initialize schema
        initialize_database(&conn)
            .map_err(|e| format!("Failed to initialize database: {}", e))?;

        tracing::info!("Database initialized at {:?}", db_path);

        Ok(Self {
            db_path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Get a reference to the database connection
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    /// Get the database path
    pub fn path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Execute a function with the database connection
    pub fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
    {
        let conn = self.conn.lock()
            .map_err(|e| format!("Failed to acquire database lock: {}", e))?;
        f(&conn).map_err(|e| format!("Database operation failed: {}", e))
    }
}

// ============================================================================
// DB PROVIDER IMPLEMENTATION
// Allows api-logs crate to access the database
// ============================================================================

impl DbProvider for DatabaseManager {
    fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
    {
        DatabaseManager::with_connection(self, f)
    }
}
