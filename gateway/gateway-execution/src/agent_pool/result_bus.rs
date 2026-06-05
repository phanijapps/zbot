use crate::handle::ExecutionHandle;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct AgentResult {
    pub execution_id: String,
    pub agent_id: String,
    pub response: String,
}

#[derive(Debug)]
pub enum AgentWaitError {
    Timeout,
    Crashed { error: String },
    NotFound(String),
}

/// In-process bus that connects `wait_agent` (the blocker) with
/// `handle_execution_success/failure` (the resolvers).
///
/// One-shot per execution_id: resolve/reject fire once and clean up.
/// Also tracks execution handles so `kill_agent` can stop a running subagent.
pub struct AgentResultBus {
    waiting: Mutex<HashMap<String, oneshot::Sender<Result<AgentResult, AgentWaitError>>>>,
    execution_handles: Mutex<HashMap<String, ExecutionHandle>>,
}

impl Default for AgentResultBus {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentResultBus {
    pub fn new() -> Self {
        Self {
            waiting: Mutex::new(HashMap::new()),
            execution_handles: Mutex::new(HashMap::new()),
        }
    }

    /// Register an execution handle keyed by execution_id for `kill_agent`.
    pub fn register_handle(&self, execution_id: &str, handle: ExecutionHandle) {
        self.execution_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(execution_id.to_string(), handle);
    }

    /// Register a waiter. Returns the receiver that `wait_agent` will `.await`.
    pub fn register_waiter(
        &self,
        execution_id: &str,
    ) -> oneshot::Receiver<Result<AgentResult, AgentWaitError>> {
        let (tx, rx) = oneshot::channel();
        self.waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(execution_id.to_string(), tx);
        rx
    }

    /// Called by handle_execution_success — unblocks any pending wait_agent.
    pub fn resolve(&self, execution_id: &str, agent_id: &str, response: &str) {
        let tx = self
            .waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(execution_id);
        if let Some(tx) = tx {
            let _ = tx.send(Ok(AgentResult {
                execution_id: execution_id.to_string(),
                agent_id: agent_id.to_string(),
                response: response.to_string(),
            }));
        }
        self.execution_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(execution_id);
    }

    /// Called by handle_execution_failure — unblocks any pending wait_agent with an error.
    pub fn reject(&self, execution_id: &str, error: AgentWaitError) {
        let tx = self
            .waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(execution_id);
        if let Some(tx) = tx {
            let _ = tx.send(Err(error));
        }
        self.execution_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(execution_id);
    }

    /// Stop the execution and unblock any waiting `wait_agent` with a killed error.
    /// Returns true if a registered handle was found and stopped.
    pub fn kill(&self, execution_id: &str) -> bool {
        let handle = self
            .execution_handles
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(execution_id);
        let had_handle = handle.is_some();
        if let Some(h) = handle {
            h.stop();
        }
        let tx = self
            .waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(execution_id);
        if let Some(tx) = tx {
            let _ = tx.send(Err(AgentWaitError::Crashed {
                error: "killed by orchestrator".to_string(),
            }));
        }
        had_handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::ExecutionHandle;
    use std::sync::Arc;

    #[tokio::test]
    async fn resolve_unblocks_pending_waiter() {
        let bus = AgentResultBus::new();
        let rx = bus.register_waiter("exec-1");
        bus.resolve("exec-1", "researcher", "found 5 sources");
        let result = rx.await.expect("channel open").expect("ok variant");
        assert_eq!(result.execution_id, "exec-1");
        assert_eq!(result.agent_id, "researcher");
        assert_eq!(result.response, "found 5 sources");
    }

    #[tokio::test]
    async fn reject_unblocks_pending_waiter_with_crashed_error() {
        let bus = AgentResultBus::new();
        let rx = bus.register_waiter("exec-2");
        bus.reject(
            "exec-2",
            AgentWaitError::Crashed {
                error: "shell failed".into(),
            },
        );
        let err = rx.await.expect("channel open").expect_err("err variant");
        match err {
            AgentWaitError::Crashed { error } => assert_eq!(error, "shell failed"),
            other => panic!("expected Crashed, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn resolve_without_waiter_is_a_noop() {
        // Fast-path case from the design doc: agent finished before any
        // wait_agent registered. resolve drops silently rather than
        // panicking; the wait_agent fast-path reads the result from the
        // StateService instead.
        let bus = AgentResultBus::new();
        bus.resolve("exec-no-waiter", "x", "ignored");
        // No assertion needed — must simply not panic. Re-registering
        // afterwards should work cleanly (state was cleaned up).
        let _rx = bus.register_waiter("exec-no-waiter");
    }

    #[test]
    fn reject_without_waiter_is_a_noop() {
        let bus = AgentResultBus::new();
        bus.reject(
            "exec-no-waiter",
            AgentWaitError::Crashed { error: "x".into() },
        );
    }

    #[tokio::test]
    async fn second_register_replaces_first_waiter_silently() {
        // Two consecutive register_waiter calls on the same execution_id
        // can happen if a tool retry hits the bus before the first
        // resolve. The second call's receiver wins; the first's is
        // dropped (its sender is replaced, sender.send → Err on
        // closed receiver, but we never send on the dropped sender).
        let bus = AgentResultBus::new();
        let _rx1 = bus.register_waiter("exec-3");
        let rx2 = bus.register_waiter("exec-3");
        bus.resolve("exec-3", "writer", "done");
        let r = rx2.await.expect("channel open").expect("ok");
        assert_eq!(r.response, "done");
    }

    #[test]
    fn kill_stops_handle_and_returns_true() {
        let bus = AgentResultBus::new();
        let handle = ExecutionHandle::new(10);
        bus.register_handle("exec-4", handle.clone());
        assert!(!handle.is_stop_requested());

        let killed = bus.kill("exec-4");
        assert!(killed, "kill should return true when a handle was present");
        assert!(
            handle.is_stop_requested(),
            "kill must trigger handle.stop()"
        );
    }

    #[test]
    fn kill_returns_false_when_no_handle_registered() {
        let bus = AgentResultBus::new();
        let killed = bus.kill("exec-unknown");
        assert!(!killed);
    }

    #[tokio::test]
    async fn kill_unblocks_pending_waiter_with_crashed_error() {
        let bus = AgentResultBus::new();
        let handle = ExecutionHandle::new(10);
        bus.register_handle("exec-5", handle);
        let rx = bus.register_waiter("exec-5");

        bus.kill("exec-5");

        let err = rx.await.expect("channel open").expect_err("err variant");
        match err {
            AgentWaitError::Crashed { error } => {
                assert!(error.contains("killed by orchestrator"));
            }
            _ => panic!("expected Crashed variant from kill"),
        }
    }

    #[test]
    fn resolve_cleans_up_execution_handles_map() {
        // After resolve, kill on the same execution_id should return
        // false — the handle entry was removed.
        let bus = AgentResultBus::new();
        bus.register_handle("exec-6", ExecutionHandle::new(10));
        bus.resolve("exec-6", "x", "y");
        assert!(!bus.kill("exec-6"));
    }

    #[test]
    fn reject_cleans_up_execution_handles_map() {
        let bus = AgentResultBus::new();
        bus.register_handle("exec-7", ExecutionHandle::new(10));
        bus.reject("exec-7", AgentWaitError::Timeout);
        assert!(!bus.kill("exec-7"));
    }

    #[tokio::test]
    async fn many_concurrent_waiters_on_distinct_ids_resolve_independently() {
        // Confirms there's no cross-talk between execution_ids when
        // multiple waiters are outstanding.
        let bus = Arc::new(AgentResultBus::new());
        let rx1 = bus.register_waiter("a");
        let rx2 = bus.register_waiter("b");
        let rx3 = bus.register_waiter("c");

        bus.resolve("b", "ag-b", "from-b");
        bus.resolve("a", "ag-a", "from-a");
        bus.resolve("c", "ag-c", "from-c");

        let r1 = rx1.await.unwrap().unwrap();
        let r2 = rx2.await.unwrap().unwrap();
        let r3 = rx3.await.unwrap().unwrap();
        assert_eq!(r1.response, "from-a");
        assert_eq!(r2.response, "from-b");
        assert_eq!(r3.response, "from-c");
    }
}
