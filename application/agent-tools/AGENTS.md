# agent-tools

Built-in tools for the agentzero application.

## Setup

```bash
# Build
cargo build

# Run tests
cargo test
```

## Code Style

- Each tool in its own module under `src/tools/`
- Re-export tools in `mod.rs`
- Use `FileSystemContext` for file operations

## Available Tools

### File Tools

- `ReadTool` - Read file contents
- `WriteTool` - Write file contents (supports conversation-scoped paths)
- `EditTool` - Edit file with exact string replacement

### Search Tools

- `GrepTool` - Search file contents with regex
- `GlobTool` - Find files by pattern

### Execution Tools

- `PythonTool` - Execute Python code
- `LoadSkillTool` - Load and execute skill files

### UI Tools

- `RequestInputTool` - Request user input
- `ShowContentTool` - Display content to user

## Conversation-Scoped File Operations

The `WriteTool` and `EditTool` support conversation-scoped paths:
- When `conversation_id` is set, paths are resolved to `~/.config/zeroagent/logs/<conversation_id>/`
- Relative paths are scoped to the conversation directory
- Absolute paths are used as-is

**Known Issue:** Path resolution currently has issues - see `memory-bank/known_issues.md`

## Factory Function

Use `builtin_tools_with_fs(fs, conversation_id)` to create all tools:
```rust
let tools = builtin_tools_with_fs(
    Arc::new(fs_context),
    Some(conversation_id.clone())
);
```

## Testing

Test with various file states (missing, readable, writable).

## Important Notes

- WriteTool requires `path` and `content` parameters
- EditTool requires `path`, `old_string`, and `new_string` parameters
- GrepTool supports `-i`, `-C`, `-A`, `-B` flags via parameters
