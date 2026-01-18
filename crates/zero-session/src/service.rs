//! # Session Service
//!
//! Service layer for managing sessions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use zero_core::Result;
use zero_core::context::Session;
use crate::session::InMemorySession;

/// Service for creating and managing sessions.
#[async_trait::async_trait]
pub trait SessionService: Send + Sync {
    /// Create a new session.
    async fn create_session(
        &self,
        app_name: String,
        user_id: String,
    ) -> Result<Arc<dyn Session>>;

    /// Get a session by ID.
    async fn get_session(&self, session_id: &str) -> Result<Option<Arc<dyn Session>>>;

    /// Delete a session.
    async fn delete_session(&self, session_id: &str) -> Result<bool>;

    /// List all sessions for a user.
    async fn list_sessions(&self, user_id: &str) -> Result<Vec<Arc<dyn Session>>>;
}

/// In-memory session service implementation.
pub struct InMemorySessionService {
    sessions: Arc<RwLock<HashMap<String, Arc<dyn Session>>>>,
}

impl InMemorySessionService {
    /// Create a new in-memory session service.
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a new session ID.
    fn generate_id() -> String {
        format!("session_{}", Uuid::new_v4())
    }

    /// Create a session with a specific ID.
    pub async fn create_with_id(
        &self,
        id: String,
        app_name: String,
        user_id: String,
    ) -> Result<Arc<dyn Session>> {
        let session: Arc<dyn Session> = Arc::new(InMemorySession::new(
            id.clone(),
            app_name,
            user_id,
        ));

        let mut sessions = self.sessions.write().await;
        sessions.insert(id, session.clone());

        Ok(session)
    }
}

impl Default for InMemorySessionService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SessionService for InMemorySessionService {
    async fn create_session(
        &self,
        app_name: String,
        user_id: String,
    ) -> Result<Arc<dyn Session>> {
        let id = Self::generate_id();
        self.create_with_id(id, app_name, user_id).await
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Arc<dyn Session>>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    async fn delete_session(&self, session_id: &str) -> Result<bool> {
        let mut sessions = self.sessions.write().await;
        Ok(sessions.remove(session_id).is_some())
    }

    async fn list_sessions(&self, user_id: &str) -> Result<Vec<Arc<dyn Session>>> {
        let sessions = self.sessions.read().await;
        let user_sessions = sessions
            .values()
            .filter(|s| s.user_id() == user_id)
            .cloned()
            .collect();

        Ok(user_sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_create_session() {
        let service = InMemorySessionService::new();

        let session = service
            .create_session("test-app".to_string(), "user-1".to_string())
            .await
            .unwrap();

        assert_eq!(session.app_name(), "test-app");
        assert_eq!(session.user_id(), "user-1");
        assert!(!session.id().is_empty());
    }

    #[tokio::test]
    async fn test_service_get_session() {
        let service = InMemorySessionService::new();

        let session = service
            .create_session("test-app".to_string(), "user-1".to_string())
            .await
            .unwrap();

        let session_id = session.id();
        let retrieved = service.get_session(session_id).await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), session_id);
    }

    #[tokio::test]
    async fn test_service_delete_session() {
        let service = InMemorySessionService::new();

        let session = service
            .create_session("test-app".to_string(), "user-1".to_string())
            .await
            .unwrap();

        let session_id = session.id();
        let deleted = service.delete_session(session_id).await.unwrap();

        assert!(deleted);

        let retrieved = service.get_session(session_id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_service_list_sessions() {
        let service = InMemorySessionService::new();

        service
            .create_session("test-app".to_string(), "user-1".to_string())
            .await
            .unwrap();
        service
            .create_session("test-app".to_string(), "user-1".to_string())
            .await
            .unwrap();
        service
            .create_session("test-app".to_string(), "user-2".to_string())
            .await
            .unwrap();

        let user1_sessions = service.list_sessions("user-1").await.unwrap();
        let user2_sessions = service.list_sessions("user-2").await.unwrap();

        assert_eq!(user1_sessions.len(), 2);
        assert_eq!(user2_sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_service_create_with_id() {
        let service = InMemorySessionService::new();

        let session = service
            .create_with_id(
                "custom-id".to_string(),
                "test-app".to_string(),
                "user-1".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(session.id(), "custom-id");
    }
}
