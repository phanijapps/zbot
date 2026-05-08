# Defect — `general-purpose` agent delegates to root + planner instead of executing locally

**Severity:** Medium (wastes tokens, adds latency, can route around the agent's bounded prompt)
**Discovered:** 2026-05-03 after first cleanup-cron tick (PR #96)
**Status:** Open, deferred

## Symptom

When the bundled `default-cleanup` cron fires and dispatches to the
`general-purpose` agent, gp's LLM calls `delegate_to_agent` to hand the
task off — typically to the planner, which routes execution through
root. The cleanup work that should be a single shell call inside gp
becomes a multi-agent planning session.

User-visible effects:
- Token cost ~3-5× higher per cron tick than necessary.
- Cleanup transcript shows up under root + planner agents instead of gp.
- gp's own prompt (which says "no delegation, no planning") is
  effectively ignored.

## Root cause

`gateway/gateway-execution/src/invoke/executor.rs:607` —
`tool_registry.register(Arc::new(DelegateTool::new()))` runs
unconditionally during executor setup, alongside `RespondTool` and
`MultimodalAnalyzeTool`. There is no per-agent gate on this
registration.

So every agent — including gp, whose `default_agents.json` entry has
empty `skills: []` and `mcps: []` and a system prompt explicitly
forbidding delegation — gets `delegate_to_agent` in its function list.

LLMs treat *tool availability* as a stronger signal than prose
instructions. Once `delegate_to_agent` is in the toolbox, the LLM will
use it on tasks that look "complex" (any cleanup framed as
"delete files older than X from Y and Z" reads as multi-step → plan →
delegate). Prompt instructions can suggest, but they can't physically
prevent the tool call.

## Reproduction

1. Run the daemon with PR #96 merged.
2. Wait for a `default-cleanup` cron tick, or trigger it via
   `POST /api/cron/default-cleanup/trigger`.
3. Inspect the spawned session in `/logs` or `/api/sessions`.
4. Observe that the active agent is `planner-agent` or `root`, not
   `general-purpose`. Tool-call log shows `delegate_to_agent` from gp
   as the first action.

## Suggested fix (~30 lines)

### Backend

1. Add `allow_delegation: Option<bool>` to the Agent config struct
   (likely in `gateway/gateway-services/src/agents.rs`). Default to
   `Some(true)` for backwards compatibility with existing agents that
   rely on delegation.
2. Wrap the registration at executor.rs:607:
   ```rust
   if agent_config.allow_delegation.unwrap_or(true) {
       tool_registry.register(Arc::new(DelegateTool::new()));
   }
   ```
3. Set `"allowDelegation": false` on the gp entry in
   `gateway/templates/default_agents.json`.

The LLM physically cannot call a tool that isn't in its function list,
so this is enforcement, not advice.

## Acceptance criteria

- Cron-fired cleanup runs execute entirely inside `general-purpose`.
- No `delegate_to_agent` tool call in gp's session transcript.
- Existing agents (planner, builder, research, writer) continue to
  delegate as before — `allow_delegation` defaults true.
- Setting `allow_delegation: false` on any agent removes the tool from
  its function list at session start.

## Notes

- This is an enforcement issue, not a prompt-engineering issue.
  Strengthening gp's prompt further is unlikely to help — LLMs ignore
  prose that contradicts available tools.
- A future enhancement could expose `allow_delegation` as a checkbox in
  the agent edit UI so users can toggle it per agent without editing
  the JSON config.
