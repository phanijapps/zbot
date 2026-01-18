# zero-middleware

Middleware and request/response processing for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Code Style

- Middleware processes requests/respects in a chain
- Use `async_trait` for middleware trait
- Each middleware should pass to the next in chain

## Middleware Pattern

Middleware wraps agent execution to add cross-cutting concerns:
- Logging
- Error handling
- Request transformation
- Response filtering

## Implementation

Middleware typically:
1. Receives the invocation context
2. Performs pre-processing
3. Calls the next middleware/agent
4. Performs post-processing on the result

## Testing

Mock the agent call to test middleware in isolation.

## Important Notes

- Order of middleware matters in the chain
- Always propagate to next for normal flow
- Return early to short-circuit (e.g., caching, auth failures)
