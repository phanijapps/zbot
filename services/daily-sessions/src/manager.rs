// ============================================================================
// DAILY SESSION MANAGER
// High-level session management operations
// ============================================================================

use crate::repository::DailySessionRepository;
use crate::types::{DailySession, DaySummary, SessionMessage};
use crate::Result;
use std::sync::Arc;

pub struct DailySessionManager {
    repository: Arc<dyn DailySessionRepository>,
}

impl DailySessionManager {
    /// Create a new manager with a repository
    pub fn new(repository: Arc<dyn DailySessionRepository>) -> Self {
        Self { repository }
    }

    /// Get or create today's session for an agent
    pub async fn get_or_create_today(&self, agent_id: &str) -> Result<DailySession> {
        self.repository.get_or_create_today_session(agent_id).await
    }

    /// List previous days for an agent
    pub async fn list_previous_days(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<DaySummary>> {
        self.repository.list_previous_days(agent_id, limit).await
    }

    /// Get messages for a session
    pub async fn get_messages(&self, session_id: &str) -> Result<Vec<SessionMessage>> {
        self.repository.get_session_messages(session_id).await
    }

    /// Generate end-of-day summary
    pub async fn generate_end_of_day_summary(&self, session_id: &str) -> Result<String> {
        let messages = self.repository.get_session_messages(session_id).await?;

        // Simple summary for now
        let summary = format!("Session with {} messages.", messages.len());

        self.repository
            .update_session_summary(session_id, summary.clone())
            .await?;
        Ok(summary)
    }

    /// Record a message in a session
    pub async fn record_message(&self, _session_id: &str, message: SessionMessage) -> Result<()> {
        self.repository.create_message(message).await
    }

    /// Clear agent history before a certain date
    pub async fn clear_agent_history(&self, agent_id: &str, before_date: &str) -> Result<usize> {
        self.repository
            .delete_sessions_before(agent_id, before_date)
            .await
    }
}
