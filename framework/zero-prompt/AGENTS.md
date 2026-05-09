# zero-prompt

Prompt template rendering with session state injection for the Zero framework.

## Key Exports

```rust
pub use template::{inject_session_state, Template, TemplateRenderer};
pub use error::{PromptError, Result};
```

## Placeholder Syntax

Templates use `{variable}` (single braces, not double):
- `{var_name}` — required variable; error if not found in context state
- `{var_name?}` — optional variable; replaced with empty string if missing
- `{artifact.file_name}` — artifact reference (optional, resolves to empty if absent)
- State-scoped vars: `{app:key}`, `{user:key}`, `{temp:key}`

## Core Functions

```rust
// Inject session state values into a template string
pub async fn inject_session_state(
    template: &str,
    ctx: &dyn CallbackContext,
) -> Result<String>;
```

`Template` and `TemplateRenderer` wrap this for reuse across multiple renders.

## Modules

| File | Purpose |
|------|---------|
| `template.rs` | `Template`, `TemplateRenderer`, `inject_session_state()` |
| `error.rs` | `PromptError` (VariableNotFound, InvalidName), `Result` |

## Intra-Repo Dependencies

- `zero-core` — `CallbackContext` for state lookup

## Notes

- Placeholder regex: `\{[^{}]*\??\}` — matches `{...}` with optional trailing `?`
- Variable names must be valid identifiers (letters/digits/underscore, start with letter or `_`).
- Used by the gateway to inject state into system prompt templates.
