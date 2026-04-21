//! # Zero Middleware — compatibility re-export
//!
//! The middleware pipeline has one canonical implementation in
//! [`agent_runtime::middleware`]. Historically `framework/zero-middleware/`
//! carried a byte-forked copy that drifted behind the runtime version
//! (the runtime gained skill-aware context editing, extra trigger
//! conditions, and `ExecutionState` while the framework copy stagnated).
//! Every production call site — `gateway`, `gateway-execution`, the
//! executor in `agent_runtime` itself — imports from the runtime module
//! directly, so the framework copy had no live consumers.
//!
//! Rather than keep ~600 lines of duplicate implementation in sync, this
//! crate is now a pure re-export of the runtime module. The public
//! surface (`zero_middleware::MiddlewarePipeline`, …) is preserved for
//! any future `zero-agent-framework` consumer that expects the framework
//! import path, but every type actually resolves to the single canonical
//! definition under `agent_runtime::middleware`.

pub use agent_runtime::middleware::{
    // Sub-module namespaces — preserves `zero_middleware::traits::...`
    // paths used in some historical doc comments.
    config,
    context_editing,
    pipeline,
    summarization,
    token_counter,
    traits,
    // Flat re-exports — this mirrors what `framework/zero-app::prelude`
    // re-exports from this crate, so external consumers keep working.
    ContextEditingConfig,
    ContextEditingMiddleware,
    EventMiddleware,
    KeepPolicy,
    MiddlewareConfig,
    MiddlewareContext,
    MiddlewareEffect,
    MiddlewarePipeline,
    PreProcessMiddleware,
    SummarizationConfig,
    SummarizationMiddleware,
    TriggerCondition,
};
