//! # Subscription Manager
//!
//! Server-side subscription routing for WebSocket events.
//!
//! This module provides conversation-scoped event routing instead of broadcasting
//! all events to all clients. Clients must explicitly subscribe to conversations
//! they want to receive events for.
//!
//! ## Design
//!
//! - Single `RwLock<SubscriptionState>` protects all state (race-condition free)
//! - Sequence numbers assigned atomically with event routing
//! - Dead clients cleaned up on send failure and via background task
//! - Metrics for observability

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::debug;

use super::messages::ServerMessage;

/// Unique client identifier (WebSocket session ID)
pub type ClientId = String;

/// Client connection state
struct Client {
    id: ClientId,
    sender: mpsc::UnboundedSender<ServerMessage>,
    connected_at: Instant,
    last_activity: Instant,
    subscription_count: usize,
    /// Track if channel has failed (for cleanup)
    channel_healthy: bool,
}

/// All subscription state protected by a SINGLE lock
struct SubscriptionState {
    clients: HashMap<ClientId, Client>,
    /// Map from conversation_id -> set of subscribed client_ids
    subscriptions: HashMap<String, HashSet<ClientId>>,
    /// Map from client_id -> set of subscribed conversation_ids
    client_subscriptions: HashMap<ClientId, HashSet<String>>,
    /// Sequence numbers per conversation
    sequence_numbers: HashMap<String, u64>,
}

/// Subscription manager for routing events to subscribed clients.
pub struct SubscriptionManager {
    state: RwLock<SubscriptionState>,
    max_subscriptions_per_client: usize,
    max_subscribers_per_conversation: usize,
    metrics: Arc<SubscriptionMetrics>,
}

/// Metrics for observability
#[derive(Default)]
pub struct SubscriptionMetrics {
    pub total_clients: AtomicU64,
    pub total_subscriptions: AtomicU64,
    pub events_routed: AtomicU64,
    pub events_dropped: AtomicU64,
    pub dead_clients_cleaned: AtomicU64,
}

/// Result of event routing
#[derive(Debug)]
pub struct RoutingResult {
    pub sent: u64,
    pub dropped: u64,
    pub dead_clients: Vec<ClientId>,
}

/// Error when subscribing
#[derive(Debug)]
pub enum SubscribeError {
    ClientNotFound,
    TooManySubscriptions { limit: usize },
    ConversationFull { limit: usize },
}

/// Result of successful subscribe
#[derive(Debug)]
pub enum SubscribeResult {
    Subscribed { current_sequence: u64 },
    AlreadySubscribed { current_sequence: u64 },
}

impl SubscriptionManager {
    const DEFAULT_MAX_SUBS_PER_CLIENT: usize = 50;
    const DEFAULT_MAX_SUBS_PER_CONV: usize = 1000;

    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self {
            state: RwLock::new(SubscriptionState {
                clients: HashMap::new(),
                subscriptions: HashMap::new(),
                client_subscriptions: HashMap::new(),
                sequence_numbers: HashMap::new(),
            }),
            max_subscriptions_per_client: Self::DEFAULT_MAX_SUBS_PER_CLIENT,
            max_subscribers_per_conversation: Self::DEFAULT_MAX_SUBS_PER_CONV,
            metrics: Arc::new(SubscriptionMetrics::default()),
        }
    }

    /// Get metrics for observability.
    pub fn metrics(&self) -> Arc<SubscriptionMetrics> {
        self.metrics.clone()
    }

    /// Register a new client - atomic operation.
    pub async fn connect(
        &self,
        client_id: ClientId,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) {
        let mut state = self.state.write().await;
        state.clients.insert(
            client_id.clone(),
            Client {
                id: client_id.clone(),
                sender,
                connected_at: Instant::now(),
                last_activity: Instant::now(),
                subscription_count: 0,
                channel_healthy: true,
            },
        );
        state
            .client_subscriptions
            .insert(client_id, HashSet::new());
        self.metrics.total_clients.fetch_add(1, Ordering::Relaxed);
    }

    /// Disconnect and cleanup - atomic, no race conditions.
    pub async fn disconnect(&self, client_id: &ClientId) {
        let mut state = self.state.write().await;
        self.disconnect_internal(&mut state, client_id);
    }

    /// Internal disconnect with state already locked.
    fn disconnect_internal(&self, state: &mut SubscriptionState, client_id: &ClientId) {
        if let Some(conversations) = state.client_subscriptions.remove(client_id) {
            for conv_id in &conversations {
                if let Some(subscribers) = state.subscriptions.get_mut(conv_id) {
                    subscribers.remove(client_id);
                    if subscribers.is_empty() {
                        state.subscriptions.remove(conv_id);
                        // Also clean up sequence numbers for empty conversations
                        state.sequence_numbers.remove(conv_id);
                    }
                }
            }
            self.metrics
                .total_subscriptions
                .fetch_sub(conversations.len() as u64, Ordering::Relaxed);
        }
        state.clients.remove(client_id);
        self.metrics.total_clients.fetch_sub(1, Ordering::Relaxed);
    }

    /// Subscribe to a conversation - atomic with limit checks.
    pub async fn subscribe(
        &self,
        client_id: &ClientId,
        conversation_id: String,
    ) -> Result<SubscribeResult, SubscribeError> {
        let mut state = self.state.write().await;

        // Check client exists and get subscription count
        let subscription_count = state
            .clients
            .get(client_id)
            .map(|c| c.subscription_count)
            .ok_or(SubscribeError::ClientNotFound)?;

        if subscription_count >= self.max_subscriptions_per_client {
            return Err(SubscribeError::TooManySubscriptions {
                limit: self.max_subscriptions_per_client,
            });
        }

        // Check if already subscribed
        let already_subscribed = state
            .subscriptions
            .get(&conversation_id)
            .map(|s| s.contains(client_id))
            .unwrap_or(false);

        if already_subscribed {
            let current_seq = *state.sequence_numbers.get(&conversation_id).unwrap_or(&0);
            return Ok(SubscribeResult::AlreadySubscribed { current_sequence: current_seq });
        }

        // Check conversation subscriber limit
        let subscriber_count = state
            .subscriptions
            .get(&conversation_id)
            .map(|s| s.len())
            .unwrap_or(0);

        if subscriber_count >= self.max_subscribers_per_conversation {
            return Err(SubscribeError::ConversationFull {
                limit: self.max_subscribers_per_conversation,
            });
        }

        // All checks passed, now do the updates
        state
            .subscriptions
            .entry(conversation_id.clone())
            .or_insert_with(HashSet::new)
            .insert(client_id.clone());

        state
            .client_subscriptions
            .get_mut(client_id)
            .unwrap()
            .insert(conversation_id.clone());

        if let Some(client) = state.clients.get_mut(client_id) {
            client.subscription_count += 1;
        }

        let current_seq = *state.sequence_numbers.entry(conversation_id).or_insert(0);

        self.metrics
            .total_subscriptions
            .fetch_add(1, Ordering::Relaxed);

        Ok(SubscribeResult::Subscribed { current_sequence: current_seq })
    }

    /// Unsubscribe from a conversation - atomic.
    pub async fn unsubscribe(&self, client_id: &ClientId, conversation_id: &str) {
        let mut state = self.state.write().await;

        // Check if client was actually subscribed
        let was_subscribed = state
            .subscriptions
            .get_mut(conversation_id)
            .map(|s| s.remove(client_id))
            .unwrap_or(false);

        if !was_subscribed {
            return;
        }

        self.metrics
            .total_subscriptions
            .fetch_sub(1, Ordering::Relaxed);

        // Update client subscription count
        if let Some(client) = state.clients.get_mut(client_id) {
            client.subscription_count = client.subscription_count.saturating_sub(1);
        }

        // Check if conversation is now empty
        let is_empty = state
            .subscriptions
            .get(conversation_id)
            .map(|s| s.is_empty())
            .unwrap_or(false);

        if is_empty {
            state.subscriptions.remove(conversation_id);
            state.sequence_numbers.remove(conversation_id);
        }

        // Remove from client's subscription list
        if let Some(client_subs) = state.client_subscriptions.get_mut(client_id) {
            client_subs.remove(conversation_id);
        }
    }

    /// Route event to subscribed clients with atomic sequence assignment.
    ///
    /// Returns routing result with counts and dead clients.
    pub async fn route_event(
        &self,
        conversation_id: &str,
        message: ServerMessage,
    ) -> RoutingResult {
        let mut state = self.state.write().await;

        // Assign sequence number atomically
        let seq = state
            .sequence_numbers
            .entry(conversation_id.to_string())
            .or_insert(0);
        *seq += 1;
        let current_seq = *seq;

        // Add sequence to message
        let message_with_seq = message.with_sequence(current_seq);

        // Collect subscriber IDs first to avoid borrowing conflicts
        let subscriber_ids: Vec<ClientId> = state
            .subscriptions
            .get(conversation_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();

        if subscriber_ids.is_empty() {
            return RoutingResult {
                sent: 0,
                dropped: 0,
                dead_clients: vec![],
            };
        }

        let mut sent = 0u64;
        let mut dropped = 0u64;
        let mut dead_clients = Vec::new();

        for client_id in subscriber_ids {
            if let Some(client) = state.clients.get_mut(&client_id) {
                match client.sender.send(message_with_seq.clone()) {
                    Ok(()) => {
                        sent += 1;
                        client.last_activity = Instant::now();
                    }
                    Err(_) => {
                        dropped += 1;
                        // Channel closed - mark for cleanup
                        client.channel_healthy = false;
                        dead_clients.push(client_id);
                    }
                }
            }
        }

        // Clean up dead clients while we still hold the lock
        for dead_id in &dead_clients {
            self.disconnect_internal(&mut state, dead_id);
            self.metrics
                .dead_clients_cleaned
                .fetch_add(1, Ordering::Relaxed);
        }

        self.metrics
            .events_routed
            .fetch_add(sent, Ordering::Relaxed);
        self.metrics
            .events_dropped
            .fetch_add(dropped, Ordering::Relaxed);

        RoutingResult {
            sent,
            dropped,
            dead_clients,
        }
    }

    /// Broadcast global event to all clients (for stats, notifications).
    pub async fn broadcast_global(&self, message: ServerMessage) {
        let state = self.state.read().await;

        for client in state.clients.values() {
            if client.channel_healthy {
                let _ = client.sender.send(message.clone());
            }
        }
    }

    /// Send message to a specific client.
    pub async fn send_to_client(&self, client_id: &ClientId, message: ServerMessage) -> bool {
        let state = self.state.read().await;

        if let Some(client) = state.clients.get(client_id) {
            if client.channel_healthy {
                return client.sender.send(message).is_ok();
            }
        }
        false
    }

    /// Background cleanup task - call periodically (e.g., every 30s).
    pub async fn cleanup_stale_clients(&self, timeout: Duration) -> usize {
        let mut state = self.state.write().await;
        let now = Instant::now();

        let stale: Vec<ClientId> = state
            .clients
            .iter()
            .filter(|(_, c)| !c.channel_healthy || now.duration_since(c.last_activity) > timeout)
            .map(|(id, _)| id.clone())
            .collect();

        let count = stale.len();
        for client_id in stale {
            debug!(client_id = %client_id, "Cleaning up stale client");
            self.disconnect_internal(&mut state, &client_id);
        }

        self.metrics
            .dead_clients_cleaned
            .fetch_add(count as u64, Ordering::Relaxed);
        count
    }

    /// Get the number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.state.read().await.clients.len()
    }

    /// Get the number of active subscriptions.
    pub async fn subscription_count(&self) -> usize {
        self.state
            .read()
            .await
            .subscriptions
            .values()
            .map(|s| s.len())
            .sum()
    }

    /// Check if a client is subscribed to a conversation.
    pub async fn is_subscribed(&self, client_id: &ClientId, conversation_id: &str) -> bool {
        let state = self.state.read().await;
        state
            .subscriptions
            .get(conversation_id)
            .map(|s| s.contains(client_id))
            .unwrap_or(false)
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sender() -> (
        mpsc::UnboundedSender<ServerMessage>,
        mpsc::UnboundedReceiver<ServerMessage>,
    ) {
        mpsc::unbounded_channel()
    }

    #[tokio::test]
    async fn test_connect_disconnect() {
        let manager = SubscriptionManager::new();
        let (tx, _rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;
        assert_eq!(manager.client_count().await, 1);

        manager.disconnect(&"client-1".to_string()).await;
        assert_eq!(manager.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_subscribe_unsubscribe() {
        let manager = SubscriptionManager::new();
        let (tx, _rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;

        // Subscribe
        let result = manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await;
        assert!(matches!(result, Ok(SubscribeResult::Subscribed { current_sequence: 0 })));
        assert!(manager.is_subscribed(&"client-1".to_string(), "conv-1").await);

        // Double subscribe returns AlreadySubscribed
        let result = manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await;
        assert!(matches!(result, Ok(SubscribeResult::AlreadySubscribed { .. })));

        // Unsubscribe
        manager
            .unsubscribe(&"client-1".to_string(), "conv-1")
            .await;
        assert!(!manager.is_subscribed(&"client-1".to_string(), "conv-1").await);
    }

    #[tokio::test]
    async fn test_subscribe_limit() {
        let manager = SubscriptionManager::new();
        let (tx, _rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;

        // Subscribe up to limit
        for i in 0..SubscriptionManager::DEFAULT_MAX_SUBS_PER_CLIENT {
            let result = manager
                .subscribe(&"client-1".to_string(), format!("conv-{}", i))
                .await;
            assert!(result.is_ok());
        }

        // Next subscription should fail
        let result = manager
            .subscribe(
                &"client-1".to_string(),
                "conv-overflow".to_string(),
            )
            .await;
        assert!(matches!(result, Err(SubscribeError::TooManySubscriptions { .. })));
    }

    #[tokio::test]
    async fn test_route_event() {
        let manager = SubscriptionManager::new();
        let (tx, mut rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;
        manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await
            .unwrap();

        // Route event
        let msg = ServerMessage::Token {
            session_id: "sess-1".to_string(),
            execution_id: "exec-1".to_string(),
            conversation_id: Some("conv-1".to_string()),
            delta: "Hello".to_string(),
            seq: None,
        };
        let result = manager.route_event("conv-1", msg).await;

        assert_eq!(result.sent, 1);
        assert_eq!(result.dropped, 0);
        assert!(result.dead_clients.is_empty());

        // Check received message has sequence
        let received = rx.recv().await.unwrap();
        if let ServerMessage::Token { seq, .. } = received {
            assert_eq!(seq, Some(1));
        } else {
            panic!("Wrong message type");
        }
    }

    #[tokio::test]
    async fn test_route_event_sequence_increments() {
        let manager = SubscriptionManager::new();
        let (tx, mut rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;
        manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await
            .unwrap();

        // Route multiple events
        for _ in 0..3 {
            let msg = ServerMessage::Token {
                session_id: "sess-1".to_string(),
                execution_id: "exec-1".to_string(),
                conversation_id: Some("conv-1".to_string()),
                delta: "x".to_string(),
                seq: None,
            };
            manager.route_event("conv-1", msg).await;
        }

        // Check sequences
        for expected_seq in 1..=3 {
            let received = rx.recv().await.unwrap();
            if let ServerMessage::Token { seq, .. } = received {
                assert_eq!(seq, Some(expected_seq));
            }
        }
    }

    #[tokio::test]
    async fn test_dead_client_cleanup() {
        let manager = SubscriptionManager::new();
        let (tx, rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;
        manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await
            .unwrap();

        // Drop receiver to simulate dead client
        drop(rx);

        // Route event - should detect dead client
        let msg = ServerMessage::Token {
            session_id: "sess-1".to_string(),
            execution_id: "exec-1".to_string(),
            conversation_id: Some("conv-1".to_string()),
            delta: "Hello".to_string(),
            seq: None,
        };
        let result = manager.route_event("conv-1", msg).await;

        assert_eq!(result.sent, 0);
        assert_eq!(result.dropped, 1);
        assert_eq!(result.dead_clients.len(), 1);

        // Client should be cleaned up
        assert_eq!(manager.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_disconnect_cleans_subscriptions() {
        let manager = SubscriptionManager::new();
        let (tx, _rx) = create_test_sender();

        manager.connect("client-1".to_string(), tx).await;
        manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await
            .unwrap();
        manager
            .subscribe(&"client-1".to_string(), "conv-2".to_string())
            .await
            .unwrap();

        assert_eq!(manager.subscription_count().await, 2);

        // Disconnect should clean up all subscriptions
        manager.disconnect(&"client-1".to_string()).await;

        assert_eq!(manager.subscription_count().await, 0);
        assert_eq!(manager.client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_global() {
        let manager = SubscriptionManager::new();
        let (tx1, mut rx1) = create_test_sender();
        let (tx2, mut rx2) = create_test_sender();

        manager.connect("client-1".to_string(), tx1).await;
        manager.connect("client-2".to_string(), tx2).await;

        // Broadcast (no subscription needed)
        let msg = ServerMessage::Pong;
        manager.broadcast_global(msg).await;

        // Both clients should receive
        assert!(rx1.recv().await.is_some());
        assert!(rx2.recv().await.is_some());
    }

    #[tokio::test]
    async fn test_conversation_isolation() {
        let manager = SubscriptionManager::new();
        let (tx1, mut rx1) = create_test_sender();
        let (tx2, mut rx2) = create_test_sender();

        manager.connect("client-1".to_string(), tx1).await;
        manager.connect("client-2".to_string(), tx2).await;

        // Client 1 subscribes to conv-1
        manager
            .subscribe(&"client-1".to_string(), "conv-1".to_string())
            .await
            .unwrap();

        // Client 2 subscribes to conv-2
        manager
            .subscribe(&"client-2".to_string(), "conv-2".to_string())
            .await
            .unwrap();

        // Route event to conv-1
        let msg = ServerMessage::Token {
            session_id: "sess-1".to_string(),
            execution_id: "exec-1".to_string(),
            conversation_id: Some("conv-1".to_string()),
            delta: "Hello".to_string(),
            seq: None,
        };
        manager.route_event("conv-1", msg).await;

        // Only client-1 should receive
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());
    }
}
