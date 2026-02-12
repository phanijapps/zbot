//! # Pending Requests
//!
//! Request/response correlation for bidirectional worker queries.
//!
//! When the server sends a ResourceQuery or CapabilityInvoke to a worker,
//! a oneshot channel is registered here. When the worker responds, the
//! matching channel is resolved.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// Manages pending request/response correlations.
pub struct PendingRequests {
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>,
}

impl PendingRequests {
    /// Create a new pending requests tracker.
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Register a pending request and return the receiver.
    pub fn register(&self, request_id: String) -> oneshot::Receiver<Result<Value, String>> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap();
        pending.insert(request_id, tx);
        rx
    }

    /// Resolve a pending request with a successful result.
    pub fn resolve(&self, request_id: &str, data: Value) -> bool {
        let mut pending = self.pending.lock().unwrap();
        if let Some(tx) = pending.remove(request_id) {
            let _ = tx.send(Ok(data));
            true
        } else {
            false
        }
    }

    /// Reject a pending request with an error.
    pub fn reject(&self, request_id: &str, error: String) -> bool {
        let mut pending = self.pending.lock().unwrap();
        if let Some(tx) = pending.remove(request_id) {
            let _ = tx.send(Err(error));
            true
        } else {
            false
        }
    }

    /// Cancel all pending requests (on disconnect).
    pub fn cancel_all(&self) {
        let mut pending = self.pending.lock().unwrap();
        for (id, tx) in pending.drain() {
            let _ = tx.send(Err(format!("Worker disconnected (request {})", id)));
        }
    }

    /// Number of pending requests.
    pub fn len(&self) -> usize {
        self.pending.lock().unwrap().len()
    }

    /// Whether there are no pending requests.
    pub fn is_empty(&self) -> bool {
        self.pending.lock().unwrap().is_empty()
    }
}

impl Default for PendingRequests {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_resolve() {
        let pending = PendingRequests::new();
        let rx = pending.register("req-1".to_string());

        assert_eq!(pending.len(), 1);
        assert!(pending.resolve("req-1", serde_json::json!({"ok": true})));
        assert_eq!(pending.len(), 0);

        let result = rx.await.unwrap();
        assert_eq!(result.unwrap()["ok"], true);
    }

    #[tokio::test]
    async fn test_register_and_reject() {
        let pending = PendingRequests::new();
        let rx = pending.register("req-1".to_string());

        assert!(pending.reject("req-1", "Not found".to_string()));

        let result = rx.await.unwrap();
        assert_eq!(result.unwrap_err(), "Not found");
    }

    #[tokio::test]
    async fn test_resolve_unknown() {
        let pending = PendingRequests::new();
        assert!(!pending.resolve("unknown", serde_json::json!(null)));
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let pending = PendingRequests::new();
        let rx1 = pending.register("req-1".to_string());
        let rx2 = pending.register("req-2".to_string());

        assert_eq!(pending.len(), 2);
        pending.cancel_all();
        assert_eq!(pending.len(), 0);

        // Both should receive errors
        assert!(rx1.await.unwrap().is_err());
        assert!(rx2.await.unwrap().is_err());
    }

    #[test]
    fn test_is_empty() {
        let pending = PendingRequests::new();
        assert!(pending.is_empty());
    }
}
