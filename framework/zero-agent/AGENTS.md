# zero-agent

Agent implementations for the Zero framework.

## Modules

| Module | Purpose |
|--------|---------|
| `llm_agent` | `LlmAgent` — primary LLM loop agent |
| `workflow` | Composable workflow agents |
| `orchestrator` | `OrchestratorAgent` — task-graph based multi-agent orchestration |

## Key Types

```rust
pub use llm_agent::{LlmAgent, LlmAgentBuilder};
pub use workflow::{ConditionalAgent, CustomAgent, CustomAgentBuilder, LlmConditionalAgent,
    LlmConditionalAgentBuilder, LoopAgent, ParallelAgent, SequentialAgent};
pub use orchestrator::{ExecutionTrace, OrchestratorAgent, OrchestratorBuilder, OrchestratorConfig,
    TaskGraph, TaskNode, TaskStatus, TraceEvent, TraceEventKind};
```

## LlmAgent

Primary LLM-based agent: builds request from session history, calls LLM, executes tool calls, repeats until turn complete.

```rust
let agent = LlmAgent::builder("my-agent", llm)
    .system_instruction("You are helpful.")
    .with_tools(tools)
    .build();
```

## Workflow Agents

| Type | Behavior |
|------|----------|
| `SequentialAgent` | Run sub-agents in sequence |
| `ParallelAgent` | Run sub-agents concurrently |
| `LoopAgent` | Repeat an agent N times |
| `ConditionalAgent` | Branch on a Rust predicate |
| `LlmConditionalAgent` | Branch using an LLM decision |
| `CustomAgent` | Closure-based custom behavior |

## OrchestratorAgent

Task-graph orchestrator: builds a `TaskGraph` of `TaskNode`s, executes them respecting dependencies, collects `ExecutionTrace`.

## Intra-Repo Dependencies

- `zero-core` — `Agent`, `BeforeAgentCallback`, `AfterAgentCallback`
- `zero-llm` — `Llm` trait (used by `LlmAgent`)
- `zero-session` — `Session`, `State` (for context)

## Notes

- Re-exports `AfterAgentCallback` and `BeforeAgentCallback` from `zero-core`.
- Agents are async-first; use `tokio::test` for unit tests.
- Tool responses must be appended back to the session for correct LLM context.
