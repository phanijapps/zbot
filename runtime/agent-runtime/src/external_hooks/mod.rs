//! File-configured operator hooks. Configuration is frozen per accepted invocation.
mod config;
pub use config::*;
mod process;
mod projection;
mod protocol;
#[cfg(target_os = "linux")]
mod trust;
pub use process::invoke_hook;
pub use projection::{project_arguments, ProjectedArguments};
pub use protocol::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStatus {
    Running,
    Completed,
    Blocked,
    Failed,
    Timeout,
    Cancelled,
    Skipped,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HookFailure {
    #[error("Hook event is invalid")]
    InvalidEvent,
    #[error("Hook event exceeds its byte limit")]
    InputTooLarge,
    #[error("Hook command is not a trusted operator entry point")]
    UntrustedProgram,
    #[error("Hook execution is unsupported on this host")]
    UnsupportedHost,
    #[error("Hook process could not execute")]
    Process,
    #[error("Hook output exceeds its byte limit")]
    OutputTooLarge,
    #[error("Hook response is invalid")]
    InvalidResponse,
    #[error("Hook context exceeds its invocation byte limit")]
    ContextTooLarge,
    #[error("Hook context checkpoint failed")]
    ContextCheckpoint,
    #[error("Hook process timed out")]
    Timeout,
    #[error("Hook process was cancelled")]
    Cancelled,
    #[error("Hook command exited unsuccessfully")]
    NonZeroExit,
}
#[derive(Debug)]
pub struct HookOutcome {
    pub status: HookStatus,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub response: Option<HookResponse>,
    pub failure: Option<HookFailure>,
}

impl HookOutcome {
    /// Failure policy can only stop a supported pre-action event, never restore authorization.
    pub fn blocks(&self, definition: &HookDefinition) -> bool {
        self.status == HookStatus::Blocked
            || (self.failure.is_some()
                && self.failure != Some(HookFailure::Cancelled)
                && definition.on_failure == HookFailurePolicy::Block
                && definition.event.allows_block())
    }
}
mod lifecycle;
pub use lifecycle::{
    HookActivity, HookActivitySink, HookContextCheckpoint, HookInvocation, HookInvocationConfig,
    HookRun, HookSettlementGuard,
};

mod model;
pub use model::HookedLlmClient;
