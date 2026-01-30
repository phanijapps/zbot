//! # Connection Lifecycle Management
//!
//! Manages MCP server connections with pooling and lifecycle.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

use super::config::McpServerConfig;

/// Connection state for an MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connecting
    Connecting,
    /// Connected and ready
    Connected,
    /// Connection failed
    Failed(String),
}

/// Managed MCP connection with lifecycle tracking.
pub struct McpConnection {
    /// Server configuration
    pub config: McpServerConfig,
    /// Current connection state
    pub state: Arc<RwLock<ConnectionState>>,
    /// Connection attempt count
    pub attempts: Arc<Mutex<u32>>,
    /// Last connected timestamp
    pub connected_at: Arc<Mutex<Option<chrono::DateTime<chrono::Utc>>>>,
}

impl McpConnection {
    /// Create a new managed connection.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            attempts: Arc::new(Mutex::new(0)),
            connected_at: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the current connection state.
    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected)
    }

    /// Set connection state.
    pub async fn set_state(&self, state: ConnectionState) {
        let mut state_guard = self.state.write().await;
        let was_connected = matches!(*state_guard, ConnectionState::Connected);
        *state_guard = state.clone();

        if matches!(state, ConnectionState::Connected) && !was_connected {
            *self.connected_at.lock().await = Some(chrono::Utc::now());
            info!("MCP server '{}' connected", self.config.id);
        }

        debug!("MCP server '{}' state: {:?}", self.config.id, state);
    }

    /// Increment connection attempts.
    pub async fn increment_attempts(&self) -> u32 {
        let mut attempts = self.attempts.lock().await;
        *attempts += 1;
        *attempts
    }

    /// Get connection attempts count.
    pub async fn get_attempts(&self) -> u32 {
        *self.attempts.lock().await
    }

    /// Get connection age if connected.
    pub async fn connection_age(&self) -> Option<chrono::Duration> {
        let connected_at = self.connected_at.lock().await;
        connected_at.map(|t| chrono::Utc::now() - t)
    }

    /// Reset connection state to disconnected.
    pub async fn reset(&self) {
        self.set_state(ConnectionState::Disconnected).await;
        *self.attempts.lock().await = 0;
        *self.connected_at.lock().await = None;
    }
}

/// Connection pool for managing multiple MCP server connections.
pub struct McpConnectionPool {
    /// Managed connections by server ID
    connections: Arc<RwLock<HashMap<String, Arc<McpConnection>>>>,
}

impl McpConnectionPool {
    /// Create a new connection pool.
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a connection for a server.
    pub async fn get(&self, config: &McpServerConfig) -> Arc<McpConnection> {
        let mut connections = self.connections.write().await;

        connections
            .entry(config.id.clone())
            .or_insert_with(|| Arc::new(McpConnection::new(config.clone())))
            .clone()
    }

    /// Get an existing connection without creating.
    pub async fn get_existing(&self, server_id: &str) -> Option<Arc<McpConnection>> {
        let connections = self.connections.read().await;
        connections.get(server_id).cloned()
    }

    /// Remove a connection from the pool.
    pub async fn remove(&self, server_id: &str) -> Option<Arc<McpConnection>> {
        let mut connections = self.connections.write().await;
        connections.remove(server_id)
    }

    /// List all server IDs in the pool.
    pub async fn list_server_ids(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections.keys().cloned().collect()
    }

    /// Get connections in a specific state.
    pub async fn get_by_state(&self, _state: ConnectionState) -> Vec<Arc<McpConnection>> {
        let connections = self.connections.read().await;
        connections
            .values()
            .cloned()
            .collect()
    }

    /// Count connections by state.
    pub async fn count_by_state(&self, target_state: ConnectionState) -> usize {
        let connections = self.connections.read().await;
        let mut count = 0;

        for conn in connections.values() {
            // Skip connections where we can't acquire the read lock
            if conn.state.try_read().map(|s| *s == target_state).unwrap_or(false) {
                count += 1;
            }
        }

        count
    }

    /// Clear all connections.
    pub async fn clear(&self) {
        let mut connections = self.connections.write().await;
        info!("Clearing MCP connection pool ({} servers)", connections.len());
        connections.clear();
    }

    /// Get connection count.
    pub async fn len(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// Check if pool is empty.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for McpConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpServerConfig, McpTransport};

    #[tokio::test]
    async fn test_connection_state_transitions() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let conn = McpConnection::new(config);

        assert_eq!(conn.get_state().await, ConnectionState::Disconnected);
        assert!(!conn.is_connected().await);

        conn.set_state(ConnectionState::Connected).await;
        assert!(conn.is_connected().await);
        assert!(conn.connection_age().await.is_some());
    }

    #[tokio::test]
    async fn test_connection_attempts() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let conn = McpConnection::new(config);

        assert_eq!(conn.get_attempts().await, 0);
        assert_eq!(conn.increment_attempts().await, 1);
        assert_eq!(conn.increment_attempts().await, 2);
    }

    #[tokio::test]
    async fn test_connection_reset() {
        let config = McpServerConfig::stdio("test", "Test", "echo");
        let conn = McpConnection::new(config);

        conn.set_state(ConnectionState::Connected).await;
        let _ = conn.increment_attempts().await;

        conn.reset().await;

        assert_eq!(conn.get_state().await, ConnectionState::Disconnected);
        assert_eq!(conn.get_attempts().await, 0);
        assert!(conn.connection_age().await.is_none());
    }

    #[tokio::test]
    async fn test_pool_get_or_create() {
        let pool = McpConnectionPool::new();
        let config = McpServerConfig::stdio("test", "Test", "echo");

        let conn1 = pool.get(&config).await;
        let conn2 = pool.get(&config).await;

        // Should return the same connection
        assert!(Arc::ptr_eq(&conn1, &conn2));
    }

    #[tokio::test]
    async fn test_pool_operations() {
        let pool = McpConnectionPool::new();
        let config1 = McpServerConfig::stdio("test1", "Test 1", "echo");
        let config2 = McpServerConfig::stdio("test2", "Test 2", "cat");

        pool.get(&config1).await;
        pool.get(&config2).await;

        assert_eq!(pool.len().await, 2);
        assert_eq!(pool.list_server_ids().await.len(), 2);

        pool.remove("test1").await;
        assert_eq!(pool.len().await, 1);
    }

    #[tokio::test]
    async fn test_pool_clear() {
        let pool = McpConnectionPool::new();
        let config1 = McpServerConfig::stdio("test1", "Test 1", "echo");
        let config2 = McpServerConfig::stdio("test2", "Test 2", "cat");

        pool.get(&config1).await;
        pool.get(&config2).await;

        pool.clear().await;
        assert!(pool.is_empty().await);
    }
}
