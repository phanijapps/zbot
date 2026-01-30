//! # Event Broadcast
//!
//! Broadcast mechanism for distributing events to subscribers.

use super::GatewayEvent;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, trace};

/// Event bus for broadcasting events to connected clients.
pub struct EventBus {
    /// Global event channel.
    global_tx: broadcast::Sender<GatewayEvent>,

    /// Per-agent event channels.
    agent_channels: RwLock<HashMap<String, broadcast::Sender<GatewayEvent>>>,

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
            capacity,
        }
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
            conversation_id: "conv-1".to_string(),
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
            conversation_id: "conv-1".to_string(),
        };

        bus.publish(event).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.conversation_id(), Some("conv-1"));
    }
}
