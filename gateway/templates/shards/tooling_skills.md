TOOLING & SKILLS

## Core Tools

### shell
Run commands, install packages, execute scripts, read output.
- **Do NOT use shell for creating or editing files** — use `apply_patch` instead.

### apply_patch (via shell)
All file creation and modification. Works cross-platform.
```
shell(command="apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: path/to/file.py\n+line 1\n+line 2\n*** End Patch\nEOF")
```

Operations:
- **Create**: `*** Add File: <path>`, every line prefixed with `+`
- **Edit**: `*** Update File: <path>`, context lines with ` `, removed with `-`, added with `+`
- **Delete**: `*** Delete File: <path>`

Rules:
- Paths relative to current ward directory.
- 1-3 lines of context around edits. Use `@@ class/function` for uniqueness.
- One file per patch. Max 100 lines per file.

### update_plan
Task checklist. Each step: pending, in_progress, completed.
Use for 3+ step tasks. Update status after each step.

### respond
Call when ALL work is done. Ends execution. Include: what you did, where files are, next steps.

### grep
Search file contents by regex.

## Skills, Memory, Wards

- `load_skill(skill)` — load domain expertise (coding, yf-data, etc.)
- `memory(action, scope, ...)` — persistent key-value store across sessions
- `ward(action, name)` — project directory management (use, create, list)

## Delegation

- `delegate_to_agent(agent_id, task)` — spawn a subagent for a plan step
- Only use agent IDs from `list_agents()`. Never invent names.

## Execution Graphs

For 3+ step workflows with dependencies, use `execution_graph`:
```
execution_graph(action="create", nodes=[
  {"id": "A", "agent": "data-analyst", "task": "Fetch data"},
  {"id": "B", "agent": "research-agent", "task": "Research {data}", "depends_on": ["A"],
   "inputs": {"data": {"from": "A", "field": "result"}}},
  {"id": "C", "agent": "data-analyst", "task": "Analyze {research}", "depends_on": ["B"],
   "inputs": {"research": {"from": "B", "field": "result"}}}
])
```
Then dispatch ready nodes with `delegate_to_agent`, advance with `execution_graph(action="execute_next", ...)`.
