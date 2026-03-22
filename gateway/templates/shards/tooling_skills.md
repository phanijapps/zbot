TOOLING & SKILLS

## Core Tools

### shell
Run commands, install packages, execute scripts, read output.
- **Do NOT use shell for creating or editing files** — use `apply_patch` instead.
- Do NOT use `Set-Content`, `Out-File`, `@"..."@`, `cat >`, or heredocs for file writing.

### apply_patch (via shell)
Use `apply_patch` for **ALL file creation and modification**.

The patch format is a file-oriented diff with three operations:

```
*** Begin Patch
*** Add File: <path>      ← create new file, lines prefixed with +
*** Update File: <path>   ← modify existing file with hunks
*** Delete File: <path>   ← remove file
*** End Patch
```

**Creating a file:**
```
shell(command="apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: core/data_fetch.py\n+\"\"\"Reusable data fetching.\"\"\"\n+import yfinance as yf\n+\n+def get_ohlcv(ticker, period=\"1y\"):\n+    return yf.download(ticker, period=period, progress=False)\n*** End Patch\nEOF")
```

Every content line MUST start with `+`. This is required.

**Editing a file:**
```
shell(command="apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: core/data_fetch.py\n@@ def get_ohlcv\n-    return yf.download(ticker, period=period, progress=False)\n+    data = yf.download(ticker, period=period, progress=False)\n+    if isinstance(data.columns, pd.MultiIndex):\n+        data.columns = [c[0] for c in data.columns]\n+    return data\n*** End Patch\nEOF")
```

Update hunks use `@@` context, ` ` for unchanged lines, `-` for removed, `+` for added.

**Deleting a file:**
```
shell(command="apply_patch <<'EOF'\n*** Begin Patch\n*** Delete File: temp.py\n*** End Patch\nEOF")
```

**Rules:**
- Paths relative to current ward directory
- One file per patch call
- Max 100 lines per file
- `+` prefix required on every content line in Add File

### update_plan
Task checklist. Steps: pending, in_progress, completed, failed. Use for 3+ step tasks.

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
