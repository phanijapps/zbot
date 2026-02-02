//! # Gateway Server
//!
//! Main server lifecycle management.

use crate::config::GatewayConfig;
use crate::error::Result;
use crate::events::EventBus;
use crate::http::create_http_router;
use crate::services::{AgentService, RuntimeService};
use crate::state::AppState;
use crate::websocket::WebSocketHandler;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Gateway server managing HTTP and WebSocket endpoints.
pub struct GatewayServer {
    config: GatewayConfig,
    state: AppState,
    ws_handler: Arc<WebSocketHandler>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl GatewayServer {
    /// Create a new gateway server with the given configuration.
    pub fn new(config: GatewayConfig, config_dir: PathBuf) -> Self {
        let state = AppState::new(config_dir);
        let ws_handler = Arc::new(WebSocketHandler::new(
            state.event_bus.clone(),
            state.runtime.clone(),
        ));

        Self {
            config,
            state,
            ws_handler,
            shutdown_tx: None,
        }
    }

    /// Create with default configuration and default data directory.
    pub fn with_defaults() -> Self {
        // Use ~/Documents/agentzero as the default data directory
        let data_dir = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentzero");
        Self::new(GatewayConfig::default(), data_dir)
    }

    /// Get the application state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get the event bus for broadcasting events.
    pub fn event_bus(&self) -> Arc<EventBus> {
        self.state.event_bus.clone()
    }

    /// Get the WebSocket handler.
    pub fn ws_handler(&self) -> Arc<WebSocketHandler> {
        self.ws_handler.clone()
    }

    /// Get the agent service.
    pub fn agents(&self) -> Arc<AgentService> {
        self.state.agents.clone()
    }

    /// Get the runtime service.
    pub fn runtime(&self) -> Arc<RuntimeService> {
        self.state.runtime.clone()
    }

    /// Start the gateway server.
    ///
    /// This spawns both HTTP and WebSocket servers and returns immediately.
    /// Use `shutdown()` to stop the servers.
    pub async fn start(&mut self) -> Result<()> {
        // Mark any RUNNING sessions as CRASHED (daemon was interrupted)
        self.recover_crashed_sessions();

        // Seed default agents and other initial data
        self.state.seed_defaults().await;

        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Start HTTP server
        let http_addr = self.config.http_addr();
        let http_router = create_http_router(self.config.clone(), self.state.clone());
        let http_shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!("Starting HTTP server on {}", http_addr);
            let listener = match tokio::net::TcpListener::bind(&http_addr).await {
                Ok(l) => l,
                Err(e) => {
                    warn!("Failed to bind HTTP server: {}", e);
                    return;
                }
            };

            let server = axum::serve(listener, http_router).with_graceful_shutdown(async move {
                let mut rx = http_shutdown_rx;
                let _ = rx.recv().await;
                info!("HTTP server shutting down");
            });

            if let Err(e) = server.await {
                warn!("HTTP server error: {}", e);
            }
        });

        // Start WebSocket server
        let ws_addr = self.config.ws_addr();
        let ws_handler = self.ws_handler.clone();
        let ws_shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!("Starting WebSocket server on {}", ws_addr);
            if let Err(e) = ws_handler.run(&ws_addr, ws_shutdown_rx).await {
                warn!("WebSocket server error: {}", e);
            }
        });

        info!(
            "Gateway started - HTTP: {}, WebSocket: {}",
            self.config.http_addr(),
            self.config.ws_addr()
        );

        Ok(())
    }

    /// Shutdown the gateway server gracefully.
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
            info!("Gateway shutdown signal sent");
        }
    }

    /// Recover sessions that were interrupted by daemon crash.
    ///
    /// Marks any sessions in RUNNING state as CRASHED so they can be resumed.
    fn recover_crashed_sessions(&self) {
        match self.state.state_service.mark_running_as_crashed() {
            Ok(count) if count > 0 => {
                warn!(
                    "Recovered {} interrupted session(s) - marked as CRASHED",
                    count
                );
            }
            Ok(_) => {
                info!("No interrupted sessions to recover");
            }
            Err(e) => {
                warn!("Failed to recover crashed sessions: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();
        let server = GatewayServer::new(GatewayConfig::default(), config_dir);
        assert!(server.shutdown_tx.is_none());
    }

    #[tokio::test]
    async fn test_custom_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();
        let config = GatewayConfig::with_ports(19000, 19001);
        let server = GatewayServer::new(config, config_dir);
        assert_eq!(server.config.websocket_port, 19000);
        assert_eq!(server.config.http_port, 19001);
    }
}
