//! Adapter from runtime observations to the existing bounded execution logs.
use agent_runtime::external_hooks::{HookActivity, HookActivitySink, HookEvent, HookStatus};
use api_logs::{
    HookActivityMetadata, HookActivityStatus, HookEventName, HookLogRecord, LogService,
};
use std::sync::Arc;
use zbot_runtime_sqlite::DatabaseManager;

pub(super) struct PersistHookActivity(pub Arc<LogService<DatabaseManager>>);
impl HookActivitySink for PersistHookActivity {
    fn record(&self, activity: HookActivity) {
        let event = match activity.event {
            HookEvent::SessionStart => HookEventName::SessionStart,
            HookEvent::UserPrompt => HookEventName::UserPrompt,
            HookEvent::RunStart => HookEventName::RunStart,
            HookEvent::RunEnd => HookEventName::RunEnd,
            HookEvent::BeforeModel => HookEventName::BeforeModel,
            HookEvent::AfterModel => HookEventName::AfterModel,
            HookEvent::BeforeTool => HookEventName::BeforeTool,
            HookEvent::AfterTool => HookEventName::AfterTool,
            HookEvent::InvalidToolCall => HookEventName::InvalidToolCall,
        };
        let status = match activity.status {
            HookStatus::Running => HookActivityStatus::Running,
            HookStatus::Completed => HookActivityStatus::Completed,
            HookStatus::Blocked => HookActivityStatus::Blocked,
            HookStatus::Failed => HookActivityStatus::Failed,
            HookStatus::Timeout => HookActivityStatus::Timeout,
            HookStatus::Cancelled => HookActivityStatus::Cancelled,
            HookStatus::Skipped => HookActivityStatus::Skipped,
        };
        let row = HookLogRecord {
            id: activity.activity_id,
            execution_id: activity.execution_id,
            session_id: activity.session_id,
            occurred_at: activity.occurred_at,
            hook: HookActivityMetadata {
                hook_id: activity.hook_id,
                event,
                event_id: activity.event_id,
                invocation_id: activity.invocation_id,
                agent_id: activity.agent_id,
                run_id: activity.run_id,
                status,
                duration_ms: (status != HookActivityStatus::Running)
                    .then_some(activity.duration_ms),
                exit_code: activity.exit_code,
            },
        };
        if self.0.log_hook_activity(&row).is_err() {
            tracing::warn!("Hook activity unavailable");
        }
    }
}
