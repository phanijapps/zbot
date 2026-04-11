//! # Session Management
//!
//! In-memory session implementation for agent conversations.

use crate::state::InMemoryState;
use zero_core::context::{Session, State};
use zero_core::types::Content;

/// In-memory session for agent conversations.
#[derive(Debug, Clone)]
pub struct InMemorySession {
    id: String,
    app_name: String,
    user_id: String,
    state: InMemoryState,
    history: Vec<Content>,
}

impl InMemorySession {
    /// Create a new session.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique session identifier
    /// * `app_name` - Application name
    /// * `user_id` - User identifier
    pub fn new(
        id: impl Into<String>,
        app_name: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            app_name: app_name.into(),
            user_id: user_id.into(),
            state: InMemoryState::new(),
            history: Vec::new(),
        }
    }

    /// Create a session with initial state.
    pub fn with_state(
        id: impl Into<String>,
        app_name: impl Into<String>,
        user_id: impl Into<String>,
        state: InMemoryState,
    ) -> Self {
        Self {
            id: id.into(),
            app_name: app_name.into(),
            user_id: user_id.into(),
            state,
            history: Vec::new(),
        }
    }

    /// Add a content to the conversation history.
    pub fn add_content(&mut self, content: Content) {
        self.history.push(content);
    }

    /// Add multiple contents to the conversation history.
    pub fn add_contents(&mut self, contents: Vec<Content>) {
        self.history.extend(contents);
    }

    /// Clear the conversation history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Get mutable reference to state.
    pub fn state_mut(&mut self) -> &mut InMemoryState {
        &mut self.state
    }

    /// Get reference to state.
    pub fn state(&self) -> &InMemoryState {
        &self.state
    }

    /// Get the session ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the app name.
    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    /// Get the user ID.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Get conversation history.
    pub fn history(&self) -> &[Content] {
        &self.history
    }

    /// Get the number of messages in history.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

impl Session for InMemorySession {
    fn id(&self) -> &str {
        self.id()
    }

    fn app_name(&self) -> &str {
        self.app_name()
    }

    fn user_id(&self) -> &str {
        self.user_id()
    }

    fn state(&self) -> &dyn State {
        &self.state
    }

    fn conversation_history(&self) -> Vec<Content> {
        self.history.clone()
    }
}

/// Wrapper for InMemorySession that implements Session with interior mutability.
/// This allows sharing the session across async contexts while satisfying the Session trait.
pub struct MutexSession(pub std::sync::Mutex<InMemorySession>);

impl MutexSession {
    /// Create a new MutexSession from an InMemorySession.
    pub fn new(session: InMemorySession) -> Self {
        Self(std::sync::Mutex::new(session))
    }

    /// Create a new MutexSession with the given parameters.
    pub fn with_params(id: String, app_name: String, user_id: String) -> Self {
        Self::new(InMemorySession::new(id, app_name, user_id))
    }

    /// Add content to the session (convenience method).
    pub fn add_content(&self, content: Content) {
        if let Ok(mut session) = self.0.lock() {
            session.add_content(content);
        }
    }

    /// Lock the session and get the inner mutex guard.
    pub fn lock(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, InMemorySession>,
        std::sync::PoisonError<std::sync::MutexGuard<'_, InMemorySession>>,
    > {
        self.0.lock()
    }
}

impl Session for MutexSession {
    fn id(&self) -> &str {
        // Return a placeholder - can't hold lock for reference
        "unknown"
    }

    fn app_name(&self) -> &str {
        "unknown"
    }

    fn user_id(&self) -> &str {
        "unknown"
    }

    fn state(&self) -> &dyn State {
        // Return a reference to a static empty state
        // Note: This is a limitation of the trait design with Mutex wrapping
        // The conversation_history() method is the primary one that works correctly
        use once_cell::sync::Lazy;
        use std::collections::HashMap;
        struct EmptyState;
        impl State for EmptyState {
            fn get(&self, _key: &str) -> Option<serde_json::Value> {
                None
            }
            fn set(&mut self, _key: String, _value: serde_json::Value) {}
            fn all(&self) -> HashMap<String, serde_json::Value> {
                HashMap::new()
            }
        }
        static EMPTY_STATE_BOX: Lazy<Box<dyn State>> =
            Lazy::new(|| Box::new(EmptyState) as Box<dyn State>);
        &**EMPTY_STATE_BOX
    }

    fn conversation_history(&self) -> Vec<Content> {
        if let Ok(session) = self.0.lock() {
            session.conversation_history()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = InMemorySession::new("session-1", "test-app", "user-1");
        assert_eq!(session.id(), "session-1");
        assert_eq!(session.app_name(), "test-app");
        assert_eq!(session.user_id(), "user-1");
        assert!(session.is_empty());
    }

    #[test]
    fn test_session_add_content() {
        let mut session = InMemorySession::new("session-1", "test-app", "user-1");

        let content = Content::user("Hello");
        session.add_content(content);

        assert_eq!(session.history_len(), 1);
        assert!(!session.is_empty());
    }

    #[test]
    fn test_session_add_contents() {
        let mut session = InMemorySession::new("session-1", "test-app", "user-1");

        let contents = vec![Content::user("Hello"), Content::assistant("Hi there!")];
        session.add_contents(contents);

        assert_eq!(session.history_len(), 2);
    }

    #[test]
    fn test_session_clear_history() {
        let mut session = InMemorySession::new("session-1", "test-app", "user-1");
        session.add_content(Content::user("Hello"));

        session.clear_history();
        assert!(session.is_empty());
    }

    #[test]
    fn test_session_conversation_history() {
        let mut session = InMemorySession::new("session-1", "test-app", "user-1");

        let content = Content::user("Hello");
        session.add_content(content.clone());

        let history = session.conversation_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].role, "user");
    }

    #[test]
    fn test_session_state() {
        let mut session = InMemorySession::new("session-1", "test-app", "user-1");

        session
            .state_mut()
            .set("user:theme".to_string(), serde_json::json!("dark"));
        assert_eq!(
            session.state().get("user:theme"),
            Some(&serde_json::json!("dark"))
        );
    }

    #[test]
    fn test_session_trait() {
        let session = InMemorySession::new("session-1", "test-app", "user-1");

        let dyn_session: &dyn Session = &session;
        assert_eq!(dyn_session.id(), "session-1");
        assert_eq!(dyn_session.app_name(), "test-app");
        assert_eq!(dyn_session.user_id(), "user-1");
    }
}
