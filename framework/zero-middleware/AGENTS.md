# zero-middleware

Compatibility re-export shim — not an implementation crate.

## Purpose

All middleware logic lives in `agent_runtime::middleware` (the canonical implementation).
This crate re-exports it wholesale so that the `framework/` import path
(`zero_middleware::MiddlewarePipeline`, etc.) remains stable for future
`zero-agent-framework` consumers.

**Do not add middleware logic here.** Add it in `runtime/agent-runtime/src/middleware/` instead.

## Re-exported Types

```rust
pub use agent_runtime::middleware::{
    config, context_editing, pipeline, summarization, token_counter, traits,
    ContextEditingConfig, ContextEditingMiddleware, EventMiddleware, KeepPolicy,
    MiddlewareConfig, MiddlewareContext, MiddlewareEffect, MiddlewarePipeline,
    PreProcessMiddleware, SummarizationConfig, SummarizationMiddleware, TriggerCondition,
};
```

## Intra-Repo Dependencies

- `agent-runtime` — provides the real middleware implementation

## Where the Logic Lives

| Module | File in agent-runtime |
|--------|----------------------|
| `pipeline` | `middleware/pipeline.rs` |
| `summarization` | `middleware/summarization.rs` |
| `context_editing` | `middleware/context_editing.rs` |
| `token_counter` | `middleware/token_counter.rs` |
| `traits` | `middleware/traits.rs` |
