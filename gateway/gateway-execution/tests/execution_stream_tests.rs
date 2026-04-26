//! ExecutionStream — per-execution event loop. Tests focus on the
//! lifecycle wiring (struct construction, type-level deps) without
//! exercising the full executor pipeline (which needs an LlmClient).
//! End-to-end coverage lives in the e2e suite.
//!
//! Run with:
//!   cargo test -p gateway-execution --test execution_stream_tests --features test-stubs

#![cfg(feature = "test-stubs")]

use std::collections::HashMap;
use std::sync::Arc;

use api_logs::LogService;
use execution_state::StateService;
use gateway_database::{ConversationRepository, DatabaseManager};
use gateway_events::EventBus;
use gateway_execution::delegation::DelegationRegistry;
use gateway_execution::runner::ExecutionStream;
use gateway_services::VaultPaths;
use tokio::sync::{mpsc, RwLock};

#[allow(deprecated)]
#[test]
fn execution_stream_constructs_with_minimum_required_deps() {
    // The compile of this test IS the assertion: ExecutionStream must
    // accept None for every Option<…> field and valid Arc for required fields.
    #[allow(deprecated)]
    let dir = tempfile::tempdir().unwrap();
    #[allow(deprecated)]
    let path = dir.into_path();
    let paths = Arc::new(VaultPaths::new(path.clone()));
    let db = Arc::new(DatabaseManager::new(paths.clone()).unwrap());
    let state = Arc::new(StateService::new(db.clone()));
    let logs = Arc::new(LogService::new(db.clone()));
    let convo = Arc::new(ConversationRepository::new(db));
    let bus = Arc::new(EventBus::new());
    let (tx, _rx) = mpsc::unbounded_channel();
    let delegation_registry = Arc::new(DelegationRegistry::new());
    let handles = Arc::new(RwLock::new(HashMap::new()));

    let _ = ExecutionStream {
        event_bus: bus,
        state_service: state,
        log_service: logs,
        conversation_repo: convo,
        delegation_tx: tx,
        delegation_registry,
        handles,
        distiller: None,
        kg_episode_repo: None,
        graph_storage: None,
        paths,
        memory_repo: None,
        connector_registry: None,
        bridge_registry: None,
        bridge_outbox: None,
    };
}
