//! # SessionInvoker
//!
//! 1-method trait that lets runner handlers spawn a session without
//! depending on `Arc<ExecutionRunner>`. Keeps each handler's
//! dependency manifest honest and makes handlers testable with
//! `StubSessionInvoker`.

use crate::config::ExecutionConfig;
use async_trait::async_trait;

#[async_trait]
pub trait SessionInvoker: Send + Sync {
    /// Spawn (or resume) a session. Wraps whatever the runner needs to
    /// do internally — handler callers pass config + message and don't
    /// see the bootstrap → stream pipeline.
    async fn spawn_session(&self, config: ExecutionConfig, message: String) -> Result<(), String>;
}

/// Test-only impl that records every call. Handlers under test inject
/// this instead of the real `ExecutionRunner` so loop logic can be
/// exercised without booting the executor pipeline.
#[cfg(any(test, feature = "test-stubs"))]
pub struct StubSessionInvoker {
    pub calls: std::sync::Mutex<Vec<(ExecutionConfig, String)>>,
}

#[cfg(any(test, feature = "test-stubs"))]
impl StubSessionInvoker {
    pub fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
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
}

// The real impl for `ExecutionRunner` lives below — wraps existing invoke().
use super::core::ExecutionRunner;

#[async_trait]
impl SessionInvoker for ExecutionRunner {
    async fn spawn_session(&self, config: ExecutionConfig, message: String) -> Result<(), String> {
        self.invoke(config, message).await.map(|_| ())
    }
}
