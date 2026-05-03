//! # Cron Scheduler
//!
//! Built-in scheduler for triggering agents on a schedule.
//!
//! The cron scheduler allows you to:
//! - Schedule agent executions at specific times (cron syntax)
//! - Route responses to connectors via `respond_to`
//! - Manage jobs through REST API
//!
//! ## Example
//!
//! ```ignore
//! // Create a daily report job
//! let job = CronJobConfig {
//!     id: "daily-report".to_string(),
//!     name: "Daily Report Generator".to_string(),
//!     schedule: "0 9 * * *".to_string(),  // 9am daily
//!     agent_id: "report-agent".to_string(),
//!     message: "Generate the daily sales report".to_string(),
//!     respond_to: vec!["email-connector".to_string()],
//!     enabled: true,
//!     timezone: Some("America/New_York".to_string()),
//!     ..Default::default()
//! };
//! ```

// Re-export config and service types from gateway-cron crate
pub use gateway_cron::*;

use crate::bus::{GatewayBus, SessionRequest};
use execution_state::TriggerSource;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Default agent for cron jobs that don't specify (or leave blank) an
/// `agent_id`. Routes through the orchestrator, which can then delegate.
const DEFAULT_CRON_AGENT_ID: &str = "root";

/// Resolve which agent a cron job dispatches to. Trims the configured
/// `agent_id`; falls back to [`DEFAULT_CRON_AGENT_ID`] when empty.
fn resolve_cron_agent_id(configured: &str) -> &str {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        DEFAULT_CRON_AGENT_ID
    } else {
        trimmed
    }
}

/// The cron scheduler for scheduling agent executions.
pub struct CronScheduler {
    service: CronService,
    /// Wrapped in Mutex because shutdown() requires &mut self
    scheduler: Mutex<JobScheduler>,
    /// Maps job ID to scheduler UUID for management
    job_uuids: RwLock<HashMap<String, Uuid>>,
    /// Gateway bus for submitting sessions
    bus: Arc<dyn GatewayBus>,
}

impl CronScheduler {
    /// Create a new cron scheduler.
    pub async fn new(
        service: CronService,
        bus: Arc<dyn GatewayBus>,
    ) -> Result<Self, CronSchedulerError> {
        let scheduler = JobScheduler::new().await?;

        Ok(Self {
            service,
            scheduler: Mutex::new(scheduler),
            job_uuids: RwLock::new(HashMap::new()),
            bus,
        })
    }

    /// Initialize and start the scheduler.
    ///
    /// This loads all enabled jobs from disk and schedules them.
    pub async fn start(&self) -> Result<(), CronSchedulerError> {
        info!("Starting cron scheduler");

        // Load jobs from disk
        let jobs = self.service.load().await?;
        let enabled_count = jobs.jobs.iter().filter(|j| j.enabled).count();

        info!(
            total = jobs.jobs.len(),
            enabled = enabled_count,
            "Loading cron jobs"
        );

        // Schedule each enabled job
        for job_config in jobs.jobs.iter().filter(|j| j.enabled) {
            if let Err(e) = self.schedule_job(job_config).await {
                warn!(
                    job_id = %job_config.id,
                    error = %e,
                    "Failed to schedule job"
                );
            }
        }

        // Start the scheduler
        {
            let scheduler = self.scheduler.lock().await;
            scheduler.start().await?;
        }

        info!(enabled = enabled_count, "Cron scheduler started");
        Ok(())
    }

    /// Stop the scheduler.
    pub async fn stop(&self) -> Result<(), CronSchedulerError> {
        info!("Stopping cron scheduler");
        let mut scheduler = self.scheduler.lock().await;
        scheduler.shutdown().await?;
        Ok(())
    }

    /// Schedule a job.
    async fn schedule_job(&self, job_config: &CronJobConfig) -> Result<(), CronSchedulerError> {
        let job_id = job_config.id.clone();
        let agent_id = job_config.agent_id.clone();
        let message = job_config.message.clone();
        let respond_to = job_config.respond_to.clone();
        let bus = self.bus.clone();
        let service = self.service.clone();

        debug!(
            job_id = %job_id,
            schedule = %job_config.schedule,
            agent_id = %agent_id,
            "Scheduling cron job"
        );

        let job = Job::new_async(job_config.schedule.as_str(), move |_uuid, _lock| {
            let job_id = job_id.clone();
            let agent_id = agent_id.clone();
            let message = message.clone();
            let respond_to = respond_to.clone();
            let bus = bus.clone();
            let service = service.clone();

            Box::pin(async move {
                let target_agent = resolve_cron_agent_id(&agent_id).to_string();
                info!(
                    job_id = %job_id,
                    agent_id = %target_agent,
                    "Cron job triggered"
                );

                let request = SessionRequest::new(&target_agent, &message)
                    .with_source(TriggerSource::Cron)
                    .with_external_ref(format!("cron-{}", job_id))
                    .with_respond_to(respond_to);

                // Submit to gateway bus
                match bus.submit(request).await {
                    Ok(handle) => {
                        info!(
                            job_id = %job_id,
                            session_id = %handle.session_id,
                            execution_id = %handle.execution_id,
                            "Cron job submitted successfully"
                        );
                    }
                    Err(e) => {
                        error!(
                            job_id = %job_id,
                            error = %e,
                            "Failed to submit cron job"
                        );
                    }
                }

                // Update last run time
                if let Err(e) = service.update_last_run(&job_id).await {
                    warn!(
                        job_id = %job_id,
                        error = %e,
                        "Failed to update last run time"
                    );
                }
            })
        })?;

        let uuid = job.guid();
        {
            let scheduler = self.scheduler.lock().await;
            scheduler.add(job).await?;
        }

        // Store the UUID for later management
        self.job_uuids
            .write()
            .await
            .insert(job_config.id.clone(), uuid);

        info!(
            job_id = %job_config.id,
            schedule = %job_config.schedule,
            "Cron job scheduled"
        );

        Ok(())
    }

    /// Unschedule a job.
    async fn unschedule_job(&self, job_id: &str) -> Result<(), CronSchedulerError> {
        let uuid = self.job_uuids.write().await.remove(job_id);

        if let Some(uuid) = uuid {
            let scheduler = self.scheduler.lock().await;
            scheduler.remove(&uuid).await?;
            info!(job_id = %job_id, "Cron job unscheduled");
        }

        Ok(())
    }

    /// Reschedule a job (unschedule and schedule again).
    pub async fn reschedule_job(&self, job_id: &str) -> Result<(), CronSchedulerError> {
        self.unschedule_job(job_id).await?;

        let job = self.service.get(job_id).await?;
        if job.enabled {
            self.schedule_job(&job).await?;
        }

        Ok(())
    }

    /// Get the service for CRUD operations.
    pub fn service(&self) -> &CronService {
        &self.service
    }

    /// Manually trigger a job.
    pub async fn trigger(&self, job_id: &str) -> Result<TriggerResult, CronSchedulerError> {
        let job = self.service.get(job_id).await?;
        let target_agent = resolve_cron_agent_id(&job.agent_id).to_string();

        info!(
            job_id = %job_id,
            agent_id = %target_agent,
            "Manually triggering cron job"
        );

        let request = SessionRequest::new(&target_agent, &job.message)
            .with_source(TriggerSource::Cron)
            .with_external_ref(format!("cron-{}-manual", job_id))
            .with_respond_to(job.respond_to.clone());

        // Submit to gateway bus
        match self.bus.submit(request).await {
            Ok(handle) => {
                // Update last run time
                let _ = self.service.update_last_run(job_id).await;

                Ok(TriggerResult {
                    success: true,
                    session_id: Some(handle.session_id),
                    execution_id: Some(handle.execution_id),
                    message: "Job triggered successfully".to_string(),
                })
            }
            Err(e) => Ok(TriggerResult {
                success: false,
                session_id: None,
                execution_id: None,
                message: format!("Failed to trigger job: {}", e),
            }),
        }
    }

    /// Create a new job and schedule it if enabled.
    pub async fn create_job(&self, request: CreateCronJobRequest) -> CronResult<CronJobConfig> {
        let job = self.service.create(request).await?;

        if job.enabled {
            if let Err(e) = self.schedule_job(&job).await {
                warn!(
                    job_id = %job.id,
                    error = %e,
                    "Job created but failed to schedule"
                );
            }
        }

        Ok(job)
    }

    /// Update a job and reschedule if needed.
    pub async fn update_job(
        &self,
        id: &str,
        request: UpdateCronJobRequest,
    ) -> CronResult<CronJobConfig> {
        let schedule_changed = request.schedule.is_some();
        let enabled_changed = request.enabled.is_some();

        let job = self.service.update(id, request).await?;

        // Reschedule if schedule or enabled status changed
        if schedule_changed || enabled_changed {
            if let Err(e) = self.reschedule_job(id).await {
                warn!(
                    job_id = %id,
                    error = %e,
                    "Job updated but failed to reschedule"
                );
            }
        }

        Ok(job)
    }

    /// Delete a job and unschedule it.
    pub async fn delete_job(&self, id: &str) -> CronResult<()> {
        // Unschedule first
        if let Err(e) = self.unschedule_job(id).await {
            warn!(
                job_id = %id,
                error = %e,
                "Failed to unschedule job before deletion"
            );
        }

        self.service.delete(id).await
    }

    /// Enable a job and schedule it.
    pub async fn enable_job(&self, id: &str) -> CronResult<CronJobConfig> {
        let job = self.service.enable(id).await?;

        if let Err(e) = self.schedule_job(&job).await {
            warn!(
                job_id = %id,
                error = %e,
                "Job enabled but failed to schedule"
            );
        }

        Ok(job)
    }

    /// Disable a job and unschedule it.
    pub async fn disable_job(&self, id: &str) -> CronResult<CronJobConfig> {
        if let Err(e) = self.unschedule_job(id).await {
            warn!(
                job_id = %id,
                error = %e,
                "Failed to unschedule job"
            );
        }

        self.service.disable(id).await
    }

    /// List all jobs.
    pub async fn list_jobs(&self) -> CronResult<Vec<CronJobConfig>> {
        self.service.list().await
    }

    /// Get a job by ID.
    pub async fn get_job(&self, id: &str) -> CronResult<CronJobConfig> {
        self.service.get(id).await
    }
}

/// Errors from the cron scheduler.
#[derive(Debug, thiserror::Error)]
pub enum CronSchedulerError {
    #[error("Service error: {0}")]
    Service(#[from] CronServiceError),

    #[error("Scheduler error: {0}")]
    Scheduler(#[from] JobSchedulerError),
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_CRON_AGENT_ID, resolve_cron_agent_id};

    #[test]
    fn returns_configured_agent_id_when_present() {
        assert_eq!(resolve_cron_agent_id("general-purpose"), "general-purpose");
        assert_eq!(resolve_cron_agent_id("research-agent"), "research-agent");
    }

    #[test]
    fn falls_back_to_root_when_empty() {
        assert_eq!(resolve_cron_agent_id(""), DEFAULT_CRON_AGENT_ID);
    }

    #[test]
    fn falls_back_to_root_when_whitespace_only() {
        assert_eq!(resolve_cron_agent_id("   "), DEFAULT_CRON_AGENT_ID);
        assert_eq!(resolve_cron_agent_id("\t\n"), DEFAULT_CRON_AGENT_ID);
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(resolve_cron_agent_id("  builder-agent  "), "builder-agent");
    }

    #[test]
    fn passes_through_explicit_root() {
        assert_eq!(resolve_cron_agent_id("root"), "root");
    }

    #[test]
    fn bundled_default_cron_template_parses() {
        let bytes = gateway_templates::Templates::get("default_cron.json")
            .expect("default_cron.json bundled in gateway-templates")
            .data;

        let requests: Vec<gateway_cron::CreateCronJobRequest> = serde_json::from_slice(&bytes)
            .expect("default_cron.json must match CreateCronJobRequest schema");

        let cleanup = requests
            .iter()
            .find(|r| r.id == "default-cleanup")
            .expect("bundled `default-cleanup` cron job missing");

        assert_eq!(cleanup.agent_id, "general-purpose");
        assert_eq!(cleanup.schedule, "0 0 */4 * * *");
        assert!(cleanup.enabled);
    }
}
