# Code Wards: Agent-Managed Project Containers

**Status**: COMPLETE (Phases 1-3). Phase 4 is future work.
**Branch**: feat/responsive-agent-phase1

## Context & Problem

Every session gets `vault/code/sess-{uuid}/` — isolated, ephemeral, gone when the session ends.
No project continuity. No code reuse. Agent starts from scratch every time.

**Goal**: Named project directories (wards) that the agent creates, manages, and navigates
autonomously. The agent decides the ward name, organizes its work, and reuses code across sessions.

## Core Principles

1. **Agent autonomy** — the agent decides which ward to work in, creates new wards, names them
2. **Creative hub** — code persists. Agents learn from past code
3. **Shared environments** — one Python venv, one Node env for all wards
4. **Simplicity** — wards are directories. No metadata databases for MVP

## Directory Structure

```
~/Documents/agentzero/
  wards/
    .venv/                          # ONE shared Python venv
    .node_env/
      node_modules/                 # ONE shared Node env

    gerome/                         # Ward: gerome (agent-named)
      trinomial_docx.js
    phenom/                         # Ward: phenom
      yfinance_for_tsla.py
    scratch/                        # Default ward for quick tasks

  agents/                           # Unchanged
  agents_data/                      # Unchanged
  agent_data/                       # Session ephemeral data (unchanged)
```

## Ward Tool

Core tool, always available. Actions:

- `ward(action="use", name="X")` — set current ward, create dir if needed, return file listing
- `ward(action="list")` — list all ward names + descriptions from ward memory
- `ward(action="create", name="X")` — alias for use (creates + sets)
- `ward(action="info", name="X")` — detailed info about a specific ward

## Agent Decision Flow

The agent is autonomous. The system prompt instructs it to:
1. `ward(action="list")` to see existing wards
2. Match task to existing ward, or create a new one
3. For quick one-offs, use the "scratch" ward

## Tool Changes

### File Tools (write, edit, read)
- `session_code_dir` → `ward_dir` when `ward_id` is set in state
- Attachments/scratchpad stay session-scoped

### Shell Tool
- `cwd` from `ward_id` instead of `session_id`
- Venv from `wards/.venv/`, node from `wards/.node_env/`

### Memory Tool
- Add `"ward"` scope → `wards/{ward_id}/.ward_memory.json`
- Auto-loaded when agent uses a ward

## Event Pipeline

New events: `StreamEvent::WardChanged`, `GatewayEvent::WardChanged`, `ServerMessage::WardChanged`

## Memory Hierarchy (4 tiers)

| Tier | Path | Purpose |
|------|------|---------|
| Global Shared | `agents_data/shared/*.json` | user_info, workspace, patterns |
| Agent | `agents_data/{agent_id}/memory.json` | Per-agent private context |
| Ward (NEW) | `wards/{ward_id}/.ward_memory.json` | Project context |
| Session | `agent_data/{session_id}/` | Ephemeral: attachments, scratchpad |

## Implementation Phases

### Phase 1: Ward Tool + FileSystem ✅
1. ✅ Add `ward_dir()`, `wards_root_dir()` to `FileSystemContext`
2. ✅ Implement in `GatewayFileSystem`
3. ✅ Create `WardTool` (use, list, create, info)
4. ✅ Register as core tool
5. ✅ Create `wards/scratch/` on startup
6. ✅ Add `WardChanged` event to StreamEvent, GatewayEvent, ServerMessage
7. ✅ Add `ward_id` to Session struct + DB schema (v6)
8. ✅ Persist ward_id on WardChanged event
9. ✅ Restore ward_id on session continuation + delegation inheritance

### Phase 2: Wire Existing Tools ✅
1. ✅ Shell: cwd from `ward_id`, venv/node from `wards/`
2. ✅ Write/Edit/Read: resolve via `ward_dir()` instead of `session_code_dir()`
3. ✅ Memory: add `"ward"` scope
4. ✅ Executor: inject `ward_id` into state, load ward memory on change

### Phase 3: System Prompt + Skill ✅
1. ✅ Add ward instructions to tooling_skills shard
2. ✅ Create `skills/code-wards/SKILL.md`
3. ✅ Ward memory auto-injection into executor state

### Phase 4: Creative Hub (Future)
1. Cross-ward code discovery
2. Pattern learning from ward code
3. Ward archival/cleanup commands
