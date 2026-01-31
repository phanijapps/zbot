//! # WebSocket Session
//!
//! Session management for WebSocket connections.

use super::messages::ServerMessage;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// A WebSocket session representing a connected client.
#[derive(Debug)]
pub struct WsSession {
    /// Unique session ID.
    pub id: String,

    /// Agent ID this session is connected to (if any).
    pub agent_id: Option<String>,

    /// Sender for messages to this client.
    pub tx: mpsc::UnboundedSender<ServerMessage>,

    /// Active conversation IDs for this session.
    pub conversations: Vec<String>,
}

impl WsSession {
    /// Create a new session.
    pub fn new(tx: mpsc::UnboundedSender<ServerMessage>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: None,
            tx,
            conversations: Vec::new(),
        }
    }

    /// Create with a specific agent ID.
    pub fn with_agent(tx: mpsc::UnboundedSender<ServerMessage>, agent_id: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agent_id: Some(agent_id),
            tx,
            conversations: Vec::new(),
        }
    }

    /// Send a message to this session.
    pub fn send(&self, msg: ServerMessage) -> Result<(), mpsc::error::SendError<ServerMessage>> {
        self.tx.send(msg)
    }

    /// Add a conversation to this session.
    pub fn add_conversation(&mut self, conversation_id: String) {
        if !self.conversations.contains(&conversation_id) {
            self.conversations.push(conversation_id);
        }
    }

    /// Remove a conversation from this session.
    pub fn remove_conversation(&mut self, conversation_id: &str) {
        self.conversations.retain(|id| id != conversation_id);
    }
}

/// Session registry for tracking all active sessions.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    /// Sessions indexed by session ID.
    sessions: RwLock<HashMap<String, WsSession>>,

    /// Sessions indexed by agent ID.
    agent_sessions: RwLock<HashMap<String, Vec<String>>>,
}

impl SessionRegistry {
    /// Create a new session registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new session.
    pub async fn register(&self, session: WsSession) -> String {
        let session_id = session.id.clone();
        let agent_id = session.agent_id.clone();

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        if let Some(agent_id) = agent_id {
            let mut agent_sessions = self.agent_sessions.write().await;
            agent_sessions
                .entry(agent_id)
                .or_default()
                .push(session_id.clone());
        }

        session_id
    }

    /// Unregister a session.
    pub async fn unregister(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.remove(session_id) {
            if let Some(agent_id) = session.agent_id {
                let mut agent_sessions = self.agent_sessions.write().await;
                if let Some(ids) = agent_sessions.get_mut(&agent_id) {
                    ids.retain(|id| id != session_id);
                    if ids.is_empty() {
                        agent_sessions.remove(&agent_id);
                    }
                }
            }
        }
    }

    /// Get a session by ID.
    pub async fn get(&self, session_id: &str) -> Option<WsSession> {
        let sessions = self.sessions.read().await;
        // We need to clone or return a reference, but WsSession contains mpsc::UnboundedSender
        // which doesn't impl Clone. For now, we'll just check existence.
        sessions.get(session_id).map(|s| {
            // This is a simplified version - in practice you'd want to send messages
            // through the registry rather than getting the session directly
            WsSession {
                id: s.id.clone(),
                agent_id: s.agent_id.clone(),
                tx: s.tx.clone(),
                conversations: s.conversations.clone(),
            }
        })
    }

    /// Send a message to all sessions for an agent.
    pub async fn broadcast_to_agent(&self, agent_id: &str, msg: ServerMessage) {
        let sessions = self.sessions.read().await;
        let agent_sessions = self.agent_sessions.read().await;

        if let Some(session_ids) = agent_sessions.get(agent_id) {
            for session_id in session_ids {
                if let Some(session) = sessions.get(session_id) {
                    let _ = session.send(msg.clone());
                }
            }
        }
    }

    /// Get session count.
    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }
}
