//! HandoffWriter — moved to `gateway_memory::sleep::handoff_writer`.
//!
//! This module is a pure re-export shim. The engine struct, the `HandoffLlm`
//! trait, the production `LlmHandoffWriter`, and all helper functions now
//! live in `gateway-memory`. Callers that previously imported from
//! `gateway_execution::sleep::handoff_writer::*` keep working.

pub use gateway_memory::sleep::handoff_writer::*;
