//! # Bridge Registry
//!
//! In-memory registry of connected bridge workers.

use crate::error::BridgeError;
use crate::protocol::{BridgeServerMessage, WorkerCapability, WorkerResource};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Entry for a connected worker in the registry.
pub struct WorkerEntry {
    /// Worker/adapter identifier.
    pub adapter_id: String,
    /// Capabilities declared by the worker.
    pub capabilities: Vec<WorkerCapability>,
    /// Resources declared by the worker.
    pub resources: Vec<WorkerResource>,
    /// When the worker connected.
    pub connected_at: DateTime<Utc>,
    /// Channel to send messages to this worker's WS session.
    pub tx: mpsc::UnboundedSender<BridgeServerMessage>,
    /// Pending request/response correlation map.
    pub pending_requests: Arc<crate::pending_requests::PendingRequests>,
}

/// In-memory registry of connected bridge workers.
#[derive(Clone)]
pub struct BridgeRegistry {
    workers: Arc<RwLock<HashMap<String, WorkerEntry>>>,
}

impl BridgeRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a connected worker.
    ///
    /// Returns an error if a worker with the same adapter_id is already connected.
    pub async fn register(
        &self,
        adapter_id: String,
        capabilities: Vec<WorkerCapability>,
        resources: Vec<WorkerResource>,
        tx: mpsc::UnboundedSender<BridgeServerMessage>,
        pending_requests: Arc<crate::pending_requests::PendingRequests>,
    ) -> Result<(), BridgeError> {
        let mut workers = self.workers.write().await;
        if workers.contains_key(&adapter_id) {
            return Err(BridgeError::AlreadyConnected(adapter_id));
        }
        workers.insert(
            adapter_id.clone(),
            WorkerEntry {
                adapter_id,
                capabilities,
                resources,
                connected_at: Utc::now(),
                tx,
                pending_requests,
            },
        );
        Ok(())
    }

    /// Unregister a worker (on disconnect).
    pub async fn unregister(&self, adapter_id: &str) -> Option<WorkerEntry> {
        let mut workers = self.workers.write().await;
        workers.remove(adapter_id)
    }

    /// Check if a worker is connected.
    pub async fn is_connected(&self, adapter_id: &str) -> bool {
        let workers = self.workers.read().await;
        workers.contains_key(adapter_id)
    }

    /// Send a message to a connected worker.
    pub async fn send(
        &self,
        adapter_id: &str,
        msg: BridgeServerMessage,
    ) -> Result<(), BridgeError> {
        let workers = self.workers.read().await;
        let entry = workers
            .get(adapter_id)
            .ok_or_else(|| BridgeError::NotConnected(adapter_id.to_string()))?;
        entry
            .tx
            .send(msg)
            .map_err(|_| BridgeError::Channel(format!("Send failed for {}", adapter_id)))
    }

    /// Get the pending requests handle for a worker.
    pub async fn pending_requests(
        &self,
        adapter_id: &str,
    ) -> Result<Arc<crate::pending_requests::PendingRequests>, BridgeError> {
        let workers = self.workers.read().await;
        let entry = workers
            .get(adapter_id)
            .ok_or_else(|| BridgeError::NotConnected(adapter_id.to_string()))?;
        Ok(entry.pending_requests.clone())
    }

    /// List all connected worker adapter IDs.
    pub async fn list(&self) -> Vec<String> {
        let workers = self.workers.read().await;
        workers.keys().cloned().collect()
    }

    /// List all connected workers with their capabilities and resources.
    pub async fn list_entries(&self) -> Vec<WorkerSummary> {
        let workers = self.workers.read().await;
        workers
            .values()
            .map(|e| WorkerSummary {
                adapter_id: e.adapter_id.clone(),
                capabilities: e.capabilities.clone(),
                resources: e.resources.clone(),
                connected_at: e.connected_at,
            })
            .collect()
    }

    /// Disconnect all workers (for shutdown).
    pub async fn disconnect_all(&self) {
        let mut workers = self.workers.write().await;
        for (id, entry) in workers.drain() {
            entry.pending_requests.cancel_all();
            tracing::info!(adapter_id = %id, "Disconnecting bridge worker");
        }
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a connected worker (without the channel).
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkerSummary {
    pub adapter_id: String,
    pub capabilities: Vec<WorkerCapability>,
    pub resources: Vec<WorkerResource>,
    pub connected_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tx() -> (
        mpsc::UnboundedSender<BridgeServerMessage>,
        mpsc::UnboundedReceiver<BridgeServerMessage>,
    ) {
        mpsc::unbounded_channel()
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let registry = BridgeRegistry::new();
        let (tx, _rx) = make_tx();
        let pending = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register(
                "worker-1".to_string(),
                vec![WorkerCapability {
                    name: "send".to_string(),
                    description: None,
                    schema: None,
                }],
                vec![],
                tx,
                pending,
            )
            .await
            .unwrap();

        assert!(registry.is_connected("worker-1").await);
        assert!(!registry.is_connected("worker-2").await);

        let list = registry.list().await;
        assert_eq!(list, vec!["worker-1"]);
    }

    #[tokio::test]
    async fn test_duplicate_rejection() {
        let registry = BridgeRegistry::new();
        let (tx1, _rx1) = make_tx();
        let (tx2, _rx2) = make_tx();
        let pending1 = Arc::new(crate::pending_requests::PendingRequests::new());
        let pending2 = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register("worker-1".to_string(), vec![], vec![], tx1, pending1)
            .await
            .unwrap();

        let result = registry
            .register("worker-1".to_string(), vec![], vec![], tx2, pending2)
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BridgeError::AlreadyConnected(_)
        ));
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = BridgeRegistry::new();
        let (tx, _rx) = make_tx();
        let pending = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register("worker-1".to_string(), vec![], vec![], tx, pending)
            .await
            .unwrap();
        assert!(registry.is_connected("worker-1").await);

        let entry = registry.unregister("worker-1").await;
        assert!(entry.is_some());
        assert!(!registry.is_connected("worker-1").await);
    }

    #[tokio::test]
    async fn test_send_to_connected() {
        let registry = BridgeRegistry::new();
        let (tx, mut rx) = make_tx();
        let pending = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register("worker-1".to_string(), vec![], vec![], tx, pending)
            .await
            .unwrap();

        registry
            .send("worker-1", BridgeServerMessage::Ping)
            .await
            .unwrap();

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, BridgeServerMessage::Ping));
    }

    #[tokio::test]
    async fn test_send_to_disconnected() {
        let registry = BridgeRegistry::new();
        let result = registry.send("worker-1", BridgeServerMessage::Ping).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BridgeError::NotConnected(_)));
    }

    #[tokio::test]
    async fn test_list_entries() {
        let registry = BridgeRegistry::new();
        let (tx, _rx) = make_tx();
        let pending = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register(
                "crm-worker".to_string(),
                vec![WorkerCapability {
                    name: "send_email".to_string(),
                    description: Some("Send email".to_string()),
                    schema: None,
                }],
                vec![WorkerResource {
                    name: "contacts".to_string(),
                    description: Some("CRM contacts".to_string()),
                }],
                tx,
                pending,
            )
            .await
            .unwrap();

        let entries = registry.list_entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].adapter_id, "crm-worker");
        assert_eq!(entries[0].capabilities.len(), 1);
        assert_eq!(entries[0].resources.len(), 1);
    }

    #[tokio::test]
    async fn test_disconnect_all() {
        let registry = BridgeRegistry::new();
        let (tx1, _rx1) = make_tx();
        let (tx2, _rx2) = make_tx();
        let p1 = Arc::new(crate::pending_requests::PendingRequests::new());
        let p2 = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register("w1".to_string(), vec![], vec![], tx1, p1)
            .await
            .unwrap();
        registry
            .register("w2".to_string(), vec![], vec![], tx2, p2)
            .await
            .unwrap();

        assert_eq!(registry.list().await.len(), 2);
        registry.disconnect_all().await;
        assert!(registry.list().await.is_empty());
    }

    #[tokio::test]
    async fn test_register_after_unregister() {
        let registry = BridgeRegistry::new();
        let (tx1, _rx1) = make_tx();
        let (tx2, _rx2) = make_tx();
        let p1 = Arc::new(crate::pending_requests::PendingRequests::new());
        let p2 = Arc::new(crate::pending_requests::PendingRequests::new());

        registry
            .register("worker-1".to_string(), vec![], vec![], tx1, p1)
            .await
            .unwrap();
        registry.unregister("worker-1").await;

        // Should succeed after unregister
        registry
            .register("worker-1".to_string(), vec![], vec![], tx2, p2)
            .await
            .unwrap();
        assert!(registry.is_connected("worker-1").await);
    }
}
