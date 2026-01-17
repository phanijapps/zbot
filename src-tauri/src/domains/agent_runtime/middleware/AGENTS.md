# Middleware Module

## Overview

This module provides a modular middleware system for agent execution, inspired by [LangChain JS middleware](https://docs.langchain.com/oss/javascript/langchain/middleware/built-in). Middleware intercepts the agent execution flow to transform messages, manage context, and react to events.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Agent Executor                          │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              Middleware Pipeline                           │ │
│  │                                                             │ │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │ │
│  │  │             │    │             │    │             │    │ │
│  │  │   Summary-  │    │  Context    │    │   Custom    │    │ │
│  │  │   ization   │───▶│   Editing   │───▶│  Middleware │    │ │
│  │  │             │    │             │    │             │    │ │
│  │  └─────────────┘    └─────────────┘    └─────────────┘    │ │
│  │                                                             │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                  │                              │
│                                  ▼                              │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    LLM Execution                           │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Storage Structure

Middleware configuration is stored in the agent's `config.yaml`:

```yaml
# ~/.config/zeroagent/agents/{agent-name}/config.yaml

name: my-agent
displayName: My Agent
# ... other config ...

middleware: |
  middleware:
    summarization:
      enabled: true
      trigger:
        tokens: 60000
      keep:
        messages: 6

    context_editing:
      enabled: true
      trigger_tokens: 60000
      keep_tool_results: 10
```

## Key Concepts

### Pre-Process Middleware

Middleware that runs **before** the LLM call, transforming or filtering messages:

- **Summarization**: Compresses conversation history when approaching token limits
- **Context Editing**: Clears older tool call outputs while keeping recent ones
- **Custom**: Any transformation (filtering, validation, etc.)

### Event Middleware

Middleware that reacts to events **during** execution:

- **Logging**: Log all tokens and events
- **Metrics**: Track token usage, response times
- **Rate Limiting**: Throttle expensive operations
- **PII Detection**: Redact sensitive information

### Middleware Effects

Middleware returns effects that control the flow:

```rust
pub enum MiddlewareEffect {
    ModifiedMessages(Vec<ChatMessage>),    // Transform messages
    Proceed,                                // Continue without changes
    EmitEvent(StreamEvent),                 // Emit event only
    EmitAndModify {                         // Both emit and transform
        event: StreamEvent,
        messages: Vec<ChatMessage>,
    },
}
```

## Middleware Configuration

### Summarization Middleware

Compresses conversation history when approaching token limits:

```yaml
middleware:
  summarization:
    enabled: true
    # Model to use (null = use agent's model)
    model: null
    # Provider to use (null = use agent's provider)
    provider: null

    # When to trigger summarization
    trigger:
      tokens: 60000              # Trigger at token count
      messages: null             # Trigger at message count
      fraction: null             # Trigger at fraction of context (0.0-1.0)

    # What to keep after summarization
    keep:
      messages: 6                # Keep N most recent messages
      tokens: null               # Keep N tokens
      fraction: null             # Keep fraction of context (0.0-1.0)

    summary_prefix: "[Previous conversation summary:]"
    summary_prompt: null         # Custom prompt (null = default)
```

**How it works**:

1. Estimation: Counts tokens using `~4 characters per token` heuristic
2. Trigger check: Activates when any trigger condition is met
3. Split: Separates messages into "keep" and "summarize" groups
4. Summarize: Calls LLM to compress old messages into summary
5. Rebuild: Creates new message list with summary + kept messages

**Example output**:
```
[Previous conversation summary:]

Summary of previous conversation:
User asked about Rust memory management. Assistant explained
stack vs heap, ownership rules, and the borrow checker.

[6 most recent messages...]
```

### Context Editing Middleware

Clears older tool call outputs when token limits are reached:

```yaml
middleware:
  context_editing:
    enabled: true
    trigger_tokens: 60000        # Trigger at token count
    keep_tool_results: 10        # Keep N most recent tool results
    min_reclaim: 1000           # Minimum tokens to reclaim
    clear_tool_inputs: false    # Also clear tool call inputs
    exclude_tools: []           # Tools to exclude from clearing
    placeholder: "[Result cleared due to context limits]"
```

**How it works**:

1. Finds all `tool` role messages (tool results)
2. Checks if tool name is in exclude list
3. Clears all but N most recent results
4. Optionally clears tool call inputs from assistant messages

**Example**:
```yaml
# Before (3 tool results)
messages: [
  {role: "user", content: "Search X"},
  {role: "assistant", tool_calls: [search("X")]},
  {role: "tool", content: "Result 1..."},  # Cleared
  {role: "user", content: "Search Y"},
  {role: "assistant", tool_calls: [search("Y")]},
  {role: "tool", content: "Result 2..."},  # Cleared
  {role: "user", content: "Search Z"},
  {role: "assistant", tool_calls: [search("Z")]},
  {role: "tool", content: "Result 3..."},  # Kept (most recent)
]

# After (keep_tool_results: 1)
messages: [
  {role: "user", content: "Search X"},
  {role: "assistant", tool_calls: [search("X")]},
  {role: "tool", content: "[Result cleared due to context limits]"},
  {role: "user", content: "Search Y"},
  {role: "assistant", tool_calls: [search("Y")]},
  {role: "tool", content: "[Result cleared due to context limits]"},
  {role: "user", content: "Search Z"},
  {role: "assistant", tool_calls: [search("Z")]},
  {role: "tool", content: "Result 3..."},
]
```

## Key Data Structures

### MiddlewareContext

Context passed to middleware during execution:

```rust
pub struct MiddlewareContext {
    pub agent_id: String,              // Agent identifier
    pub conversation_id: Option<String>, // Conversation (if available)
    pub provider_id: String,           // Provider ID
    pub model: String,                 // Model name
    pub message_count: usize,          // Current message count
    pub estimated_tokens: usize,       // Estimated token count
    pub metadata: Value,               // Additional metadata
}
```

### TriggerCondition

Flexible trigger conditions for middleware:

```rust
pub struct TriggerCondition {
    pub tokens: Option<u64>,           // Trigger at token count
    pub messages: Option<usize>,       // Trigger at message count
    pub fraction: Option<f64>,         // Trigger at fraction (0.0-1.0)
}
```

### KeepPolicy

Flexible keep policy for summarization:

```rust
pub struct KeepPolicy {
    pub messages: Option<usize>,       // Keep N messages
    pub tokens: Option<usize>,         // Keep N tokens
    pub fraction: Option<f64>,         // Keep fraction (0.0-1.0)
}
```

## Implementation Details

### Async Trait Pattern

Middleware uses async traits with a workaround for dyn-compatibility:

**Problem**: Rust async traits cannot be made into trait objects (`dyn Trait`).

**Solution**: Return events in the `MiddlewareEffect` enum instead of calling callbacks:

```rust
// ❌ Doesn't work (async trait with callback)
async fn process(&self, messages: Vec<ChatMessage>, on_event: impl FnMut(Event));

// ✅ Works (return events in effect)
async fn process(&self, messages: Vec<ChatMessage>) -> Result<MiddlewareEffect, String>;
```

**Learnings**:
- Generic parameters in async traits prevent dyn-compatibility
- Use enum variants to return events instead of callbacks
- Pipeline extracts events and emits them via callback

### Token Estimation

Uses character-based estimation (no API calls):

```rust
pub fn estimate_total_tokens(messages: &[ChatMessage]) -> usize {
    let total_chars: usize = messages
        .iter()
        .map(|msg| msg.content.len())
        .sum();

    // Rough estimate: ~4 characters per token
    (total_chars / 4) as usize
}
```

**Learnings**:
- OpenAI uses ~4 chars/token for English text
- Code and other languages may vary
- Good enough for context management (not billing)

### Clone Box Pattern

Middleware must be cloneable for use in enums:

```rust
pub trait PreProcessMiddleware: Send + Sync {
    fn clone_box(&self) -> Box<dyn PreProcessMiddleware>;
    // ...
}

impl Clone for Box<dyn PreProcessMiddleware> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
```

**Learnings**:
- Trait objects cannot derive Clone
- Use `clone_box()` pattern for manual cloning
- Required for storing middleware in structs

### Context Window Sizes

Built-in context windows for common models:

```rust
pub fn get_model_context_window(model: &str) -> usize {
    match model {
        // GPT models
        m if m.contains("gpt-4o") => 128000,
        m if m.contains("gpt-4-turbo") => 128000,
        m if m.contains("gpt-4") => 8192,
        m if m.contains("gpt-3.5-turbo") => 16385,

        // Claude models
        m if m.contains("claude-3-5-sonnet") => 200000,
        m if m.contains("claude-3-opus") => 200000,
        m if m.contains("claude-3-sonnet") => 200000,

        // DeepSeek
        m if m.contains("deepseek") => 128000,

        // GLM
        m if m.contains("glm-4") => 128000,

        // Default fallback
        _ => 8192,
    }
}
```

## Working Scenarios

### Scenario 1: Long Conversation with Summarization

**Use case**: Agent has long conversation approaching token limit

**Flow**:
1. Conversation reaches 60,000 tokens (summarization trigger)
2. Summarization middleware activates
3. Keeps 6 most recent messages (~2,000 tokens)
4. Summarizes older messages into ~500 tokens
5. New context: Summary + 6 messages (~2,500 total)

**Configuration**:
```yaml
middleware:
  summarization:
    enabled: true
    trigger:
      tokens: 60000
    keep:
      messages: 6
```

### Scenario 2: Tool-Heavy Conversation with Context Editing

**Use case**: Agent uses many tools (search, database, etc.)

**Flow**:
1. Conversation has 20 tool results (~40,000 tokens)
2. Context editing middleware activates at 60,000 tokens
3. Keeps 10 most recent tool results (~20,000 tokens)
4. Clears 10 oldest results (~20,000 reclaimed)
5. Reclaim threshold met (20,000 > 1,000 min)

**Configuration**:
```yaml
middleware:
  context_editing:
    enabled: true
    trigger_tokens: 60000
    keep_tool_results: 10
    min_reclaim: 1000
```

### Scenario 3: Combined Summarization + Context Editing

**Use case**: Long conversation with both chat and tools

**Flow**:
1. Conversation at 65,000 tokens
2. Summarization runs first (compresses to ~2,500 tokens)
3. Context editing runs second (clears old tool results)
4. Final context: Summary + recent messages + recent tool results

**Configuration**:
```yaml
middleware:
  summarization:
    enabled: true
    trigger:
      tokens: 60000
    keep:
      messages: 6
  context_editing:
    enabled: true
    trigger_tokens: 60000
    keep_tool_results: 10
```

### Scenario 4: Custom Model for Summarization

**Use case**: Use cheaper/faster model for summarization

**Flow**:
1. Agent uses `gpt-4o` for main conversation
2. Summarization uses `gpt-3.5-turbo` (faster, cheaper)
3. Summary is injected back into gpt-4o context

**Configuration**:
```yaml
# Agent config
providerId: openai-gpt4
model: gpt-4o

# Middleware config
middleware:
  summarization:
    enabled: true
    provider: openai-gpt35   # Different provider
    model: gpt-3.5-turbo     # Cheaper model
    trigger:
      tokens: 60000
```

## Logging

### Middleware Events

Middleware emits `StreamEvent::Token` events for visibility:

```rust
// Summarization event
StreamEvent::Token {
    timestamp: ...,
    content: "[Previous conversation summary:]\n[Summarized 24 messages into 456 characters]",
}

// Context editing event
StreamEvent::Token {
    timestamp: ...,
    content: "[Cleared 15 tool results (reclaimed ~18234 tokens)]",
}
```

These appear in the UI as token events, showing middleware activity.

### Minimal Logging

**Design principle**: Middleware should be "quiet" by default

- Only emit events when middleware actually runs
- Use descriptive but concise messages
- Don't log debug info in production

**Example**:
```rust
// ✅ Good: Single event with summary
emit_event("[Cleared 15 tool results (reclaimed ~18234 tokens)]");

// ❌ Bad: Verbose logging
log_debug("Starting context editing...");
log_debug("Found 20 tool results");
log_debug("Clearing 15 results");
log_debug("Reclaimed 18234 tokens");
```

## Learnings

### 1. Dyn-Compatibility Issues

**Problem**: Async traits with generic parameters cannot be trait objects.

**Solution**:
- Remove callback parameters from trait methods
- Return events in `MiddlewareEffect` enum
- Pipeline handles event emission

**Code**:
```rust
// ❌ Doesn't compile
pub trait PreProcessMiddleware {
    async fn process(
        &self,
        messages: Vec<ChatMessage>,
        on_event: impl FnMut(StreamEvent),  // Generic parameter
    ) -> Result<Vec<ChatMessage>, String>;
}

// ✅ Compiles
pub trait PreProcessMiddleware {
    async fn process(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<MiddlewareEffect, String>;  // No generics
}
```

### 2. Move Semantics in Loops

**Problem**: Taking ownership in loop invalidates source.

**Solution**: Use `std::mem::take()` to leave empty vec behind.

**Code**:
```rust
// ❌ Error: value moved
for middleware in &self.pre_processors {
    match middleware.process(messages, context).await? {  // messages moved here
        // ...
    }
}

// ✅ Works: take ownership, leave empty vec
let mut current_messages = messages;
for middleware in &self.pre_processors {
    match middleware.process(std::mem::take(&mut current_messages), context).await? {
        MiddlewareEffect::ModifiedMessages(msgs) => {
            current_messages = msgs;  // Reassign
        }
        // ...
    }
}
```

### 3. Tool Call Mutation

**Problem**: Need to modify tool call arguments, but `ToolCall` fields are private.

**Solution**: Create new `ToolCall` with empty arguments instead of mutating.

**Code**:
```rust
// ❌ Doesn't work: fields are private
tool_call.arguments = serde_json::json!({});

// ✅ Works: create new instance
if let Ok(new_call) = ToolCall::new(
    tool_call.id.clone(),
    tool_call.name().to_string(),
    serde_json::json!({})  // Empty arguments
) {
    tool_calls[i] = new_call;
}
```

### 4. Borrow Checker in Clear Function

**Problem**: Cannot borrow messages as both immutable and mutable.

**Solution**: Extract value first, then do mutable iteration.

**Code**:
```rust
// ❌ Error: immutable borrow while mutable borrow exists
fn clear_tool_inputs(&mut self, messages: &mut Vec<ChatMessage>, idx: usize) {
    if let Some(tool_result) = messages.get(idx) {  // Immutable borrow
        if let Some(id) = &tool_result.tool_call_id {
            for msg in messages.iter_mut() {  // Mutable borrow
                // ...
            }
        }
    }
}

// ✅ Works: extract first, then iterate
fn clear_tool_inputs(&mut self, messages: &mut Vec<ChatMessage>, idx: usize) {
    let tool_call_id_to_clear = messages.get(idx)
        .and_then(|msg| msg.tool_call_id.as_ref())
        .map(|id| id.clone());  // Extract (clone) the value

    if let Some(tool_call_id) = tool_call_id_to_clear {
        for msg in messages.iter_mut() {  // Now we can mutably borrow
            // ...
        }
    }
}
```

## Future Enhancements

### Planned Middlewares

1. **TodoList Middleware**: Auto-generate todo lists from conversation
2. **PII Detection**: Redact sensitive information (emails, phone numbers)
3. **Message Filtering**: Remove redundant or low-value messages
4. **Compression**: Use semantic compression instead of summarization

### Configuration Improvements

1. **UI Builder**: Visual middleware builder (no YAML editing)
2. **Templates**: Pre-built middleware configurations
3. **Validation**: YAML schema validation with helpful errors
4. **Testing**: Test middleware against sample conversations

### Performance

1. **Caching**: Cache summaries for repeated conversations
2. **Parallel Processing**: Run independent middlewares in parallel
3. **Streaming**: Stream summaries for large conversations
4. **Token Counting**: Use tokenizer for accurate counts

## Related Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports |
| `traits.rs` | Middleware trait definitions |
| `pipeline.rs` | Middleware orchestration |
| `config.rs` | Configuration structures |
| `summarization.rs` | Summarization middleware |
| `context_editing.rs` | Context editing middleware |
| `token_counter.rs` | Token estimation utilities |
