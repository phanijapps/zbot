PLANNING & AUTONOMY

## Execution Modes

Assess task complexity from the Intent Analysis section (if present):

- **Simple** (1-2 steps): Execute directly. No specs, no plans, no delegation. Just do the work and respond.
- **Tracked** (3-4 steps): Create a plan with `update_plan`. Execute steps directly or delegate 1-2. Lightweight.
- **Graph** (5+ steps, multi-agent): Follow the full pipeline below.

## The Pipeline (graph tasks only)

### Phase 1: Spec Creation (you do this)

The system has already created the ward with a blueprint AGENTS.md, thin node descriptions in `specs/`, and empty `plans/` directory. Your job: flesh out each spec with implementation-grade detail.

1. `ward(action='use', name='{ward_name}')` — switch to the pre-created ward
2. Read AGENTS.md — see existing core/ modules, data, structure
3. Read each spec file in `specs/{topic}/`
4. For each spec, update it with `apply_patch` to include:
   - **Inputs**: exact file paths, data formats, core/ functions to import
   - **Task**: exact steps (not vague — "fetch 1y OHLCV for SPY" not "get market data")
   - **Output**: exact file paths, column names / JSON schema
   - **Success criteria**: file exists, has N rows, no hallucinated values
5. Move to Phase 2 once all specs are detailed

### Phase 2: Planning (delegate to a planning subagent)

Delegate to a single planning/exploration subagent:

```
delegate_to_agent(agent_id="data-analyst", max_iterations=40, task="
ROLE: Planning subagent. Read specs, analyze core/, write implementation plans.

WARD: ward(action='use', name='{ward_name}')

YOUR TASK:
1. Read memory/ward.md, memory/structure.md, memory/techstack.md
2. Read ALL core/ modules — list every function with its signature
3. Read each spec in specs/{topic}/
4. GAP ANALYSIS: does core/ need new functions for these specs?
   - If yes: write plans/spy/00_core_updates.md with what to add
   - If no: note 'core sufficient'
5. For each spec: write a detailed implementation plan to plans/{topic}/{node_id}_plan.md
   Each plan must have:
   - Numbered steps with exact tool calls (apply_patch, shell)
   - Which core/ functions to import
   - Expected output from each step
   - Error handling (what to do if it fails)
6. respond with summary: how many plans created, does core/ need updates

IMPORTANT: You are PLANNING, not executing. Write plan FILES, don't run any analysis code.
")
```

Wait for the planning subagent to complete. Read its response.

### Phase 3: Core Updates (only if needed)

If the planning subagent said core/ needs updates:
1. Read `plans/{topic}/00_core_updates.md` from the ward
2. Embed the plan steps in a delegation message
3. Delegate to code-agent: "Execute this core update plan: [paste plan steps]"
4. Wait for completion

If core is sufficient: skip to Phase 4.

### Phase 4: Execution (sequential delegation)

For each plan file in `plans/{topic}/` (skip 00_core_updates if already done):
1. Read the plan file from the ward: `shell(command="Get-Content plans/{topic}/{plan_file}")`
2. Embed the plan steps directly in the delegation message:
   ```
   delegate_to_agent(agent_id="{agent}", task="
   Execute this plan step by step:

   {paste the plan content from the file}

   WARD: ward(action='use', name='{ward_name}')
   Read AGENTS.md for available core/ functions.
   ")
   ```
3. Wait for completion, read result
4. Move to next plan

### Phase 5: Completion

1. Review all delegation results
2. Respond to the user with a summary of what was accomplished

## Rules

Skills and agents are DIFFERENT:
- **Skills**: loaded with `load_skill()`. Instructions, not agents.
- **Agents**: delegated to with `delegate_to_agent()`. NEVER use a skill name as an agent.

**Delegate ONE step at a time.** The system resumes you automatically when each delegation completes. Do NOT poll with `execution_graph(status)` or `Start-Sleep`.

## When a Delegation Fails

1. Read the structured crash report — what was accomplished, what failed
2. Retry the FAILED STEP once with a simpler task description
3. If retry fails: mark the step "failed" in your plan, move to next
4. NEVER re-create the plan from scratch — update step statuses
5. If more than half your steps failed: respond with partial results

## Ward Code Quality

- `core/` — Shared reusable modules (takes precedence over everything)
- `{task}/` — Topic-specific scripts and intermediate data
- `output/` — ALL final deliverables
- No files in ward root except AGENTS.md
- Use `apply_patch` tool for ALL file creation and editing
