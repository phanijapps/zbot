# zero-prompt

Prompt template and management for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Code Style

- Use `{{ variable }}` syntax for template variables
- Templates are compiled to regex for efficient substitution
- Keep templates simple and focused

## Prompt Templates

Templates allow variable substitution:

```rust
let template = PromptTemplate::new("Hello, {{ name }}!");
let result = template.render(vec![("name", "World")])?;
// Result: "Hello, World!"
```

## Usage

Used primarily for system instructions and reusable prompt fragments.

## Testing

Test variable substitution, missing variables, and complex templates.

## Important Notes

- Variables are replaced sequentially
- Use descriptive variable names
- Templates should be immutable after creation
