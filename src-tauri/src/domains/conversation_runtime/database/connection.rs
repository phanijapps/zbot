// ============================================================================
// DATABASE CONNECTION
// Manages SQLite database connection and initialization
// ============================================================================

use rusqlite::{Connection, Transaction};
use std::path::PathBuf;
use std::sync::{Mutex, Arc};

use crate::settings::AppDirs;
use super::schema::initialize_database;

/// Database connection manager
pub struct DatabaseManager {
    _db_path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseManager {
    /// Create a new database manager
    pub fn new() -> Result<Self, String> {
        let dirs = AppDirs::get().map_err(|e| e.to_string())?;
        let db_path = dirs.config_dir.join("conversations.db");

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

        Ok(Self {
            _db_path: db_path,
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Get a reference to the database connection
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    /// Get the database path (for debugging/info)
    pub fn path(&self) -> &PathBuf {
        &self._db_path
    }

    /// Execute a transaction with the database connection
    pub fn transaction<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Transaction) -> Result<R, rusqlite::Error>,
    {
        let mut conn = self.conn.lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        let tx = conn.transaction().map_err(|e| format!("Failed to start transaction: {}", e))?;
        let result = match f(&tx) {
            Ok(r) => r,
            Err(e) => return Err(format!("Transaction failed: {}", e)),
        };
        tx.commit().map_err(|e| format!("Failed to commit transaction: {}", e))?;
        Ok(result)
    }
}

/// Global database manager instance
static DB_MANAGER: Mutex<Option<Arc<DatabaseManager>>> = Mutex::new(None);

/// Initialize the global database manager
pub fn init_database() -> Result<(), String> {
    let manager = DatabaseManager::new()?;
    let mut global = DB_MANAGER.lock()
        .map_err(|e| format!("Failed to acquire lock: {}", e))?;

    *global = Some(Arc::new(manager));
    Ok(())
}

/// Get the global database manager
pub fn get_database() -> Result<Arc<DatabaseManager>, String> {
    let global = DB_MANAGER.lock()
        .map_err(|e| format!("Failed to acquire lock: {}", e))?;

    global.as_ref()
        .cloned()
        .ok_or_else(|| "Database not initialized".to_string())
}
