use crate::handle::ExecutionHandle;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

pub struct AgentResult {
    pub execution_id: String,
    pub agent_id: String,
    pub response: String,
}

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
            .unwrap()
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
            .unwrap()
            .insert(execution_id.to_string(), tx);
        rx
    }

    /// Called by handle_execution_success — unblocks any pending wait_agent.
    pub fn resolve(&self, execution_id: &str, agent_id: &str, response: &str) {
        let tx = self.waiting.lock().unwrap().remove(execution_id);
        if let Some(tx) = tx {
            let _ = tx.send(Ok(AgentResult {
                execution_id: execution_id.to_string(),
                agent_id: agent_id.to_string(),
                response: response.to_string(),
            }));
        }
        self.execution_handles.lock().unwrap().remove(execution_id);
    }

    /// Called by handle_execution_failure — unblocks any pending wait_agent with an error.
    pub fn reject(&self, execution_id: &str, error: AgentWaitError) {
        let tx = self.waiting.lock().unwrap().remove(execution_id);
        if let Some(tx) = tx {
            let _ = tx.send(Err(error));
        }
        self.execution_handles.lock().unwrap().remove(execution_id);
    }

    /// Stop the execution and unblock any waiting `wait_agent` with a killed error.
    /// Returns true if a registered handle was found and stopped.
    pub fn kill(&self, execution_id: &str) -> bool {
        let handle = self.execution_handles.lock().unwrap().remove(execution_id);
        let had_handle = handle.is_some();
        if let Some(h) = handle {
            h.stop();
        }
        let tx = self.waiting.lock().unwrap().remove(execution_id);
        if let Some(tx) = tx {
            let _ = tx.send(Err(AgentWaitError::Crashed {
                error: "killed by orchestrator".to_string(),
            }));
        }
        had_handle
    }
}
