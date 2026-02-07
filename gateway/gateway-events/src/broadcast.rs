//! # Event Broadcast
//!
//! Broadcast mechanism for distributing events to subscribers.

use crate::GatewayEvent;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, trace};

/// Event bus for broadcasting events to connected clients.
pub struct EventBus {
    /// Global event channel.
    global_tx: broadcast::Sender<GatewayEvent>,

    /// Per-agent event channels.
    agent_channels: RwLock<HashMap<String, broadcast::Sender<GatewayEvent>>>,

    /// Session-specific channels for continuation events.
    session_channels: RwLock<HashMap<String, broadcast::Sender<GatewayEvent>>>,

    /// Channel capacity.
    capacity: usize,
}

impl EventBus {
    /// Create a new event bus.
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    /// Create with custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (global_tx, _) = broadcast::channel(capacity);
        Self {
            global_tx,
            agent_channels: RwLock::new(HashMap::new()),
            session_channels: RwLock::new(HashMap::new()),
            capacity,
        }
    }

    /// Publish an event synchronously to the global broadcast channel.
    ///
    /// Preserves insertion order when called sequentially from the same thread.
    /// Does NOT send to agent-specific channels (use `publish()` for that).
    pub fn publish_sync(&self, event: GatewayEvent) {
        let _ = self.global_tx.send(event);
    }

    /// Publish an event.
    pub async fn publish(&self, event: GatewayEvent) {
        let agent_id = event.agent_id().map(String::from).unwrap_or_default();
        trace!("Publishing event for agent {}: {:?}", agent_id, event);

        // Send to global channel
        let _ = self.global_tx.send(event.clone());

        // Send to agent-specific channel if exists
        if !agent_id.is_empty() {
            let channels = self.agent_channels.read().await;
            if let Some(tx) = channels.get(&agent_id) {
                let _ = tx.send(event);
            }
        }
    }

    /// Subscribe to all events.
    pub fn subscribe_all(&self) -> broadcast::Receiver<GatewayEvent> {
        self.global_tx.subscribe()
    }

    /// Subscribe to events for a specific agent.
    pub async fn subscribe_agent(&self, agent_id: &str) -> broadcast::Receiver<GatewayEvent> {
        let mut channels = self.agent_channels.write().await;

        if let Some(tx) = channels.get(agent_id) {
            return tx.subscribe();
        }

        // Create new channel for this agent
        let (tx, rx) = broadcast::channel(self.capacity);
        channels.insert(agent_id.to_string(), tx);
        rx
    }

    /// Unsubscribe from agent events (cleanup when no more subscribers).
    pub async fn cleanup_agent(&self, agent_id: &str) {
        let mut channels = self.agent_channels.write().await;

        // Only remove if no receivers
        if let Some(tx) = channels.get(agent_id) {
            if tx.receiver_count() == 0 {
                channels.remove(agent_id);
                debug!("Cleaned up event channel for agent {}", agent_id);
            }
        }
    }

    /// Get count of active agent channels.
    pub async fn agent_channel_count(&self) -> usize {
        self.agent_channels.read().await.len()
    }

    /// Subscribe to events for a specific session.
    pub async fn subscribe_session(&self, session_id: &str) -> broadcast::Receiver<GatewayEvent> {
        let mut channels = self.session_channels.write().await;

        if let Some(tx) = channels.get(session_id) {
            return tx.subscribe();
        }

        // Create new channel for this session
        let (tx, rx) = broadcast::channel(self.capacity);
        channels.insert(session_id.to_string(), tx);
        rx
    }

    /// Publish event to session-specific channel.
    pub async fn publish_session(&self, session_id: &str, event: GatewayEvent) {
        // Also publish to global channel
        let _ = self.global_tx.send(event.clone());

        // Send to session-specific channel if exists
        let channels = self.session_channels.read().await;
        if let Some(tx) = channels.get(session_id) {
            let _ = tx.send(event);
        }
    }

    /// Clean up session channel when session completes.
    pub async fn remove_session_channel(&self, session_id: &str) {
        let mut channels = self.session_channels.write().await;
        if channels.remove(session_id).is_some() {
            debug!("Cleaned up event channel for session {}", session_id);
        }
    }

    /// Get count of active session channels.
    pub async fn session_channel_count(&self) -> usize {
        self.session_channels.read().await.len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe_all();

        let event = GatewayEvent::AgentStarted {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
            execution_id: "exec-1".to_string(),
            conversation_id: Some("conv-1".to_string()),
        };

        bus.publish(event.clone()).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.agent_id(), Some("agent-1"));
    }

    #[tokio::test]
    async fn test_agent_subscription() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe_agent("agent-1").await;

        let event = GatewayEvent::AgentStarted {
            agent_id: "agent-1".to_string(),
            session_id: "session-1".to_string(),
            execution_id: "exec-1".to_string(),
            conversation_id: Some("conv-1".to_string()),
        };

        bus.publish(event).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.conversation_id(), Some("conv-1"));
    }

    #[tokio::test]
    async fn test_session_subscription() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe_session("sess-123").await;

        bus.publish_session(
            "sess-123",
            GatewayEvent::SessionContinuationReady {
                session_id: "sess-123".to_string(),
                root_agent_id: "root".to_string(),
                root_execution_id: "exec-1".to_string(),
            },
        )
        .await;

        let event = rx.recv().await.unwrap();
        match event {
            GatewayEvent::SessionContinuationReady { session_id, .. } => {
                assert_eq!(session_id, "sess-123");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_session_channel_isolation() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe_session("sess-1").await;
        let mut rx2 = bus.subscribe_session("sess-2").await;

        bus.publish_session(
            "sess-1",
            GatewayEvent::SessionContinuationReady {
                session_id: "sess-1".to_string(),
                root_agent_id: "root".to_string(),
                root_execution_id: "exec-1".to_string(),
            },
        )
        .await;

        // sess-1 should receive
        assert!(rx1.try_recv().is_ok());
        // sess-2 should not receive
        assert!(rx2.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_session_channel_cleanup() {
        let bus = EventBus::new();

        // Subscribe to create channel
        let _rx = bus.subscribe_session("sess-cleanup").await;
        assert_eq!(bus.session_channel_count().await, 1);

        // Remove channel
        bus.remove_session_channel("sess-cleanup").await;
        assert_eq!(bus.session_channel_count().await, 0);
    }
}
