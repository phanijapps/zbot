//! # SessionInvoker
//!
//! Narrow seam runner handlers depend on instead of holding
//! `Arc<ExecutionRunner>`. Two methods, one per spawn shape:
//!
//! - `spawn_session` — fresh session (used by DelegationDispatcher
//!   and direct API callers).
//! - `spawn_continuation` — resume an existing root execution
//!   (used by ContinuationWatcher). Routes to `invoke_continuation`
//!   on the real impl, which reactivates the session, loads
//!   history, prepends recall, builds the continuation message
//!   with plan injection, and runs the re-delegation + distillation
//!   tail-effects.

use crate::config::ExecutionConfig;
use async_trait::async_trait;
#[cfg(any(test, feature = "test-stubs"))]
use std::sync::Mutex;

#[async_trait]
pub trait SessionInvoker: Send + Sync {
    /// Spawn (or resume) a session. Wraps whatever the runner needs to
    /// do internally — handler callers pass config + message and don't
    /// see the bootstrap → stream pipeline.
    async fn spawn_session(&self, config: ExecutionConfig, message: String) -> Result<(), String>;

    /// Resume an existing root execution. The handler does NOT pass a
    /// message — the impl loads history, prepends recall, and builds
    /// the continuation message itself (preserves the legacy
    /// `invoke_continuation` flow).
    async fn spawn_continuation(
        &self,
        session_id: String,
        root_agent_id: String,
    ) -> Result<(), String>;
}

/// Test-only impl that records every call. Handlers under test inject
/// this instead of the real `ExecutionRunner` so loop logic can be
/// exercised without booting the executor pipeline.
#[cfg(any(test, feature = "test-stubs"))]
pub struct StubSessionInvoker {
    pub calls: Mutex<Vec<(ExecutionConfig, String)>>,
    pub continuation_calls: Mutex<Vec<(String, String)>>, // (session_id, root_agent_id)
}

#[cfg(any(test, feature = "test-stubs"))]
impl StubSessionInvoker {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            continuation_calls: Mutex::new(Vec::new()),
        }
    }
}

#[cfg(any(test, feature = "test-stubs"))]
impl Default for StubSessionInvoker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-stubs"))]
#[async_trait]
impl SessionInvoker for StubSessionInvoker {
    async fn spawn_session(&self, config: ExecutionConfig, message: String) -> Result<(), String> {
        self.calls.lock().unwrap().push((config, message));
        Ok(())
    }

    async fn spawn_continuation(
        &self,
        session_id: String,
        root_agent_id: String,
    ) -> Result<(), String> {
        self.continuation_calls
            .lock()
            .unwrap()
            .push((session_id, root_agent_id));
        Ok(())
    }
}

// The real impl for `ExecutionRunner` lives below — wraps existing invoke().
use super::core::ExecutionRunner;

#[async_trait]
impl SessionInvoker for ExecutionRunner {
    async fn spawn_session(&self, config: ExecutionConfig, message: String) -> Result<(), String> {
        self.invoke(config, message).await.map(|_| ())
    }

    async fn spawn_continuation(
        &self,
        session_id: String,
        root_agent_id: String,
    ) -> Result<(), String> {
        self.make_continuation_invoker()
            .spawn_continuation(session_id, root_agent_id)
            .await
    }
}
