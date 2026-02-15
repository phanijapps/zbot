// ============================================================================
// DATABASE CONNECTION
// Manages SQLite connection pool and initialization
// ============================================================================

use api_logs::DbProvider;
use execution_state::StateDbProvider;
use gateway_services::SharedVaultPaths;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::time::Duration;

use crate::schema::initialize_database;

/// Database connection pool manager.
///
/// Uses r2d2 connection pool instead of a single Mutex<Connection>.
/// Each `with_connection()` call borrows a connection from the pool,
/// allowing concurrent reads (WAL mode) and reducing lock contention.
pub struct DatabaseManager {
    pool: Pool<SqliteConnectionManager>,
}

/// r2d2 connection customizer that applies pragmas to every new connection.
#[derive(Debug)]
struct PragmaCustomizer;

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for PragmaCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        // Performance pragmas applied to each connection in the pool.
        // WAL mode is database-wide (set once), but these per-connection
        // settings ensure every pooled connection is properly configured.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA busy_timeout = 5000;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA temp_store = MEMORY;
             PRAGMA foreign_keys = ON;"
        )?;
        Ok(())
    }
}

impl DatabaseManager {
    /// Create a new database manager with a connection pool.
    ///
    /// Opens a pool of up to 8 SQLite connections with WAL mode enabled.
    /// Schema is initialized on the first connection.
    pub fn new(paths: SharedVaultPaths) -> Result<Self, String> {
        let db_path = paths.conversations_db();

        // Ensure the data directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create database directory: {e}"))?;
        }

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .connection_timeout(Duration::from_secs(5))
            .connection_customizer(Box::new(PragmaCustomizer))
            .build(manager)
            .map_err(|e| format!("Failed to create connection pool: {e}"))?;

        // Initialize schema on a connection from the pool
        {
            let conn = pool
                .get()
                .map_err(|e| format!("Failed to get connection for schema init: {e}"))?;
            initialize_database(&conn)
                .map_err(|e| format!("Failed to initialize database: {e}"))?;
        }

        tracing::info!(
            "Database pool initialized at {:?} (WAL mode, pool_size=8)",
            db_path
        );

        Ok(Self { pool })
    }

    /// Execute a function with a database connection from the pool.
    ///
    /// Borrows a connection for the duration of `f`, then returns it to the pool.
    /// Multiple concurrent callers each get their own connection (up to pool max).
    pub fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
    {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Failed to get connection from pool: {e}"))?;
        f(&conn).map_err(|e| format!("Database operation failed: {e}"))
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

// ============================================================================
// STATE DB PROVIDER IMPLEMENTATION
// Allows execution-state crate to access the database
// ============================================================================

impl StateDbProvider for DatabaseManager {
    fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
    {
        DatabaseManager::with_connection(self, f)
    }
}
