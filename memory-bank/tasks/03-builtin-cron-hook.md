# Task 03: Built-in Cron Hook

## Context

Cron jobs trigger agents on a schedule. Unlike Web or CLI, cron jobs don't have a user waiting for a response - they run in the background and results are logged.

### Current State
- UI placeholder exists at `src/features/cron/WebCronPanel.tsx`
- No backend cron implementation exists

### Target Behavior
1. User creates cron job via API: agent_id, schedule, message
2. Cron service runs jobs on schedule
3. Each run creates `HookContext` with `BuiltinHookType::Cron`
4. Agent executes, response is logged (not sent anywhere)
5. Execution history stored for debugging

### Why Cron is Built-in
- Core scheduling is tightly coupled with gateway lifecycle
- Needs access to agent execution directly
- No external callback needed - just logging

---

## Specifications (BDD)

### Feature: Cron Job Management

```gherkin
Feature: Cron Job Management
  As a user
  I want to schedule agents to run automatically
  So that I can automate recurring tasks

  Scenario: Create a cron job
    When I call POST /api/cron with:
      """
      {
        "id": "daily-report",
        "agent_id": "report-agent",
        "schedule": "0 9 * * *",
        "message": "Generate daily sales report",
        "enabled": true
      }
      """
    Then the job is saved to cron.json
    And the job is registered with the scheduler

  Scenario: List cron jobs
    Given cron jobs exist:
      | id           | agent_id     | schedule    | enabled |
      | daily-report | report-agent | 0 9 * * *   | true    |
      | weekly-clean | cleanup      | 0 0 * * 0   | false   |
    When I call GET /api/cron
    Then I receive both jobs with next_run times

  Scenario: Delete a cron job
    Given cron job "daily-report" exists
    When I call DELETE /api/cron/daily-report
    Then the job is removed from cron.json
    And the job is unregistered from scheduler

  Scenario: Disable a cron job
    Given cron job "daily-report" exists and is enabled
    When I call PATCH /api/cron/daily-report with:
      """
      { "enabled": false }
      """
    Then the job remains in cron.json
    But it no longer triggers on schedule
```

### Feature: Cron Job Execution

```gherkin
Feature: Cron Job Execution
  As the cron scheduler
  I need to execute agents on schedule
  So that automated tasks run reliably

  Background:
    Given cron job exists:
      | id           | agent_id     | schedule  | message                  |
      | daily-report | report-agent | 0 9 * * * | Generate daily report    |

  Scenario: Scheduled execution
    Given the current time matches schedule "0 9 * * *"
    When the cron scheduler triggers
    Then a HookContext is created:
      | field           | value                        |
      | hook_type       | Builtin(Cron { job_id: "daily-report" }) |
      | source_id       | "cron:daily-report"          |
      | conversation_id | "cron-daily-report-{timestamp}" |
    And the agent is invoked with message "Generate daily report"

  Scenario: Execution logging
    Given the cron job triggers
    When the agent completes with response "Report generated: 150 sales"
    Then execution is logged to cron_history table:
      | job_id       | status    | response                      |
      | daily-report | completed | Report generated: 150 sales   |
    And no callback is made (cron has no callback_url)

  Scenario: Execution failure
    Given the cron job triggers
    When the agent fails with error "Database connection failed"
    Then execution is logged with status "failed"
    And error is logged to cron_history table

  Scenario: Agent uses respond tool
    Given the cron job triggers
    When the agent uses respond tool with "Task complete"
    Then the Cron hook logs the response
    But no external callback is made
```

---

## Implementation

### File: `application/gateway/src/hooks/builtin/cron.rs`

```rust
use crate::hooks::{HookContext, HookType, BuiltinHookType};
use tracing::{info, warn};

/// Built-in hook for Cron jobs (log-only responses)
pub struct CronHook;

impl CronHook {
    pub fn new() -> Self {
        Self
    }

    /// "Respond" for cron just logs the message
    pub async fn respond(&self, ctx: &HookContext, message: &str) -> Result<(), String> {
        if !matches!(ctx.hook_type, HookType::Builtin(BuiltinHookType::Cron { .. })) {
            return Err("CronHook cannot handle non-Cron context".into());
        }

        let job_id = match &ctx.hook_type {
            HookType::Builtin(BuiltinHookType::Cron { job_id }) => job_id,
            _ => unreachable!(),
        };

        info!(
            job_id = %job_id,
            source_id = %ctx.source_id,
            message = %message,
            "Cron job response (logged only)"
        );

        Ok(())
    }

    pub fn can_handle(&self, ctx: &HookContext) -> bool {
        matches!(ctx.hook_type, HookType::Builtin(BuiltinHookType::Cron { .. }))
    }
}

impl Default for CronHook {
    fn default() -> Self {
        Self::new()
    }
}
```

### File: `application/gateway/src/services/cron.rs`

```rust
use crate::hooks::{HookContext, BuiltinHookType};
use chrono::{DateTime, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub agent_id: String,
    pub schedule: String,  // cron expression
    pub message: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CronExecution {
    pub id: String,
    pub job_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: CronExecutionStatus,
    pub response: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CronExecutionStatus {
    Running,
    Completed,
    Failed,
}

pub struct CronService {
    jobs: RwLock<HashMap<String, CronJob>>,
    config_path: String,
    runtime: Arc<crate::services::RuntimeService>,
}

impl CronService {
    pub async fn new(config_path: &str, runtime: Arc<crate::services::RuntimeService>) -> Self {
        let jobs = Self::load_jobs(config_path).await.unwrap_or_default();
        Self {
            jobs: RwLock::new(jobs),
            config_path: config_path.to_string(),
            runtime,
        }
    }

    /// Start the cron scheduler loop
    pub async fn start(&self) {
        let mut ticker = interval(Duration::from_secs(60));  // Check every minute

        loop {
            ticker.tick().await;
            self.check_and_run_jobs().await;
        }
    }

    async fn check_and_run_jobs(&self) {
        let jobs = self.jobs.read().await;
        let now = Utc::now();

        for job in jobs.values() {
            if !job.enabled {
                continue;
            }

            if self.should_run(job, now) {
                let job = job.clone();
                let runtime = self.runtime.clone();

                tokio::spawn(async move {
                    Self::execute_job(&runtime, &job).await;
                });
            }
        }
    }

    fn should_run(&self, job: &CronJob, now: DateTime<Utc>) -> bool {
        let schedule = match Schedule::from_str(&job.schedule) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Check if current minute matches schedule
        schedule.upcoming(Utc).take(1).next()
            .map(|next| (next - now).num_seconds().abs() < 60)
            .unwrap_or(false)
    }

    async fn execute_job(runtime: &crate::services::RuntimeService, job: &CronJob) {
        let conversation_id = format!("cron-{}-{}", job.id, Uuid::new_v4());

        let hook_context = HookContext::builtin(
            BuiltinHookType::Cron { job_id: job.id.clone() },
            format!("cron:{}", job.id),
        ).with_conversation(&conversation_id);

        tracing::info!(job_id = %job.id, "Executing cron job");

        if let Err(e) = runtime.invoke_with_hook(
            &job.agent_id,
            &conversation_id,
            &job.message,
            hook_context,
        ).await {
            tracing::error!(job_id = %job.id, error = %e, "Cron job failed");
        }
    }

    // CRUD operations
    pub async fn list(&self) -> Vec<CronJob> {
        self.jobs.read().await.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<CronJob> {
        self.jobs.read().await.get(id).cloned()
    }

    pub async fn create(&self, job: CronJob) -> Result<(), String> {
        // Validate cron expression
        Schedule::from_str(&job.schedule)
            .map_err(|e| format!("Invalid cron expression: {}", e))?;

        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job);
        drop(jobs);

        self.save_jobs().await
    }

    pub async fn update(&self, id: &str, enabled: Option<bool>) -> Result<(), String> {
        let mut jobs = self.jobs.write().await;
        let job = jobs.get_mut(id).ok_or("Job not found")?;

        if let Some(enabled) = enabled {
            job.enabled = enabled;
        }

        drop(jobs);
        self.save_jobs().await
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let mut jobs = self.jobs.write().await;
        jobs.remove(id).ok_or("Job not found")?;
        drop(jobs);
        self.save_jobs().await
    }

    async fn load_jobs(path: &str) -> Result<HashMap<String, CronJob>, String> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    async fn save_jobs(&self) -> Result<(), String> {
        let jobs = self.jobs.read().await;
        let content = serde_json::to_string_pretty(&*jobs)
            .map_err(|e| e.to_string())?;
        tokio::fs::write(&self.config_path, content).await
            .map_err(|e| e.to_string())
    }
}
```

### File: `application/gateway/src/http/cron.rs`

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::services::cron::{CronJob, CronService};
use crate::state::AppState;
use serde::Deserialize;

pub async fn list_jobs(
    State(state): State<AppState>,
) -> Json<Vec<CronJob>> {
    Json(state.cron_service.list().await)
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CronJob>, StatusCode> {
    state.cron_service.get(&id).await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn create_job(
    State(state): State<AppState>,
    Json(job): Json<CronJob>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.cron_service.create(job).await
        .map(|_| StatusCode::CREATED)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

#[derive(Deserialize)]
pub struct UpdateJob {
    enabled: Option<bool>,
}

pub async fn update_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<UpdateJob>,
) -> Result<StatusCode, StatusCode> {
    state.cron_service.update(&id, update.enabled).await
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.cron_service.delete(&id).await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}
```

### Update: `application/gateway/src/hooks/builtin/mod.rs`

```rust
mod web;
mod cron;

pub use web::WebHook;
pub use cron::CronHook;
```

---

## Verification

### Unit Tests

```rust
#[tokio::test]
async fn test_cron_hook_logs_response() {
    let hook = CronHook::new();
    let ctx = HookContext::builtin(
        BuiltinHookType::Cron { job_id: "test-job".into() },
        "cron:test-job"
    );

    // Should succeed (just logs)
    let result = hook.respond(&ctx, "Job completed").await;
    assert!(result.is_ok());
}

#[test]
fn test_cron_expression_parsing() {
    use cron::Schedule;
    use std::str::FromStr;

    // Valid expressions
    assert!(Schedule::from_str("0 9 * * *").is_ok());  // 9 AM daily
    assert!(Schedule::from_str("*/15 * * * *").is_ok());  // Every 15 min
    assert!(Schedule::from_str("0 0 * * 0").is_ok());  // Sunday midnight

    // Invalid
    assert!(Schedule::from_str("invalid").is_err());
}
```

### API Tests

```bash
# Create job
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{"id":"test","agent_id":"root","schedule":"*/5 * * * *","message":"Test","enabled":true}'

# List jobs
curl http://localhost:18791/api/cron

# Disable job
curl -X PATCH http://localhost:18791/api/cron/test \
  -H "Content-Type: application/json" \
  -d '{"enabled":false}'

# Delete job
curl -X DELETE http://localhost:18791/api/cron/test
```

---

## Dependencies

- Task 01, 02 complete
- `cron` crate for schedule parsing
- Add to Cargo.toml: `cron = "0.12"`

## Outputs

- `application/gateway/src/hooks/builtin/cron.rs`
- `application/gateway/src/services/cron.rs`
- `application/gateway/src/http/cron.rs`
- `cron.json` - persisted job configurations

## Next Task

Task 04: External Hook Registration API
