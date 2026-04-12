//! `KnowledgeDatabase` — r2d2 pool for `knowledge.db` with sqlite-vec
//! extension auto-loaded on every connection.

use gateway_services::SharedVaultPaths;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::time::Duration;

use crate::knowledge_schema::{initialize_knowledge_database, initialize_vec_tables};
use crate::sqlite_vec_loader::load_sqlite_vec;

/// Connection pool for `knowledge.db`.
///
/// Every connection has sqlite-vec loaded and WAL-mode pragmas applied.
/// Schema v22 is initialized on first construction.
pub struct KnowledgeDatabase {
    pool: Pool<SqliteConnectionManager>,
}

/// Customizer: applies WAL pragmas and loads sqlite-vec on every acquired connection.
#[derive(Debug)]
struct KnowledgeConnectionCustomizer;

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for KnowledgeConnectionCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;
             PRAGMA busy_timeout = 5000;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA temp_store = MEMORY;
             PRAGMA foreign_keys = ON;",
        )?;
        load_sqlite_vec(conn)?;
        Ok(())
    }
}

impl KnowledgeDatabase {
    /// Create a new knowledge database manager.
    ///
    /// Creates the data directory if it doesn't exist, builds an r2d2 pool
    /// of up to 8 connections (each with sqlite-vec loaded), and initializes
    /// the v22 schema + vec0 tables. Idempotent — safe to call on an
    /// already-initialized DB.
    pub fn new(paths: SharedVaultPaths) -> Result<Self, String> {
        let db_path = paths.knowledge_db();

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create data dir: {e}"))?;
        }

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .connection_timeout(Duration::from_secs(5))
            .connection_customizer(Box::new(KnowledgeConnectionCustomizer))
            .build(manager)
            .map_err(|e| format!("Failed to create knowledge pool: {e}"))?;

        // Initialize schema + vec tables on a single connection from the pool.
        {
            let conn = pool
                .get()
                .map_err(|e| format!("Failed to get init connection: {e}"))?;
            initialize_knowledge_database(&conn)
                .map_err(|e| format!("Failed to init knowledge schema: {e}"))?;
            initialize_vec_tables(&conn).map_err(|e| format!("Failed to init vec tables: {e}"))?;
        }

        tracing::info!("Knowledge database initialized at {:?}", db_path);

        Ok(Self { pool })
    }

    /// Borrow a connection from the pool and run `f`.
    pub fn with_connection<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
    {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("Failed to get knowledge connection: {e}"))?;
        f(&conn).map_err(|e| format!("Knowledge DB operation failed: {e}"))
    }
}
