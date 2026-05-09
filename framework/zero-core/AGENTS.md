# Zero Core

Core traits, types, and abstractions for the Zero agent framework.

## Overview

This crate provides the foundational abstractions that all other Zero crates build upon.

## Key Abstractions

### Agent Trait

```rust
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn sub_agents(&self) -> &[Arc<dyn Agent>];
    async fn run(&self, ctx: Arc<dyn InvocationContext>) -> Result<EventStream>;
}
```

Agents are invokable AI entities that receive context and produce a stream of events.

### Tool Trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value>;
    fn response_schema(&self) -> Option<Value>;
    fn permissions(&self) -> ToolPermissions;  // Risk level, capabilities
    fn validate(&self, args: &Value) -> Result<()>;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}
```

Tools are callable functions with JSON schemas for parameters and responses.

### Context Hierarchy

```
ReadonlyContext (base)
├── invocation_id, agent_name, user_id, session_id, user_content
│
├─ CallbackContext (extends ReadonlyContext)
│  └── get_state(key), set_state(key, value)
│
├─ ToolContext (extends CallbackContext)
│  └── function_call_id(), actions()
│
└─ InvocationContext (extends CallbackContext)
   └── agent(), session(), run_config(), end_invocation()
```

### Event System

Events are immutable log entries with:
- `id`: Unique UUID
- `invocation_id`: Groups events from single agent run
- `author`: "user", "agent", "tool", "system"
- `content`: Optional role-based message
- `actions`: State deltas, transfers, escalations

## Tool Policy Framework

Every tool has permission requirements:

```rust
pub enum ToolRiskLevel {
    Safe,      // Read-only, no side effects
    Moderate,  // Controlled side effects (sandboxed writes)
    Dangerous, // Can affect system (shell, browser)
    Critical,  // Requires explicit approval
}

pub struct ToolPermissions {
    pub risk_level: ToolRiskLevel,
    pub requires: Vec<String>,      // Required capabilities
    pub auto_approve: bool,         // Can skip confirmation
    pub max_duration_secs: Option<u64>,
    pub max_output_bytes: Option<usize>,
}
```

### Standard Capabilities

Tools declare required capabilities:
- `filesystem:read` - Read files
- `filesystem:write` - Write/modify files
- `network:http` - Make HTTP requests
- `shell:execute` - Run shell commands
- `browser:automation` - Control browser

### Risk Levels by Tool Category

| Category | Risk Level | Examples |
|----------|------------|----------|
| File Read | Safe | `read`, `grep`, `glob` |
| File Write | Moderate | `write`, `edit` |
| Network | Moderate | `web_fetch` |
| Code Execution | Dangerous | `python`, `shell` |
| System Config | Critical | `configure_system` |

## State Management

State is managed via prefixed keys:

```rust
pub const KEY_PREFIX_USER: &str = "user:";   // Persists across sessions
pub const KEY_PREFIX_APP: &str = "app:";     // Application-wide
pub const KEY_PREFIX_TEMP: &str = "temp:";   // Cleared each turn
```

Common state keys:
- `app:agent_id` - Current agent
- `app:root_agent_id` - Parent agent (for subagents)
- `app:conversation_id` - Current conversation
- `app:todo_list` - TODO items

## FileSystem Abstraction

```rust
pub trait FileSystemContext: Send + Sync {
    fn conversation_dir(&self, id: &str) -> Option<PathBuf>;
    fn outputs_dir(&self) -> Option<PathBuf>;
    fn skills_dir(&self) -> Option<PathBuf>;
    fn agents_dir(&self) -> Option<PathBuf>;
    fn agent_data_dir(&self, agent_id: &str) -> Option<PathBuf>;
    fn python_executable(&self) -> Option<PathBuf>;
    fn vault_path(&self) -> Option<PathBuf>;
}
```

Enables portable path resolution across different environments.

## Error Handling

```rust
pub enum ZeroError {
    Llm(String),
    Tool(String),
    Mcp(String),
    Config(String),
    Serialization(serde_json::Error),
    Io(std::io::Error),
    Generic(String),
}
```

All errors convert to `ZeroError` for consistent handling.
