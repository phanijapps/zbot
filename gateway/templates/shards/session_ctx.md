<session_ctx_usage>
Every session in this system carries shared context accessible to all agents via the memory tool. You will see a `<session_ctx ... />` tag in your task prefix with these runtime values:

- `sid` — session id (e.g. `sess-beb261fd`)
- `ward` — the active ward name
- `step` — which step of the plan you are executing (e.g. `3/7`). Absent for ad-hoc single-step delegations.
- `prior_states` — execution ids of completed prior subagents in this session.

Read shared context with the memory tool:

```
memory(action="get_fact", key="ctx.<sid>.<field>")
```

Canonical fields, root-owned (you cannot overwrite these):

| Field | Content | When to fetch |
|---|---|---|
| `intent` | Intent-analyzer's interpretation of the user's ask (ward pick, skill matches, approach) | Before making a plan-shape decision |
| `prompt` | User's original message verbatim | When you need the exact wording the user used |
| `plan` | Current execution plan | At every turn, if you're executing a plan step |
| `ward_briefing` | Ward-tree snapshot captured at session start | When you need to know what else is in this ward |

Per-step handoff fields, each owned by the subagent that wrote it:

| Field | Content |
|---|---|
| `state.<exec_id>` | Summary of what a prior subagent did: its artifacts, imports used, key findings, handoff notes |

To read all prior handoffs, iterate over `prior_states` and call `get_fact` for each. Agents that come after yours will read the fact YOU write via `respond()` the same way.

Usage rules:
- **Read on-demand, not speculatively.** Fetch only the fields you need for the current step. Each `get_fact` call adds the fact content to your conversation context.
- **You cannot write root-owned keys.** The memory tool rejects `save_fact(category="ctx", key="ctx.<sid>.intent", ...)` from subagents with a clear error. Your `respond()` output auto-populates `state.<your_exec_id>` — you do not need to write it manually.
- **The namespace is session-scoped.** Your reads never leak from other sessions; a TSLA session's ctx does not contaminate an AAPL session's recall. Ctx facts are excluded from fuzzy `recall` entirely — only `get_fact` by exact key surfaces them.
- **When Step N's summary is what you need, ask `get_fact` for that exec_id directly.** Don't grep the ward for file traces; the prior agent told you what it did.

If a `<session_ctx />` tag is absent from your task, you are not inside a managed session — the memory tool is still available but ctx fields may return `{found: false}`.
</session_ctx_usage>
