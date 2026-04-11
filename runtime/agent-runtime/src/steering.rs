//! Steering queue for mid-execution message injection.
//!
//! Allows external callers (UI, parent agents, system budgets) to inject
//! messages into a running executor without waiting for a tool round.

use tokio::sync::mpsc;

/// A message injected into the executor mid-execution.
#[derive(Debug, Clone)]
pub struct SteeringMessage {
    /// Content of the steering message.
    pub content: String,
    /// Who sent this steering message.
    pub source: SteeringSource,
    /// Priority level.
    pub priority: SteeringPriority,
}

/// Source of a steering message.
#[derive(Debug, Clone, PartialEq)]
pub enum SteeringSource {
    /// User typed a message in the UI.
    User,
    /// System budget/complexity enforcement.
    System,
    /// Parent agent steering a subagent.
    Parent,
}

/// Priority of a steering message.
#[derive(Debug, Clone, PartialEq)]
pub enum SteeringPriority {
    /// Inject after current tool round completes.
    Normal,
    /// Inject immediately before next LLM call.
    Interrupt,
}

/// Thread-safe channel for injecting messages into a running executor.
///
/// Create with `SteeringQueue::new()`. The sender half (`SteeringHandle`) can
/// be cloned and shared across threads. The receiver half stays with the executor.
pub struct SteeringQueue {
    rx: mpsc::UnboundedReceiver<SteeringMessage>,
}

/// Handle for sending steering messages to a running executor.
///
/// Clone this and hand it to the UI, parent agent, or budget enforcer.
#[derive(Clone)]
pub struct SteeringHandle {
    tx: mpsc::UnboundedSender<SteeringMessage>,
}

impl SteeringQueue {
    /// Create a new steering queue, returning (queue, handle).
    ///
    /// The queue is consumed by the executor. The handle is shared with callers.
    #[must_use] 
    pub fn new() -> (Self, SteeringHandle) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { rx }, SteeringHandle { tx })
    }

    /// Drain all pending messages. Non-blocking — returns empty vec if nothing pending.
    pub fn drain(&mut self) -> Vec<SteeringMessage> {
        let mut messages = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            messages.push(msg);
        }
        messages
    }
}

impl SteeringHandle {
    /// Send a steering message. Returns Err if the executor has been dropped.
    pub fn send(&self, message: SteeringMessage) -> Result<(), SteeringMessage> {
        self.tx.send(message).map_err(|e| e.0)
    }

    /// Convenience: send a system steering message.
    pub fn send_system(&self, content: impl Into<String>) -> Result<(), SteeringMessage> {
        self.send(SteeringMessage {
            content: content.into(),
            source: SteeringSource::System,
            priority: SteeringPriority::Normal,
        })
    }
}

impl std::fmt::Display for SteeringSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "User"),
            Self::System => write!(f, "System"),
            Self::Parent => write!(f, "Parent"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steering_queue_drain_empty() {
        let (mut queue, _handle) = SteeringQueue::new();
        let messages = queue.drain();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_steering_queue_send_and_drain() {
        let (mut queue, handle) = SteeringQueue::new();
        handle
            .send(SteeringMessage {
                content: "wrap up".to_string(),
                source: SteeringSource::System,
                priority: SteeringPriority::Normal,
            })
            .unwrap();
        handle
            .send(SteeringMessage {
                content: "user says stop".to_string(),
                source: SteeringSource::User,
                priority: SteeringPriority::Interrupt,
            })
            .unwrap();

        let messages = queue.drain();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "wrap up");
        assert_eq!(messages[0].source, SteeringSource::System);
        assert_eq!(messages[1].content, "user says stop");
        assert_eq!(messages[1].source, SteeringSource::User);
        assert_eq!(messages[1].priority, SteeringPriority::Interrupt);
    }

    #[test]
    fn test_steering_queue_drain_clears() {
        let (mut queue, handle) = SteeringQueue::new();
        handle.send_system("nudge").unwrap();
        let _ = queue.drain();
        let messages = queue.drain();
        assert!(messages.is_empty(), "drain should clear the queue");
    }

    #[test]
    fn test_steering_handle_clone() {
        let (mut queue, handle) = SteeringQueue::new();
        let handle2 = handle.clone();
        handle.send_system("from handle 1").unwrap();
        handle2.send_system("from handle 2").unwrap();
        let messages = queue.drain();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_steering_source_display() {
        assert_eq!(format!("{}", SteeringSource::User), "User");
        assert_eq!(format!("{}", SteeringSource::System), "System");
        assert_eq!(format!("{}", SteeringSource::Parent), "Parent");
    }
}
