# Agent Tools

Built-in tools for AgentZero agents. Tools are organized by category and registered via `core_tools()` and `optional_tools()`.

## Build & Test

```bash
cargo test -p agent-tools      # 25 tests
```

## Tool Categories

### File I/O (`tools/file.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `read` | Safe | Read file contents with optional offset/limit. Resolves via `ward_dir` when ward_id is set. |
| `write` | Moderate | Write/append content to ward workspace |
| `edit` | Moderate | Search and replace in ward files |

### Search (`tools/search.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `grep` | Safe | Regex search in files |
| `glob` | Safe | File pattern matching |

### Execution (`tools/execution/`)
| Tool | Risk | Description |
|------|------|-------------|
| `shell` | Dangerous | Run shell commands. cwd = `wards/{ward_id}/`, venv = `wards/.venv/`. |
| `python` | Dangerous | Execute Python code (optional) |
| `load_skill` | Safe | Load skill instructions into agent context |
| `list_skills` | Safe | List available skills |

### Ward (`tools/ward.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `ward` | Safe | Manage code wards: `use` (switch), `list`, `create`, `info`. Sets `ward_id` in context, emits `WardChanged` event. |

### Memory (`tools/memory.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `memory` | Safe | Persistent key-value storage. Scopes: `shared`, `agent`. |

### Web (`tools/web.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `web_fetch` | Moderate | HTTP requests (GET/POST/PUT/DELETE). Blocks internal networks. Optional. |

### Knowledge Graph (`tools/knowledge_graph.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `list_entities` / `search_entities` / `get_entity_relationships` | Safe | Query entities |
| `add_entity` / `add_relationship` | Moderate | Modify graph |

### UI (`tools/ui.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `request_input` | Safe | Collect structured user input via forms |
| `show_content` | Safe | Display rich content in UI |

### Agent (`tools/agent.rs`, `tools/todo.rs`)
| Tool | Risk | Description |
|------|------|-------------|
| `create_agent` | Moderate | Create new agent config (optional) |
| `todos` | Safe | Task management |

## Registration

```rust
// In tools/mod.rs
pub fn core_tools(fs: Arc<dyn FileSystemContext>) -> Vec<Arc<dyn Tool>>
pub fn optional_tools(fs: Arc<dyn FileSystemContext>, settings: &ToolSettings) -> Vec<Arc<dyn Tool>>
```

Core tools are always registered. Optional tools depend on `ToolSettings` flags.

## Context & State Keys

Tools receive `ToolContext` which provides:
- `get_state("ward_id")` — Current code ward (set by ward tool)
- `get_state("session_id")` — Current session ID
- `get_state("app:agent_id")` — Current agent ID
- `get_state("workspace")` — Workspace context from shared memory
- `get_state("available_agents")` — For list_agents tool
- `get_state("available_skills")` — For list_skills tool

## Security

- **Path sanitization**: Reject `..`, absolute paths. Resolve relative to ward or agent data dir.
- **Network blocking**: Block localhost, private networks, cloud metadata endpoints.
- **Shell safety**: Block 40+ dangerous commands, detect suspicious patterns, enforce timeouts.
