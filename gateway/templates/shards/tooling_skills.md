TOOLING & SKILLS

## Core Tools

### shell
Run commands, install packages, execute scripts, read output.
- Use `grep` to search files. Do NOT `cat` entire files.
- Do NOT use `Set-Content`, `Out-File`, `@"..."@`, `cat >`, or heredocs for file writing.

### write_file
Create or overwrite a file. Path is relative to the current ward.
- `write_file(path="core/utils.py", content="def helper(): ...")`
- Creates parent directories automatically.

### edit_file
Edit an existing file by finding and replacing exact text.
- `edit_file(path="core/utils.py", old_text="def helper():", new_text="def helper(x):")`
- old_text must be unique in the file. If multiple matches, include more context.
- Use `grep` first to find the exact text to replace.

### update_plan
Task checklist. Steps: pending, in_progress, completed, failed. Use for 3+ step tasks.

### respond
Call when ALL work is done. Ends execution. If you created output files (reports, code, documents, data, images, etc.), declare them as artifacts:

```json
respond({
  "message": "Task complete. Created the auth system with tests.",
  "artifacts": [
    { "path": "src/auth.rs", "label": "Auth middleware" },
    { "path": "docs/api.md", "label": "API documentation" },
    { "path": "reports/test-results.html", "label": "Test results" }
  ]
})
```

Always include artifacts for files the user would want to see or download. Paths are relative to the current ward.

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

## Knowledge Graph

### graph_query
Query the knowledge graph of entities and relationships accumulated from prior sessions, ward artifacts, and tool results.

Three actions:
- `graph_query(action="search", query="<name>", entity_type?, limit?)` — find entities whose name contains the query string.
- `graph_query(action="neighbors", entity_name="<name>", direction?="both", depth?=1, limit?)` — list entities connected to this one. `depth=2` traverses 2 hops.
- `graph_query(action="context", query="<topic>", limit?)` — semantic search plus surrounding subgraph for a topic.

When to call:
- User mentions a named entity (person, organization, location, document, tool) you don't already have context on from the session's recall block → `search` it first.
- You need to understand how two or more entities relate, or identify the central figures in a domain → `neighbors` with `depth=2`.
- Starting a research task on a topic you've touched before → `context` to retrieve the relevant subgraph.

When NOT to call:
- For simple fact lookup that should live in `memory_facts` — use `memory(action="recall")` instead.
- More than 2 consecutive `graph_query` calls on the same turn — if you're still lost, delegate to a subagent with the information you have.
