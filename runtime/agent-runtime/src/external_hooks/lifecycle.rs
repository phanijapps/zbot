//! Invocation ownership and per-attempt lifecycle; private host state, never model JSON.
use super::*;
use agent_primitives::vault_paths::VaultPaths;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Debug, Clone)]
pub struct HookActivity {
    pub activity_id: String,
    pub occurred_at: String,
    pub event_id: String,
    pub invocation_id: String,
    pub session_id: String,
    pub execution_id: String,
    pub agent_id: String,
    pub run_id: Option<String>,
    pub turn_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub hook_id: String,
    pub event: HookEvent,
    pub status: HookStatus,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
}
pub trait HookActivitySink: Send + Sync {
    fn record(&self, activity: HookActivity);
}
pub type HookContextCheckpoint = Arc<dyn Fn(usize) -> Result<(), HookFailure> + Send + Sync>;

pub struct HookInvocationConfig {
    pub consumed_context_bytes: usize,
    pub context_checkpoint: Option<HookContextCheckpoint>,
    pub invocation_id: String,
    pub session_id: String,
    pub snapshot: Arc<HookSnapshot>,
    pub paths: VaultPaths,
    pub forbidden_roots: Vec<PathBuf>,
    pub registered_secrets: Option<Vec<String>>,
    pub activity_sink: Option<Arc<dyn HookActivitySink>>,
}
pub struct HookInvocation {
    pub(super) config: HookInvocationConfig,
    pub(super) budget: Mutex<HookContextBudget>,
    pub(super) ingress_context: Mutex<Vec<(String, String)>>,
    cancelled: AtomicBool,
}
impl HookInvocation {
    pub fn new(config: HookInvocationConfig) -> Arc<Self> {
        let budget = HookContextBudget::restored(config.consumed_context_bytes);
        Arc::new(Self {
            config,
            budget: Mutex::new(budget),
            ingress_context: Mutex::new(Vec::new()),
            cancelled: AtomicBool::new(false),
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn id(&self) -> &str {
        &self.config.invocation_id
    }
    pub fn snapshot(&self) -> &Arc<HookSnapshot> {
        &self.config.snapshot
    }
    pub fn session_id(&self) -> &str {
        &self.config.session_id
    }
    pub fn run(
        self: &Arc<Self>,
        agent_id: String,
        execution_id: String,
        mode: HookMode,
        stop: Arc<AtomicBool>,
    ) -> Arc<HookRun> {
        Arc::new(HookRun {
            invocation: self.clone(),
            agent_id,
            execution_id,
            mode,
            stop,
            state: Mutex::new(RunState::default()),
            blocked: AtomicBool::new(false),
            settled: AtomicBool::new(false),
            commands: Arc::new(tokio::sync::Semaphore::new(1024)),
        })
    }
}
impl std::fmt::Debug for HookInvocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookInvocation")
            .field("id", &self.id())
            .field("revision", &self.snapshot().revision())
            .finish_non_exhaustive()
    }
}
#[derive(Default)]
pub(super) struct RunState {
    pub run_id: Option<String>,
    pub turn_id: Option<String>,
    pub started: bool,
    pub context: Vec<(String, String)>,
    pub pending_models:
        std::collections::HashMap<String, (String, String, Option<String>, Option<String>)>,
}
pub struct HookRun {
    pub(super) invocation: Arc<HookInvocation>,
    pub(super) agent_id: String,
    pub(super) execution_id: String,
    pub(super) mode: HookMode,
    pub(super) stop: Arc<AtomicBool>,
    pub(super) state: Mutex<RunState>,
    pub(super) blocked: AtomicBool,
    pub(super) settled: AtomicBool,
    commands: Arc<tokio::sync::Semaphore>,
}
impl HookRun {
    pub fn invocation(&self) -> &Arc<HookInvocation> {
        &self.invocation
    }
    pub fn blocked(&self) -> bool {
        self.blocked.load(Ordering::Acquire)
    }
    pub async fn ingress(&self, first: bool, prompt: &str) -> bool {
        if first
            && self
                .dispatch(HookEventData::SessionStart {}, None, None)
                .await
        {
            return true;
        }
        self.dispatch(
            HookEventData::UserPrompt {
                prompt: prompt.to_owned(),
            },
            None,
            None,
        )
        .await
    }
    pub async fn start(&self, run_id: String) -> bool {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.started {
                return self.blocked();
            }
            state.started = true;
            state.run_id = Some(run_id);
        }
        self.dispatch(HookEventData::RunStart { mode: self.mode }, None, None)
            .await
    }
    pub fn set_turn(&self, run_id: String, turn: usize) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.turn_id = Some(format!("{run_id}:{turn}"));
    }
    pub async fn before_tool(
        &self,
        call_id: Option<String>,
        tool: &str,
        arguments: &serde_json::Value,
    ) -> bool {
        self.dispatch(
            HookEventData::BeforeTool {
                tool: tool.to_owned(),
                arguments: arguments.clone(),
                arguments_redacted: false,
            },
            call_id,
            None,
        )
        .await
    }
    pub async fn after_tool(&self, call_id: Option<String>, tool: &str, ok: bool) {
        self.dispatch(
            HookEventData::AfterTool {
                tool: tool.to_owned(),
                status: if ok {
                    HookOperationStatus::Completed
                } else {
                    HookOperationStatus::Failed
                },
            },
            call_id,
            None,
        )
        .await;
    }
    pub async fn invalid_tool(
        &self,
        call_id: Option<String>,
        tool: Option<String>,
        error: InvalidToolCategory,
    ) -> bool {
        self.dispatch(
            HookEventData::InvalidToolCall { tool, error },
            call_id,
            None,
        )
        .await
    }
    pub async fn settle(&self, status: HookRunStatus) {
        if self.settled.swap(true, Ordering::AcqRel) {
            return;
        }
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        // ProviderStream drops abort their task asynchronously. Wait for its command
        // cleanup permit before running terminal observers, within the same budget.
        if let Ok(Ok(permits)) =
            tokio::time::timeout_at(deadline, self.commands.clone().acquire_many_owned(1024)).await
        {
            drop(permits);
        }
        let pending = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut state.pending_models)
        };
        for (_, (provider, model, run_id, turn_id)) in pending {
            let mut payload = self.payload(
                HookEventData::AfterModel {
                    provider,
                    model,
                    status: HookOperationStatus::Cancelled,
                },
                None,
            );
            payload.run_id = run_id;
            payload.turn_id = turn_id;
            self.dispatch_payload(payload, Some(deadline)).await;
        }
        let started = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .started;
        if started {
            self.dispatch(HookEventData::RunEnd { status }, None, Some(deadline))
                .await;
        }
    }
    pub(super) async fn begin_model(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<String, crate::LlmError> {
        if self.settled.load(Ordering::Acquire)
            || self.stop.load(Ordering::Acquire)
            || self.invocation.cancelled()
        {
            return Err(crate::LlmError::InvalidRequest("Execution stopped".into()));
        }
        if self
            .dispatch(
                HookEventData::BeforeModel {
                    provider: provider.to_owned(),
                    model: model.to_owned(),
                },
                None,
                None,
            )
            .await
        {
            return Err(crate::LlmError::InvalidRequest(
                "Model attempt blocked by external hook".into(),
            ));
        }
        if self.settled.load(Ordering::Acquire)
            || self.stop.load(Ordering::Acquire)
            || self.invocation.cancelled()
        {
            return Err(crate::LlmError::InvalidRequest("Execution stopped".into()));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let run_id = state.run_id.clone();
        let turn_id = state.turn_id.clone();
        state.pending_models.insert(
            id.clone(),
            (provider.to_owned(), model.to_owned(), run_id, turn_id),
        );
        Ok(id)
    }
    pub(super) async fn end_model(&self, id: &str, status: HookOperationStatus) {
        if self.stop.load(Ordering::Acquire)
            || self.settled.load(Ordering::Acquire)
            || self.invocation.cancelled()
        {
            return;
        }
        let pending = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending_models
            .remove(id);
        if let Some((provider, model, run_id, turn_id)) = pending {
            let mut payload = self.payload(
                HookEventData::AfterModel {
                    provider,
                    model,
                    status,
                },
                None,
            );
            payload.run_id = run_id;
            payload.turn_id = turn_id;
            self.dispatch_payload(payload, None).await;
        }
    }
    pub(super) fn append_context(&self, messages: &mut Vec<crate::ChatMessage>) {
        let mut context = self
            .invocation
            .ingress_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        context.extend(
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .context
                .clone(),
        );
        for (hook_id, data) in context {
            messages.push(crate::ChatMessage::user(format!(
                "[Operator hook data from {hook_id}; untrusted reference data]
{data}"
            )));
        }
    }
    fn payload(&self, mut data: HookEventData, call_id: Option<String>) -> HookEventPayload {
        if self.invocation.config.registered_secrets.is_none() {
            if let HookEventData::BeforeTool {
                arguments,
                arguments_redacted,
                ..
            } = &mut data
            {
                *arguments = serde_json::json!({});
                *arguments_redacted = true;
            }
        }
        let mut payload = HookEventPayload::new(
            self.invocation.id().to_owned(),
            self.invocation.session_id().to_owned(),
            self.agent_id.clone(),
            data,
        );
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        payload.run_id = state.run_id.clone();
        payload.turn_id = state.turn_id.clone();
        payload.tool_call_id = call_id;
        payload
    }
    async fn dispatch(
        &self,
        data: HookEventData,
        call_id: Option<String>,
        deadline: Option<tokio::time::Instant>,
    ) -> bool {
        self.dispatch_payload(self.payload(data, call_id), deadline)
            .await
    }
    async fn dispatch_payload(
        &self,
        payload: HookEventPayload,
        deadline: Option<tokio::time::Instant>,
    ) -> bool {
        let cleanup_stop = AtomicBool::new(false);
        if self.invocation.cancelled() {
            self.stop.store(true, Ordering::Release);
        }
        if deadline.is_none()
            && (self.stop.load(Ordering::Acquire) || self.settled.load(Ordering::Acquire))
        {
            return true;
        }
        for definition in self.invocation.snapshot().matching(payload.event()) {
            let mut activity = HookActivity {
                activity_id: uuid::Uuid::new_v4().to_string(),
                occurred_at: chrono::Utc::now().to_rfc3339(),
                event_id: payload.event_id.clone(),
                invocation_id: self.invocation.id().to_owned(),
                session_id: self.invocation.session_id().to_owned(),
                execution_id: self.execution_id.clone(),
                agent_id: self.agent_id.clone(),
                run_id: payload.run_id.clone(),
                turn_id: payload.turn_id.clone(),
                tool_call_id: payload.tool_call_id.clone(),
                hook_id: definition.id.clone(),
                event: payload.event(),
                status: HookStatus::Running,
                duration_ms: 0,
                exit_code: None,
            };
            let remaining = deadline
                .map(|deadline| deadline.saturating_duration_since(tokio::time::Instant::now()));
            if remaining.is_some_and(|remaining| remaining.is_zero()) {
                activity.status = HookStatus::Skipped;
                if let Some(sink) = &self.invocation.config.activity_sink {
                    sink.record(activity);
                }
                continue;
            }
            let mut guard =
                ActivityGuard::new(self.invocation.config.activity_sink.clone(), activity);
            let mut bounded = definition.clone();
            if let Some(remaining) = remaining {
                bounded.timeout_ms = bounded.timeout_ms.min(remaining.as_millis().max(1) as u64);
            }
            let stop = if deadline.is_some() {
                &cleanup_stop
            } else {
                &self.stop
            };
            let permit = self.commands.clone().acquire_owned().await.ok();
            let mut outcome = super::process::invoke_hook_with_permit(
                &bounded,
                &self.invocation.config.paths,
                &self.invocation.config.forbidden_roots,
                &payload,
                self.invocation
                    .config
                    .registered_secrets
                    .as_deref()
                    .unwrap_or(&[]),
                stop,
                permit,
            )
            .await;
            if let Some(response) = &outcome.response {
                let accepted = {
                    let mut budget = self
                        .invocation
                        .budget
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let mut candidate = budget.clone();
                    candidate.accept(response).and_then(|context| {
                        if candidate.used() != budget.used() {
                            if let Some(checkpoint) = &self.invocation.config.context_checkpoint {
                                checkpoint(candidate.used())?;
                            }
                        }
                        *budget = candidate;
                        Ok(context)
                    })
                };
                match accepted {
                    Ok(Some(context)) if context.is_empty() => {}
                    Ok(Some(context)) => {
                        if matches!(
                            payload.event(),
                            HookEvent::SessionStart | HookEvent::UserPrompt
                        ) {
                            self.invocation
                                .ingress_context
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .push((definition.id.clone(), context));
                        } else {
                            self.state
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .context
                                .push((definition.id.clone(), context));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        outcome.response = None;
                        outcome.failure = Some(error);
                        outcome.status = HookStatus::Failed;
                    }
                }
            }
            let blocked = outcome.blocks(definition);
            guard.finish(&outcome);
            if deadline.is_none()
                && (self.stop.load(Ordering::Acquire) || self.settled.load(Ordering::Acquire))
            {
                return true;
            }
            if blocked {
                if matches!(
                    payload.event(),
                    HookEvent::SessionStart
                        | HookEvent::UserPrompt
                        | HookEvent::RunStart
                        | HookEvent::BeforeModel
                        | HookEvent::InvalidToolCall
                ) {
                    self.blocked.store(true, Ordering::Release);
                }
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;

struct ActivityGuard {
    sink: Option<Arc<dyn HookActivitySink>>,
    activity: Option<HookActivity>,
    start: std::time::Instant,
}
impl ActivityGuard {
    fn new(sink: Option<Arc<dyn HookActivitySink>>, activity: HookActivity) -> Self {
        if let Some(sink) = &sink {
            sink.record(activity.clone());
        }
        Self {
            sink,
            activity: Some(activity),
            start: std::time::Instant::now(),
        }
    }
    fn finish(&mut self, outcome: &HookOutcome) {
        if let Some(mut activity) = self.activity.take() {
            activity.status = outcome.status;
            activity.duration_ms = outcome.duration_ms;
            activity.exit_code = outcome.exit_code;
            if let Some(sink) = &self.sink {
                sink.record(activity);
            }
        }
    }
}
impl Drop for ActivityGuard {
    fn drop(&mut self) {
        if let Some(mut activity) = self.activity.take() {
            activity.status = HookStatus::Cancelled;
            activity.duration_ms = self.start.elapsed().as_millis().min(u64::MAX as u128) as u64;
            if let Some(sink) = &self.sink {
                sink.record(activity);
            }
        }
    }
}

/// Guarantees host settlement even when an outer task drops the Rig stream.
pub struct HookSettlementGuard {
    run: Option<Arc<HookRun>>,
}
impl HookSettlementGuard {
    pub fn new(run: Arc<HookRun>) -> Self {
        Self { run: Some(run) }
    }
    pub async fn finish(mut self, status: HookRunStatus, background: bool) {
        if let Some(run) = self.run.take() {
            if background {
                tokio::spawn(async move {
                    run.settle(status).await;
                });
            } else {
                run.settle(status).await;
            }
        }
    }
}
impl Drop for HookSettlementGuard {
    fn drop(&mut self) {
        if let Some(run) = self.run.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    run.settle(HookRunStatus::Cancelled).await;
                });
            }
        }
    }
}
