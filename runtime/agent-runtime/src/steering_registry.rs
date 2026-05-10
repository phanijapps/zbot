//! Registry mapping execution IDs to live SteeringHandles.
//!
//! Created once at server startup. Passed to both the executor builder
//! (for SteerAgentTool) and spawn_delegated_agent (to store handles).

use crate::steering::{SteeringHandle, SteeringMessage, SteeringPriority, SteeringSource};
use std::collections::HashMap;
use std::sync::RwLock;

/// Result of a steer attempt.
#[derive(Debug, PartialEq)]
pub enum SteerResult {
    /// Message delivered to the running agent.
    Delivered,
    /// No agent found with that execution_id (completed, failed, or unknown).
    AgentNotRunning,
}

/// Thread-safe map from execution_id to SteeringHandle.
#[derive(Default)]
pub struct SteeringRegistry {
    handles: RwLock<HashMap<String, SteeringHandle>>,
}

impl SteeringRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handle when a subagent starts.
    pub fn register(&self, execution_id: &str, handle: SteeringHandle) {
        self.handles
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(execution_id.to_string(), handle);
    }

    /// Remove a handle when a subagent completes.
    pub fn remove(&self, execution_id: &str) {
        self.handles
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(execution_id);
    }

    /// Send a parent-sourced steering message to a running subagent.
    ///
    /// Returns `AgentNotRunning` if no handle exists or the channel is closed.
    pub fn steer(&self, execution_id: &str, message: impl Into<String>) -> SteerResult {
        let handles = self.handles.read().unwrap_or_else(|e| e.into_inner());
        match handles.get(execution_id) {
            None => SteerResult::AgentNotRunning,
            Some(handle) => {
                let msg = SteeringMessage {
                    content: message.into(),
                    source: SteeringSource::Parent,
                    priority: SteeringPriority::Normal,
                };
                match handle.send(msg) {
                    Ok(()) => SteerResult::Delivered,
                    Err(_) => SteerResult::AgentNotRunning, // channel closed — agent done
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steering::SteeringQueue;

    #[test]
    fn steer_unknown_id_returns_not_running() {
        let registry = SteeringRegistry::new();
        assert_eq!(
            registry.steer("exec-unknown", "hello"),
            SteerResult::AgentNotRunning
        );
    }

    #[test]
    fn register_then_steer_delivers_message() {
        let (mut queue, handle) = SteeringQueue::new();
        let registry = SteeringRegistry::new();
        registry.register("exec-123", handle);

        let result = registry.steer("exec-123", "pivot to approach B");
        assert_eq!(result, SteerResult::Delivered);

        let messages = queue.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "pivot to approach B");
        assert_eq!(messages[0].source, SteeringSource::Parent);
    }

    #[test]
    fn remove_then_steer_returns_not_running() {
        let (_queue, handle) = SteeringQueue::new();
        let registry = SteeringRegistry::new();
        registry.register("exec-456", handle);
        registry.remove("exec-456");

        assert_eq!(
            registry.steer("exec-456", "too late"),
            SteerResult::AgentNotRunning
        );
    }

    #[test]
    fn steer_dropped_channel_returns_not_running() {
        let (queue, handle) = SteeringQueue::new();
        let registry = SteeringRegistry::new();
        registry.register("exec-789", handle);
        drop(queue); // drop receiver — channel closed

        assert_eq!(
            registry.steer("exec-789", "no one listening"),
            SteerResult::AgentNotRunning
        );
    }
}
