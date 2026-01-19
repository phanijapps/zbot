// ============================================================================
// DAILY SESSION MANAGER
// Core logic for daily session lifecycle and management
// ============================================================================

use crate::repository::DailySessionRepository;
use crate::types::{DailySession, DaySummary};
use crate::Result;
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, info};

/// Manager for daily session operations
pub struct DailySessionManager {
    repository: Arc<dyn DailySessionRepository>,
}

impl DailySessionManager {
    /// Create a new daily session manager
    pub fn new(repository: Arc<dyn DailySessionRepository>) -> Self {
        Self { repository }
    }

    /// Get or create today's session for an agent
    ///
    /// This is the main entry point for agent interactions.
    /// Automatically creates a new session if today's doesn't exist.
    pub async fn get_or_create_today(&self, agent_id: &str) -> Result<DailySession> {
        debug!("Getting or creating today's session for agent: {}", agent_id);
        self.repository.get_or_create_today_session(agent_id).await
    }

    /// Get a specific session by ID
    pub async fn get_session(&self, session_id: &str) -> Result<Option<DailySession>> {
        debug!("Getting session: {}", session_id);
        self.repository.get_session(session_id).await
    }

    /// List previous days for an agent
    ///
    /// Returns day summaries for previous days, most recent first.
    pub async fn list_previous_days(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<DaySummary>> {
        debug!(
            "Listing previous {} days for agent: {}",
            limit,
            agent_id
        );
        self.repository.list_previous_days(agent_id, limit).await
    }

    /// Generate end-of-day summary for a session
    ///
    /// This would typically be called:
    /// - When the app closes
    /// - When a new day starts
    /// - When user explicitly triggers it
    pub async fn generate_end_of_day_summary(&self, session_id: &str) -> Result<String> {
        info!("Generating end-of-day summary for session: {}", session_id);

        // Get the session's messages
        let messages = self.repository.get_session_messages(session_id).await?;

        // TODO: Use LLM to generate summary
        // For now, return a placeholder
        let summary = format!(
            "Session had {} messages. Summary generation not yet implemented.",
            messages.len()
        );

        // Store the summary
        self.repository
            .update_session_summary(session_id, summary.clone())
            .await?;

        Ok(summary)
    }

    /// Create a new session with a reference to a previous session's summary
    ///
    /// This is used when starting a new day to carry forward context.
    pub async fn create_with_summary_reference(
        &self,
        agent_id: &str,
        previous_summary: String,
    ) -> Result<DailySession> {
        info!(
            "Creating new session for agent {} with previous summary",
            agent_id
        );

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let mut session = DailySession::new(agent_id.to_string(), today);

        // Store the summary as the context for this new session
        session.summary = Some(previous_summary);

        // TODO: Save to database
        Ok(session)
    }

    /// Check if a new day has started and trigger summary generation
    ///
    /// Returns the previous day's summary if a new day was detected.
    pub async fn check_and_transition_day(
        &self,
        agent_id: &str,
        last_session_date: &str,
    ) -> Result<Option<String>> {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        if last_session_date != today {
            info!(
                "Day transition detected for agent {}: {} -> {}",
                agent_id, last_session_date, today
            );

            // Get the last session for the previous day
            let previous_session_id =
                DailySession::generate_id(agent_id, last_session_date);

            // Generate summary for the previous day
            let summary = self
                .generate_end_of_day_summary(&previous_session_id)
                .await?;

            Ok(Some(summary))
        } else {
            Ok(None)
        }
    }

    /// Delete agent history before a certain date
    ///
    /// Used for Chrome-style history removal.
    pub async fn clear_agent_history(
        &self,
        agent_id: &str,
        before_date: &str,
    ) -> Result<usize> {
        info!(
            "Clearing history for agent {} before {}",
            agent_id,
            before_date
        );
        self.repository
            .delete_sessions_before(agent_id, before_date)
            .await
    }

    /// Record a message in the session
    pub async fn record_message(
        &self,
        session_id: &str,
        message: crate::types::SessionMessage,
    ) -> Result<()> {
        debug!("Recording message in session: {}", session_id);

        // Create the message
        self.repository.create_message(message).await?;

        // Increment message count
        self.repository.increment_message_count(session_id).await?;

        Ok(())
    }

    /// Record tokens used in a session
    pub async fn record_tokens(&self, session_id: &str, tokens: i64) -> Result<()> {
        debug!("Recording {} tokens for session: {}", tokens, session_id);
        self.repository.add_token_count(session_id, tokens).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::SqliteDailySessionRepository;

    #[tokio::test]
    async fn test_daily_session_manager() {
        let repo = Arc::new(SqliteDailySessionRepository::new()) as Arc<dyn DailySessionRepository>;
        let manager = DailySessionManager::new(repo);

        let session = manager
            .get_or_create_today("test-agent")
            .await
            .unwrap();

        assert_eq!(session.agent_id, "test-agent");
        assert!(session.is_today());
    }

    #[tokio::test]
    async fn test_day_transition_detection() {
        let repo = Arc::new(SqliteDailySessionRepository::new()) as Arc<dyn DailySessionRepository>;
        let manager = DailySessionManager::new(repo);

        // Same day - no transition
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let result = manager
            .check_and_transition_day("test-agent", &today)
            .await
            .unwrap();

        assert!(result.is_none());

        // Different day - transition detected
        let yesterday = "2025-01-17".to_string();
        let result = manager
            .check_and_transition_day("test-agent", &yesterday)
            .await
            .unwrap();

        // Should return a summary (placeholder for now)
        assert!(result.is_some());
    }
}
