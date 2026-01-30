# Agent Tools

Built-in tools for the AgentZero application.

## Overview

This crate provides all built-in tools that agents can use. Tools are organized by category and registered via `builtin_tools_with_fs()`.

## Tool Categories

### File I/O
| Tool | Risk | Description |
|------|------|-------------|
| `read` | Safe | Read file contents with optional offset/limit |
| `write` | Moderate | Write/append content to agent workspace |
| `edit` | Moderate | Search and replace in files |

### Search
| Tool | Risk | Description |
|------|------|-------------|
| `grep` | Safe | Regex search in files |
| `glob` | Safe | File pattern matching |

### Execution
| Tool | Risk | Description |
|------|------|-------------|
| `python` | Dangerous | Execute Python code |
| `shell` | Dangerous | Execute shell commands |
| `load_skill` | Safe | Load skill instructions |

### Web
| Tool | Risk | Description |
|------|------|-------------|
| `web_fetch` | Moderate | HTTP requests (GET/POST/PUT/DELETE) |

### Memory
| Tool | Risk | Description |
|------|------|-------------|
| `memory` | Safe | Persistent key-value storage |

### Knowledge Graph
| Tool | Risk | Description |
|------|------|-------------|
| `list_entities` | Safe | List all entities |
| `search_entities` | Safe | Search entities by name |
| `get_entity_relationships` | Safe | Get entity relationships |
| `add_entity` | Moderate | Add new entity |
| `add_relationship` | Moderate | Add entity relationship |

### UI
| Tool | Risk | Description |
|------|------|-------------|
| `request_input` | Safe | Collect structured user input |
| `show_content` | Safe | Display content in UI |

### Agent
| Tool | Risk | Description |
|------|------|-------------|
| `create_agent` | Moderate | Create new agent |
| `todos` | Safe | Manage TODO list |

## Adding New Tools

1. Create a new file in `src/tools/` or add to existing category
2. Implement the `Tool` trait:

```rust
use zero_core::{Tool, ToolContext, ToolPermissions, Result};

pub struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    
    fn description(&self) -> &str { "Description shown to agent" }
    
    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": { ... },
            "required": [...]
        }))
    }
    
    fn permissions(&self) -> ToolPermissions {
        // Choose appropriate risk level
        ToolPermissions::safe()           // Read-only
        ToolPermissions::moderate(vec![]) // Controlled side effects
        ToolPermissions::dangerous(vec![])// System affecting
    }
    
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Implementation
    }
}
```

3. Export in `mod.rs`
4. Add to `builtin_tools_with_fs()` factory

## Security Guidelines

### Path Sanitization (File Tools)
- Reject paths with `..`
- Reject absolute paths starting with `/` or `\`
- Resolve paths relative to agent's data directory

### Network Security (Web Tools)
- Block internal/private networks (localhost, 10.x, 192.168.x)
- Block cloud metadata endpoints (169.254.169.254)
- Enforce size limits and timeouts

### Shell Security
- Block 40+ dangerous commands (see `shell.rs`)
- Detect suspicious patterns (curl|sh, base64 -d)
- Disable when running as root/administrator
- Enforce timeouts and output limits

## Context Access

Tools receive `ToolContext` which provides:
- `get_state(key)` / `set_state(key, value)` - Session state
- `function_call_id()` - Current tool call ID
- `user_content()` - Original user message
- `agent_name()` - Current agent

Common state keys:
- `app:agent_id` - Current agent ID
- `app:root_agent_id` - Parent agent (for subagents)
- `app:conversation_id` - Current conversation
