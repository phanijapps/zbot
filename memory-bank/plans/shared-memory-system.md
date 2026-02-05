# Shared Memory System

## Status: COMPLETE

## Workspace Auto-Inject (COMPLETE)

The executor automatically loads `workspace.json` from shared memory and injects it into initial state:
- Loaded at executor build time in `gateway/src/execution/invoke/executor.rs`
- Available to tools via `context.get_state("workspace")`
- Contains flattened key-value pairs (e.g., `working_dir`, `project_name`)

## Future Enhancements

- **Fuzzy search**: Add fuzzy matching to `memory(action="search")` for approximate key/value matching
- **Concurrent access safety**: File locking or atomic writes for parallel agent access
  - Multiple agents may read/write shared memory simultaneously
  - Options: file locks (`flock`), atomic rename, or SQLite for shared state
- **Inter-agent communication**: Agents may need to communicate via shared memory
  - Consider a message queue or pub/sub pattern for real-time coordination
  - Separate from persistent memory (ephemeral channels vs durable storage)

## Overview

Extend the memory tool to support shared memory across all sessions, enabling pattern learning and cross-session knowledge retention.

## Current State

```
agents_data/{agent_id}/memory.json  ← single file, per-agent only
```

Memory tool signature:
```json
{"action": "set", "key": "...", "value": "...", "tags": [...]}
```

## Target State

```
agents_data/
├── shared/                         # Cross-session shared memory
│   ├── user_info.json              # User identity, preferences
│   ├── workspace.json              # Working dir, project paths
│   ├── patterns.json               # Learned patterns/conventions
│   └── session_summaries.json      # Distilled session learnings
├── root/
│   └── memory.json                 # Agent-specific (unchanged)
```

Memory tool signature (extended):
```json
{"action": "set", "scope": "shared", "file": "patterns", "key": "...", "value": "..."}
{"action": "get", "scope": "shared", "file": "user_info", "key": "name"}
{"action": "list", "scope": "shared", "file": "patterns"}
```

## Task Breakdown

### Phase 1: Extend Memory Tool

| Task | Description | File |
|------|-------------|------|
| #1.1 | Add `scope` parameter ("agent" default, "shared") | `runtime/agent-tools/src/tools/memory.rs` |
| #1.2 | Add `file` parameter for shared scope | `runtime/agent-tools/src/tools/memory.rs` |
| #1.3 | Create `shared_memory_path()` helper | `runtime/agent-tools/src/tools/memory.rs` |
| #1.4 | Update `load_store()` to handle both scopes | `runtime/agent-tools/src/tools/memory.rs` |
| #1.5 | Update `save_store()` to handle both scopes | `runtime/agent-tools/src/tools/memory.rs` |
| #1.6 | Add unit tests for shared memory | `runtime/agent-tools/src/tools/memory.rs` |

### Phase 2: Update System Prompt

| Task | Description | File |
|------|-------------|------|
| #2.1 | Add MEMORY & LEARNING section to INSTRUCTIONS.md | `gateway/templates/system_prompt.md` |
| #2.2 | Document shared memory files and their purpose | `gateway/templates/system_prompt.md` |
| #2.3 | Add pattern learning guidelines | `gateway/templates/system_prompt.md` |

## Code Changes

### memory.rs - Schema Update

```rust
fn parameters_schema(&self) -> Option<Value> {
    Some(json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["get", "set", "delete", "list", "search"],
                "description": "The memory operation to perform"
            },
            "scope": {
                "type": "string",
                "enum": ["agent", "shared"],
                "default": "agent",
                "description": "Memory scope: 'agent' for agent-specific, 'shared' for cross-session"
            },
            "file": {
                "type": "string",
                "enum": ["user_info", "workspace", "patterns", "session_summaries"],
                "description": "Shared memory file (required when scope is 'shared')"
            },
            "key": { ... },
            "value": { ... },
            // ... rest unchanged
        },
        "required": ["action"]
    }))
}
```

### memory.rs - Path Resolution

```rust
/// Get memory file path based on scope
fn memory_path(&self, agent_id: &str, scope: &str, file: Option<&str>) -> Result<PathBuf> {
    match scope {
        "shared" => {
            let file = file.ok_or_else(||
                ZeroError::Tool("'file' required for shared scope".into()))?;
            // Validate file name
            let valid_files = ["user_info", "workspace", "patterns", "session_summaries"];
            if !valid_files.contains(&file) {
                return Err(ZeroError::Tool(format!("Invalid shared file: {}", file)));
            }
            self.fs
                .vault_path()
                .map(|p| p.join("agents_data").join("shared").join(format!("{}.json", file)))
                .ok_or_else(|| ZeroError::Tool("No vault path configured".into()))
        }
        "agent" | _ => {
            self.fs
                .agent_data_dir(agent_id)
                .map(|dir| dir.join(MEMORY_FILE))
                .ok_or_else(|| ZeroError::Tool("No agent data directory configured".into()))
        }
    }
}
```

### INSTRUCTIONS.md - Memory Section

```markdown
MEMORY & LEARNING
- You have persistent memory that survives across sessions.
- Use shared memory to remember important information:

  **user_info**: User preferences, name, working style
  - memory set --scope shared --file user_info --key name --value "..."

  **workspace**: Project paths, working directories
  - memory set --scope shared --file workspace --key working_dir --value "/path/to/project"

  **patterns**: Learned patterns and conventions
  - memory set --scope shared --file patterns --key rust_test_cmd --value "cargo test"
  - memory set --scope shared --file patterns --key commit_style --value "conventional commits"

  **session_summaries**: Key learnings from sessions
  - memory set --scope shared --file session_summaries --key 2024-02-04 --value "..."

- At session start, check shared memory for relevant context:
  - memory list --scope shared --file workspace
  - memory list --scope shared --file patterns

- When you learn something reusable (commands, preferences, patterns):
  - Save it to shared memory for future sessions
  - Be concise: store the actionable pattern, not verbose explanations

- Agent-specific memory (default scope) is for temporary, agent-local data.
```

## Test Plan

### Unit Tests

```rust
#[test]
fn test_shared_memory_set_and_get() {
    // Create temp dir with vault structure
    // Set value with scope=shared, file=patterns
    // Get value back, verify
}

#[test]
fn test_shared_memory_requires_file() {
    // scope=shared without file parameter should error
}

#[test]
fn test_shared_memory_invalid_file() {
    // scope=shared with invalid file name should error
}

#[test]
fn test_agent_memory_unchanged() {
    // Default behavior (no scope) should work as before
}
```

## Verification

```bash
# Run memory tool tests
cargo test -p agent-tools -- memory

# Run all agent-tools tests
cargo test -p agent-tools

# Check compilation
cargo check --workspace
```

## Success Criteria

- [ ] Memory tool accepts `scope` and `file` parameters
- [ ] Shared memory stored in `agents_data/shared/{file}.json`
- [ ] Agent memory unchanged (backward compatible)
- [ ] INSTRUCTIONS.md documents memory usage
- [ ] All tests pass
