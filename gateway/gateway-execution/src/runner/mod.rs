//! # Runner
//!
//! Session orchestration. Decomposed from a 3,067-LOC god module into
//! five focused units. **Read `AGENTS.md` in this directory before
//! adding code here.** (AGENTS.md is added in Task 6.)

mod continuation_watcher;
pub(super) mod core;
mod delegation_dispatcher;
mod execution_stream;
mod session_invoker;

pub use continuation_watcher::ContinuationWatcher;
pub use core::*;
pub use delegation_dispatcher::DelegationDispatcher;
pub use execution_stream::{ExecutionContext, ExecutionStream};
pub use session_invoker::SessionInvoker;
#[cfg(any(test, feature = "test-stubs"))]
pub use session_invoker::StubSessionInvoker;
