# zero-tool

Tool registry and concrete tool infrastructure for the Zero framework.

## What It Provides

- `ToolRegistry` — in-memory store of `Arc<dyn Tool>` instances, keyed by name
- `ToolContextImpl` — concrete implementation of `zero_core::ToolContext`
- `FunctionTool` — wraps a Rust closure as a `Tool`

## Key Types (re-exported from zero-core)

```rust
pub use zero_core::{Tool, ToolContext, Toolset};
```

## Tool Trait (defined in zero-core)

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value>;    // JSON Schema for LLM
    fn response_schema(&self) -> Option<Value>;      // optional
    fn permissions(&self) -> ToolPermissions;        // risk level, capabilities
    fn validate(&self, args: &Value) -> Result<()>;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}
```

## Modules

| Module | Purpose |
|--------|---------|
| `registry` | `ToolRegistry` — stores and looks up tools by name |
| `function` | `FunctionTool` — closure-based tool adapter |
| `context_impl` | `ToolContextImpl` — runtime `ToolContext` implementation |

## Intra-Repo Dependencies

- `zero-core` — `Tool`, `ToolContext`, `Toolset` traits

## Notes

- This crate provides registry infrastructure only.
- Concrete tool implementations (shell, file, memory, etc.) live in `runtime/agent-tools`.
- Use `ToolRegistry::register()` to add tools and `ToolRegistry::get()` to look them up.
