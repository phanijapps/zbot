# Rust Code Quality Rules

## Formatting
- Run `cargo fmt --all` before committing any Rust code
- Never push code that fails `cargo fmt --all --check`

## Clippy
- All code must pass `cargo clippy --all-targets -- -D warnings`
- If a lint must be suppressed, use `#[allow(clippy::rule_name)]` with a comment explaining why
- Never suppress at the crate level without team discussion

## Cognitive Complexity
- Keep function cognitive complexity under 15
- If a function has a large match/if-else chain, extract each branch into a named helper function
- Prefer early returns over deep nesting
- Long closures should be extracted into named functions

## Type Safety
- No unnecessary `as` casts (e.g., `x as usize` when `x` is already `usize`)
- Prefer `TryFrom`/`TryInto` over `as` for fallible conversions
- Use `#[allow(clippy::cast_sign_loss)]` only when the cast is provably safe, with a comment

## Error Handling
- No `unwrap()` in production code — use `?`, `unwrap_or`, `unwrap_or_else`, or `unwrap_or_default`
- `unwrap()` is acceptable in tests and setup code
- Use meaningful error messages with `map_err`

## Dependencies
- Keep `Cargo.lock` committed
- Run `cargo audit` periodically to check for known vulnerabilities
