## 1. Separate decision layers

**Problem**
Your agent can interpret, plan, execute, use tools, and write memory in the same flow. That makes reasoning messy and hard to debug.

**Why it is a big deal**
When one loop does everything, bad assumptions spread fast. A weak interpretation becomes a weak plan, then bad execution, then polluted memory. The system looks autonomous but becomes brittle.

**How to solve it**
Split the runtime into explicit stages:
Interpret → Plan → Execute → Verify → Commit memory.
Each stage should produce structured output. Memory writes should only happen after verification, not during raw execution.

**My notes**
Partially agree. The critique assumes z-Bot runs everything in one undifferentiated loop — it doesn't. The actual flow is already layered:

- **Intent analysis middleware** (interpret) — runs before execution, selects relevant skills/agents/wards
- **Recall injection** (context) — fires at session start, injects prioritized facts + episodes
- **Executor loop** (plan + execute) — LLM calls + tool execution with turn budgets and stuck-loop detection
- **Distillation** (commit memory) — fires AFTER session completion, not during execution

Memory writes do NOT happen during raw execution. The `save_fact` tool lets the agent explicitly save during a session, but distillation (the bulk writer) runs post-session. Strategy emergence requires 2+ repeated successes — it won't commit bad patterns from a single session.

Where I agree: the interpret/plan distinction inside the executor IS blended — the LLM interprets and plans in the same call. But splitting that into separate LLM calls would double latency and token cost for marginal gain. The middleware pipeline is the right separation boundary. Full stage isolation is an academic ideal that hurts latency in production agents.

**Verdict: No change needed. The existing architecture already separates the layers that matter. The separation is in the pipeline, not inside the LLM call.**

---

## 2. Add retrieval gates

**Problem**
If recall happens only at startup, the agent misses useful information learned during the session. If recall happens too often, the context gets noisy.

**Why it is a big deal**
Too little recall causes drift. Too much recall causes confusion, token waste, and wrong prioritization. Both reduce reliability on long tasks.

**How to solve it**
Trigger recall only on important events: plan change, failure, contradiction, subagent handoff, ward entry, or final synthesis. Make retrieval event-driven, not constant.

**My notes**
The critique says "event-driven, not constant." We built turn-based (every N turns), which is the pragmatic middle ground. Here's why:

Event detection itself requires computation. Detecting "plan change" or "contradiction" means either (a) an extra LLM call to classify, or (b) heuristics that are fragile. Turn-based with novelty filtering achieves 80% of the benefit at 10% of the complexity:

- `every_n_turns: 5` — predictable, no detection overhead
- `min_novelty_score: 0.3` — only injects facts the agent doesn't already have
- Key-based dedup via `HashSet<String>` — same fact never injected twice per session
- Configurable in `recall_config.json` — tune without rebuilding

Where the critique IS right: delegation handoff and ward entry are high-value recall triggers. We already added recall at delegation spawn (Task 11). Ward entry recall could be added as a specific hook — when the agent calls the `ward` tool, trigger a recall scoped to that ward. This is worth doing.

**Verdict: Keep turn-based as the baseline. Add ward-entry recall as a targeted event trigger. Skip the complex event classification — it's a solution looking for a problem at this scale.**

---

## 3. Make memory typed

**Problem**
If all memory is stored in one generic form, the agent cannot distinguish between facts, procedures, preferences, and temporary observations.

**Why it is a big deal**
A system that mixes memory types will retrieve the wrong thing at the wrong time. That leads to hallucinated continuity and poor steering.

**How to solve it**
Create separate classes for facts, procedures, preferences, episode summaries, unresolved assumptions, and tool outputs. Each type should have its own storage, retrieval rules, and decay policy.

**My notes**
Already done. This is the one area where z-Bot is ahead of the critique.

Current memory types with separate storage and retrieval rules:

| Type | Storage | Retrieval | Decay |
|---|---|---|---|
| **Facts** (user, domain, pattern, instruction, correction) | `memory_facts` with category column | Hybrid search + category priority weights | Confidence * recency * mention_boost |
| **Strategies** (procedural) | `memory_facts` category='strategy' | Boosted 1.4x in recall priority | Only emerges from 2+ successes |
| **Episodes** (session summaries) | `session_episodes` with outcome tracking | Cosine similarity on task_summary embedding | Token cost tracked, failed episodes are warnings |
| **Entities + Relationships** | `kg_entities` + `kg_relationships` in knowledge_graph.db | Graph traversal via `recall_with_graph()` | mention_count + first/last_seen timestamps |
| **Resource indices** (skills, agents, wards) | `memory_facts` category='skill'/'agent'/'ward' | Lower priority weights (0.7-0.8x) | Confidence=1.0, re-indexed each session |

Each type has its own confidence, priority weight, and recall behavior. The recall priority engine (correction 1.5x > strategy 1.4x > user 1.3x > domain 1.0x > pattern 0.9x > indices 0.7x) ensures the right type surfaces at the right time.

What we DON'T have yet: "unresolved assumptions" as a type. This is a good idea for Approach C — facts with a `provisional` flag that decay faster and get promoted or pruned after verification. Worth adding to the roadmap, not blocking now.

**Verdict: Already implemented. Consider adding `provisional` flag for uncertain facts in Approach C.**

---

## 4. Force verification

**Problem**
The agent may act as if progress happened without proving it.

**Why it is a big deal**
This is one of the fastest ways autonomous systems fail. They sound confident, but the work is incomplete, wrong, or unsupported.

**How to solve it**
Require every major step to emit: claim, evidence, confidence, and next step. Add checks after code execution, file reads, tool calls, and memory-based conclusions.

**My notes**
Good principle, wrong layer. This is about the agent's EXECUTION quality, not the memory system.

Action-level verification (claim + evidence + confidence for every tool call) would mean:
- Every `shell` call gets a verification step → 2x token usage
- Every `read` call gets a confidence assessment → adds latency to file reads
- Every delegation produces a structured proof → massively increases output verbosity

At the session level, we already have this via episodic memory: outcome (success/partial/failed), strategy_used, key_learnings. This is the right granularity — the distiller assesses the WHOLE session's success, not each micro-action.

Where the critique IS right: the agent should not claim "done" without evidence. But that's a prompt engineering concern (the system instructions in INSTRUCTIONS.md should say "verify your work before responding"), not an infrastructure concern. Adding structured verification to the memory pipeline would be over-engineering.

The stuck-loop detection in executor.rs (score < 0 = repeated similar actions) is a lightweight form of this — it detects when the agent isn't making real progress.

**Verdict: Session-level verification via episodic memory is sufficient. Action-level verification belongs in prompt engineering, not infrastructure. The meta-cognitive loop already captures what worked and what didn't.**

---

## 5. Give subagents contracts

**Problem**
Subagents can become vague mini-agents with too much freedom and unclear boundaries.

**Why it is a big deal**
Without constraints, subagents drift, overuse tools, ignore context rules, and return inconsistent outputs. That makes orchestration weak.

**How to solve it**
Give each subagent a strict contract: objective, allowed tools, memory scope, input schema, output schema, stop condition, and escalation rule.

**My notes**
Agree this is a real gap, but it's an agent framework concern, not a memory concern.

What z-Bot already has:
- Agent configs with defined tools per agent (`config.yaml` per agent)
- Delegation passes a `task` description
- Delegation semaphore limits concurrency to 3
- Turn budgets (soft 25, hard 50) enforce stop conditions
- Each agent has its own system instructions (SOUL.md + INSTRUCTIONS.md + shards)

What's missing:
- **Output schema enforcement** — delegated agents return free text, not structured responses. The root agent has to parse the result. This causes "vague returns" where the child says "done" without structured deliverables.
- **Memory scope isolation** — child agents can read/write the same `memory_facts` as the root. A child could overwrite a root fact. Ward scoping (what we just built) helps, but agent-level write permissions would be stronger.
- **Escalation rules** — no formal "if stuck, escalate to root instead of thrashing."

The output schema is the highest-value improvement here. A delegation call like `delegate_to_agent("data-analyst", task, expected_output_schema)` where the child MUST return structured data matching the schema. This would dramatically improve orchestration quality.

**Verdict: Output schema for delegations is the single highest-value improvement. Add to Approach C roadmap. Memory scope isolation partially addressed by ward_id. Escalation rules are nice-to-have.**

---

## 6. Use checkpointed plans

**Problem**
If the agent gets lost mid-task, it may not know where it went wrong.

**Why it is a big deal**
Long tasks need recovery. Without checkpoints, one bad step can force a full restart or produce silent corruption.

**How to solve it**
Persist the plan state after each major milestone. Store assumptions, completed actions, pending actions, and artifacts. Allow rollback to the last valid checkpoint.

**My notes**
Partially addressed, partially a real gap.

What exists:
- `update_plan` tool lets the agent persist plan state mid-session
- `execution-state` service tracks session lifecycle (CREATED → RUNNING → COMPLETED/PAUSED/CRASHED)
- Context compaction fires at 80% context window — injects "save important facts now" warning before trimming
- Conversation history is persisted in `messages` table per session

What's missing:
- **No rollback mechanism.** If step 3 of 5 fails, the agent can't "undo" steps 1-2. It either restarts the whole task or tries to patch forward.
- **No assumption tracking.** The plan doesn't record "I assumed X was true when I started step 2." If X turns out false, there's no way to invalidate downstream steps.

Is this critical now? For the financial analysis tasks your agent runs (SPY/PTON analysis), sessions are relatively short (10-30 minutes, 1-5 delegations). Checkpointed rollback matters more for multi-hour tasks with many dependencies.

The episodic memory we just built is a form of cross-session checkpoint: "last time this approach failed at step X, try Y instead." It doesn't help mid-session, but it helps across sessions.

**Verdict: Real gap, but low priority for current task patterns. Add assumption tracking to Approach C. The episodic memory system provides cross-session learning that reduces the need for mid-session rollback.**

---

## 7. Control memory writes

**Problem**
If the agent writes too aggressively, it will store guesses, low-quality summaries, and wrong lessons.

**Why it is a big deal**
Bad memory is worse than no memory because it keeps poisoning future runs.

**How to solve it**
Add a write filter. Only commit durable memory after a critic or verifier pass. Use confidence thresholds and deduplication. Mark uncertain items as provisional, not permanent.

**My notes**
Already implemented. This is where the critique doesn't account for what z-Bot actually has.

Current memory write controls:

1. **Post-session distillation** — memory writes happen AFTER the session completes, not during raw execution. The LLM reviews the full transcript with distance and extracts durable facts. This IS the critic pass.

2. **Confidence thresholds** — facts have confidence 0.0-1.0. High-confidence facts (>= 0.9) get priority recall. Low-confidence facts decay via `recency * confidence * mention_boost` scoring.

3. **Deduplication** — `UNIQUE(agent_id, scope, ward_id, key)` constraint. Repeated observations bump `mention_count` and `MAX(confidence)` via upsert, not duplicate rows.

4. **Strategy emergence threshold** — strategies require 2+ similar successful episodes before being written. A single session can't create a strategy fact.

5. **Ward scoping** — ward-local facts don't pollute global memory. A pattern learned in `finance-ward` stays there unless explicitly global.

What could be stronger:
- **Provisional flag** — mark uncertain facts and promote/prune after N sessions. Currently all facts are "permanent" (with natural decay via recency scoring).
- **Contradiction detection** — if a new fact contradicts an existing one, the system should flag it rather than silently upsert. This is a valuable Approach C feature.

ChatGPT says "bad memory is worse than missing memory." Correct — and that's exactly why distillation runs post-session with LLM review, not mid-execution with raw writes.

**Verdict: Already implemented. The distillation pipeline IS the critic pass. Consider adding provisional flags and contradiction detection in Approach C.**

---

## 8. Score robustness

**Problem**
You may know the system “feels good,” but not where it fails systematically.

**Why it is a big deal**
Without metrics, improvement becomes random. You will keep tuning prompts and components without knowing which change helped.

**How to solve it**
Track: task completion rate, recovery rate after failure, memory precision, memory usefulness, tool misuse, plan drift, and subagent success rate. Review failed runs as a first-class dataset.

**My notes**
Agree this is important, and we have the foundation but not the dashboard.

What we already track:
- **Task completion**: `session_episodes.outcome` (success/partial/failed/crashed) — per session
- **Recovery after failure**: episodic memory surfaces past failures during recall, agent adjusts strategy
- **Memory precision**: `distillation_runs` tracks facts_extracted, entities_extracted per session
- **Token efficiency**: `session_episodes.token_cost` + `agent_executions.tokens_in/out`
- **Distillation health**: `distillation_runs` with success/failed/skipped/permanently_failed status
- **Learning health**: Observatory shows distillation stats in the health bar

What's missing:
- **Memory usefulness score** — are recalled facts actually used by the agent? We'd need to correlate recall injection with agent behavior (did it follow the recalled strategy?). This requires comparing recall content with agent actions — expensive but possible.
- **Plan drift** — how far did the agent deviate from its initial plan? Requires comparing `update_plan` snapshots.
- **Tool misuse rate** — how often do tool calls fail? Already in `execution_logs` (163 warnings, 14 errors), but not surfaced in a dashboard.
- **Subagent success rate** — delegated sessions with outcome tracking. We have this in `session_episodes` now.

The Observatory is the right place for a "Robustness" tab. The data exists in the tables — it needs a UI to surface it.

**Verdict: Foundation is built. The data is there (episodes, distillation_runs, execution_logs). Needs a Robustness dashboard in the Observatory — add to Approach C. The single highest-value metric is: "did the agent produce a correct result without asking follow-up questions?" — that's the north star metric for a goal-oriented agent.**

