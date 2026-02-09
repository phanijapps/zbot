//! # Cron Job Configuration
//!
//! Types for cron job configuration and scheduling.

use serde::{Deserialize, Serialize};

/// Configuration for a scheduled cron job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobConfig {
    /// Unique identifier.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Cron schedule expression (e.g., "0 9 * * *" for 9am daily).
    pub schedule: String,

    /// Agent ID to execute.
    pub agent_id: String,

    /// Message to send to the agent.
    pub message: String,

    /// Connector IDs to send the response to.
    #[serde(default)]
    pub respond_to: Vec<String>,

    /// Whether the job is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Timezone for schedule interpretation (e.g., "America/New_York").
    /// Defaults to UTC if not specified.
    #[serde(default)]
    pub timezone: Option<String>,

    /// Optional metadata for the job.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,

    /// Last execution timestamp.
    #[serde(default)]
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,

    /// Next scheduled execution timestamp.
    #[serde(default)]
    pub next_run: Option<chrono::DateTime<chrono::Utc>>,

    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_enabled() -> bool {
    true
}

/// Request to create a new cron job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCronJobRequest {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub agent_id: String,
    pub message: String,
    #[serde(default)]
    pub respond_to: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Request to update a cron job.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCronJobRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub respond_to: Option<Vec<String>>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Stored cron jobs for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CronJobsStore {
    pub jobs: Vec<CronJobConfig>,
}

/// Result of triggering a cron job manually.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub execution_id: Option<String>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_job_serialization() {
        let job = CronJobConfig {
            id: "daily-report".to_string(),
            name: "Daily Report".to_string(),
            schedule: "0 0 9 * * *".to_string(), // sec min hour day month weekday
            agent_id: "report-agent".to_string(),
            message: "Generate daily report".to_string(),
            respond_to: vec!["email-connector".to_string()],
            enabled: true,
            timezone: Some("America/New_York".to_string()),
            metadata: None,
            last_run: None,
            next_run: None,
            created_at: None,
            updated_at: None,
        };

        let json = serde_json::to_string(&job).unwrap();
        let parsed: CronJobConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "daily-report");
        assert_eq!(parsed.schedule, "0 0 9 * * *");
    }

    #[test]
    fn test_cron_job_defaults() {
        let json = r#"{
            "id": "test",
            "name": "Test Job",
            "schedule": "0 * * * * *",
            "agent_id": "root",
            "message": "Hello"
        }"#;

        let job: CronJobConfig = serde_json::from_str(json).unwrap();
        assert!(job.enabled);
        assert!(job.respond_to.is_empty());
        assert!(job.timezone.is_none());
    }
}
