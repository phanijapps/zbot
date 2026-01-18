# zero-tool

Tool definitions and abstractions for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Code Style

- Tools implement the `Tool` trait from `zero-core`
- Use `serde_json::Value` for flexible parameter handling
- Return structured results as `Value` (can be objects, strings, arrays)
- Use `thiserror` for error conversion

## Tool Trait

The `Tool` trait is defined in `zero-core` and implemented here:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<Value>;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}
```

## Parameter Schema

Use JSON Schema for parameters:

```rust
serde_json::json!({
    "type": "object",
    "properties": {
        "param1": {
            "type": "string",
            "description": "Parameter description"
        }
    },
    "required": ["param1"]
})
```

## ToolContext

The context (from `zero-core`) provides:
- `conversation_id()` - Current conversation ID for scoping
- Access to state through the context interface

## Testing

Test with various argument shapes (valid, missing required, wrong types).

## Important Notes

- Always validate required parameters and return descriptive errors
- Use `args.get("key")` for optional parameters
- Use `and_then(|v| v.as_str())` for type-safe string extraction
- This crate defines the abstraction only - see `agent-tools` for concrete tool implementations
