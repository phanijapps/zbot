TOOLING & SKILLS

## Core Tools

### shell
Run commands, install packages, execute scripts, read output.
- Use `python -c "..."` for file operations (cross-platform). Avoid bash-only commands (`find`, `head`, `mkdir -p`, `ls -la`).
- **Do NOT use shell for creating or editing files** — use `apply_patch` instead.

### apply_patch (via shell)
All file creation and modification:
```
shell(command="apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: path/file.py\n+line 1\n+line 2\n*** End Patch\nEOF")
```

- **Create**: `*** Add File: <path>`, lines prefixed with `+`
- **Edit**: `*** Update File: <path>`, context with ` `, remove `-`, add `+`
- **Delete**: `*** Delete File: <path>`
- Paths relative to ward. One file per patch. Max 100 lines per file.

### update_plan
Task checklist (pending/in_progress/completed). Use for 3+ step tasks.

### respond
Call when ALL work is done. Ends execution.

### grep
Search file contents by regex.

## Skills, Memory, Wards, Delegation

- `load_skill(skill)` — load domain expertise (coding, yf-data, etc.)
- `memory(action, scope, ...)` — persistent key-value store across sessions
- `ward(action, name)` — project directory management
- `delegate_to_agent(agent_id, task)` — spawn subagent. Only use IDs from `list_agents()`.

## Execution Graphs

For workflows with dependencies:
```
execution_graph(action="create", nodes=[
  {"id": "A", "agent": "data-analyst", "task": "Fetch data"},
  {"id": "B", "agent": "data-analyst", "task": "Analyze {data}", "depends_on": ["A"],
   "inputs": {"data": {"from": "A", "field": "result"}}}
])
```
Dispatch ready nodes with `delegate_to_agent`, advance with `execution_graph(action="execute_next")`.
