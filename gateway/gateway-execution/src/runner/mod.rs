//! # Runner
//!
//! Session orchestration. Decomposed from a 3,067-LOC god module into
//! six focused units. **Read `AGENTS.md` in this directory before
//! adding code here.**

mod continuation_watcher;
pub(super) mod core;
mod delegation_dispatcher;
mod execution_stream;
mod invoke_bootstrap;
mod session_invoker;

pub use continuation_watcher::ContinuationWatcher;
pub use core::*;
pub use delegation_dispatcher::DelegationDispatcher;
#[cfg(any(test, feature = "test-stubs"))]
pub use session_invoker::StubSessionInvoker;
pub use session_invoker::{ContinuationSpawner, DelegationSpawner, SessionSpawner};
