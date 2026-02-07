# CodeAct: Python & Node Execution

> **STATUS: SUPERSEDED** — Replaced by Code Wards (`plans/code-wards.md`).
> Wards provide persistent named project directories with shared venv/node envs.

**Status**: ~~Planning~~ Superseded
**Branch**: op_jaffa

## Goal

Enable agents to execute Python/Node code using shell + managed state in memory.

## Architecture Decision

**Use shell + memory instead of specialized tools.**

- Agent uses shell to run Python/Node
- Memory holds structured environment state
- Skills teach patterns for checking/updating state
- No auto-creation of envs - agent asks first

## Memory Structure

### `workspace.json` entries

```json
{
  "python_env": {
    "value": "{\"exists\":true,\"venv_path\":\"...\",\"executable\":\"...\",\"pip\":\"...\",\"packages\":[\"pandas\",\"requests\"],\"last_updated\":\"...\"}",
    "tags": ["system", "python", "env"]
  },
  "node_env": {
    "value": "{\"exists\":true,\"env_path\":\"...\",\"node_modules\":\"...\",\"packages\":[\"axios\"],\"last_updated\":\"...\"}",
    "tags": ["system", "node", "env"]
  }
}
```

### When env doesn't exist

```json
{
  "python_env": {
    "value": "{\"exists\":false,\"venv_path\":\"~/Documents/agentzero/venv\",\"setup_command\":\"python -m venv ...\"}",
    "tags": ["system", "python", "env"]
  }
}
```

## Agent Behavior (enforced by skill)

### Before running code:
1. Check `memory(action="get", scope="shared", file="workspace", key="python_env")`
2. Parse JSON value
3. If `exists: false` → ask user before creating
4. If package not in `packages` list → ask user before installing

### After installing packages:
1. Update memory with new package list
2. Update `last_updated` timestamp

### Never:
- Auto-create venv on first failure
- Install packages without asking
- Assume env exists without checking memory

## Gateway Seeding (startup)

On startup, gateway seeds memory with current state:
1. Check if venv exists → set `exists: true/false`
2. If exists, run `pip list` → populate `packages`
3. Same for node_env with `npm list`

Only seeds if entry doesn't exist (preserve user's state).

## System Prompt Injection

Keep the simple version (already implemented):
```
ENVIRONMENT
- OS: windows (x86_64)
- Vault: ~/Documents/agentzero
- Python: .../venv/Scripts/python.exe (if exists)
- NodeModules: .../node_env/node_modules (if exists)
```

This gives immediate visibility. Memory gives structured access.

## Skill: python-codeact

```markdown
## Before Running Python

1. Check environment state:
   memory(action="get", scope="shared", file="workspace", key="python_env")

2. Parse the JSON value. If `exists: false`:
   - Tell user: "Python venv not set up. Create it?"
   - If yes: run setup_command via shell
   - Update memory with exists: true

3. If package needed but not in packages list:
   - Tell user: "Package X not installed. Install it?"
   - If yes: run pip install via shell
   - Update memory with new package in list

## Running Python

Use the executable from memory:
shell("<executable> -c \"<code>\"")

Or for scripts:
shell("<executable> script.py")

## After Installing Packages

Always update memory:
memory(action="set", scope="shared", file="workspace", key="python_env",
       value="{...updated JSON with new packages...}")
```

## Implementation Checklist

### Gateway (state.rs)
- [ ] Add `seed_workspace_defaults()` function
- [ ] Detect venv exists → set python_env with packages from `pip list --format=json`
- [ ] Detect node_env exists → set node_env with packages from package.json
- [ ] Only seed if entry doesn't already exist

### Templates (already done)
- [x] Inject paths into system prompt

### Skills
- [ ] Create `skills/python-codeact/SKILL.md`
- [ ] Create `skills/node-codeact/SKILL.md`

## Open Questions

1. **Refresh packages** - Should we re-scan packages on each startup or trust memory?
   - Option A: Always re-scan (slower, accurate)
   - Option B: Trust memory, skill can trigger refresh
   - Recommendation: Option B - memory is source of truth, skill has "refresh" pattern

2. **Package format** - Just names or include versions?
   - Option A: `["pandas", "requests"]`
   - Option B: `[{"name": "pandas", "version": "2.0.0"}, ...]`
   - Recommendation: Option A for simplicity, agent can query versions if needed

3. **Concurrent updates** - What if user installs package outside agent?
   - Skill should have "sync" pattern to refresh from pip list
   - Agent should be instructed to sync periodically or on errors
