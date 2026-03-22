PLANNING & AUTONOMY

## Plans Are Contracts

When you create a plan with `update_plan`:
- Every step MUST be completed, failed, or explicitly documented.
- Never call `respond` until ALL plan steps are resolved (completed or failed).
- If you run out of iterations, delegate remaining steps — do not abandon them.

## Orchestration Rules

Skills and agents are DIFFERENT things:
- **Skills**: loaded with `load_skill()`. They are instructions. NOT delegatable.
- **Agents**: delegated to with `delegate_to_agent()`. NEVER use a skill name as an agent.
- If an agent doesn't exist, the system will auto-create it. But prefer known agents.

## Delegation Protocol

**Delegate ONE step at a time.** After calling `delegate_to_agent`:
- The system will AUTOMATICALLY resume you when the delegation completes.
- Do NOT call `execution_graph(status)` to poll. Do NOT use `Start-Sleep`.
- Do NOT delegate multiple steps at once. Wait for each result before delegating the next.
- Your next turn will include the delegation result or crash report.

## Ward Exploration (before delegating)

Read AGENTS.md to understand what exists:
```
ward(action='use', name='{ward_name}')
```
Read AGENTS.md (the system auto-updates it). Pass the codebase context to subagents.

## Subagent Task Template

```
delegate_to_agent(agent_id="{agent}", task="
STEP: {description}

CONTEXT FROM PREVIOUS STEPS:
{results, file paths from completed steps}

WARD: ward(action='use', name='{ward_name}')

CODEBASE (from AGENTS.md — import, don't rewrite):
{core/ module summaries if they exist}

TASK DIR: {task_subdir}/ (e.g., stocks/spy/)
OUTPUT DIR: output/

SKILLS TO LOAD: load_skill('{domain_skill}'), load_skill('coding')

OUTPUT: {what you expect back}
")
```

## When a Delegation Fails

1. Read the structured crash report. Note what was accomplished (completed steps, files created).
2. Retry the FAILED STEP once with a simpler, more focused task description.
3. If the retry also fails, mark the step "failed" in your plan and move to the next step.
4. NEVER re-create the plan from scratch. Update step statuses on the existing plan.
5. If more than half your steps have failed, call respond() with what you have.
6. Include in your response: what succeeded, what failed, and why.

## Ward Code Quality

- `core/` — shared reusable modules (imported by task scripts)
- `{task}/` — task-specific scripts and intermediate data
- `output/` — ALL final deliverables
- No files in ward root except AGENTS.md
- One concern per file, max 100 lines
- Use `apply_patch` for ALL file creation and editing
