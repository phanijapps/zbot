# zero-agent

Agent implementations for the Zero framework.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo test
```

## Code Style

- Use builder pattern for agent configuration
- Loop on tool calls until `turn_complete == true`
- Always add tool responses back to the session
- Use `MutexSession` wrapper for thread-safe session access

## Agent Types

### LlmAgent

The primary LLM-based agent that:
1. Builds request from session history + system instruction
2. Calls LLM
3. Executes tools if present
4. Repeats until turn is complete
5. Returns final response

**Builder pattern:**
```rust
let agent = LlmAgent::builder()
    .with_llm(llm)
    .with_session(session)
    .with_tools(tools)
    .with_system_instruction(instruction)
    .build();
```

### Workflow Agents

- `SequentialAgent` - Run agents in sequence
- `ParallelAgent` - Run agents in parallel
- `LoopAgent` - Repeat an agent N times
- `ConditionalAgent` - Branch based on predicate
- `LlmConditionalAgent` - Use LLM to decide branch
- `CustomAgent` - Define custom agent behavior

## Session Management

The agent appends each exchange to the session:
1. User/assistant message before LLM call
2. Tool call event after detecting tool calls
3. Tool response events after execution
4. Final assistant message when complete

## Testing

Use `tokio::test` for async tests. Mock the LLM trait for unit tests.

## Important Notes

- Always check for duplicate content before adding to request (see `build_request()`)
- Tool responses must be added back to session for context
- Conversation ID should be in ToolContext for conversation-scoped operations
- Use `turn_complete` flag to determine when to stop looping
