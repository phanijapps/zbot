//! Per-call host outcomes retained until Rig publishes its settled tool batch.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Default)]
pub(super) struct ToolOutcome {
    pub raw: Option<String>,
    pub error: Option<String>,
    pub duration_ms: i64,
    pub context: Option<String>,
    pub actions: agent_primitives::EventActions,
    pub rejected_call: Option<(String, serde_json::Value)>,
}

#[derive(Default)]
pub(super) struct ToolResults {
    outcome: Mutex<std::collections::HashMap<String, ToolOutcome>>,
    terminal: AtomicBool,
    peer_influenced: AtomicBool,
}

pub(super) type SharedToolResults = Arc<ToolResults>;

impl ToolResults {
    pub fn peer_influenced(&self) -> bool {
        self.peer_influenced.load(Ordering::Acquire)
    }

    pub fn mark_peer_influenced(&self) {
        self.peer_influenced.store(true, Ordering::Release);
    }
    pub fn terminal(&self) -> bool {
        self.terminal.load(Ordering::Acquire)
    }
    pub fn mark_terminal(&self) {
        self.terminal.store(true, Ordering::Release);
    }
    pub fn record(&self, id: &str, outcome: ToolOutcome) {
        self.outcome.lock().unwrap().insert(id.to_owned(), outcome);
    }
    pub fn snapshot(&self, id: &str) -> ToolOutcome {
        self.outcome
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or_default()
    }
    pub fn take(&self, id: &str) -> ToolOutcome {
        self.outcome.lock().unwrap().remove(id).unwrap_or_default()
    }
}
