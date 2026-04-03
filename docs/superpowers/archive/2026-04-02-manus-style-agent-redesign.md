# Agent System Redesign — Harness-Enforced Goal-Oriented Orchestration

## Problem Statement

The agent system relies on prompts to enforce behavior. This fails:
1. "ONE AT A TIME" → model batches 3 tool calls into one garbled JSON blob
2. "Do NOT call list_skills" → model calls it anyway
3. "Delegate to planner-agent" → model skips planner and dumps everything on one agent
4. "Do NOT load skills" → model loads skills anyway
5. Compaction drops messages blindly — no compression, no preservation
6. Summarization middleware exists but is dead code — never registered

**Root cause:** Prompts describe, they don't enforce. The harness (runtime, gateway, executor) must enforce constraints. The prompts should only describe context, identity, and the current plan.

## Design Principles

From Manus research + production multi-agent systems:

1. **Harness enforces, prompts describe.** Use code to prevent, prompts to inform.
2. **Planner is a module, not an agent.** Runs before executor, injects plan into context.
3. **One action per iteration.** Runtime executes only the first tool call. Extras dropped.
4. **Plan file as attention.** Agent maintains plan.md, reciting goals into recent context.
5. **Compress, don't delete.** Old messages get compressed to one-liners, not dropped.
6. **Remove tools, don't prohibit them.** If root can't load skills, it won't try.
7. **Stable prompt prefix.** System prompt sections are static. Dynamic content appended.

---

## Change 1: Single Tool Call Enforcement

### Problem
minimax-m2.7 outputs `{"action":"recall",...}{"title":"..."}{"action":"use",...}` — three tool calls concatenated. The JSON parser sees "trailing characters" and fails. The model retries the same garbled format 3-5 times, wasting turns.

### Solution

**A. Recover garbled concatenated JSON** (in openai.rs parse error path):
```rust
// When serde_json fails with "trailing characters", attempt to extract first JSON object
if acc.arguments.contains("}{") {
    let split_pos = acc.arguments.find("}{").unwrap() + 1;
    let first_json = &acc.arguments[..split_pos];
    if let Ok(args) = serde_json::from_str::<Value>(first_json) {
        tracing::info!(
            "Recovered first JSON object from concatenated tool calls for '{}'",
            acc.name
        );
        // Use recovered args instead of error
    }
}
```

**B. Truncate to single tool call** (in executor.rs, configurable):
```rust
// ExecutorConfig
pub single_action_mode: bool,  // default: false

// In executor loop, after collecting tool_calls:
if self.config.single_action_mode && tool_calls.len() > 1 {
    tracing::info!(
        "Single-action mode: executing '{}', dropping {} extra tool calls",
        tool_calls[0].name, tool_calls.len() - 1
    );
    tool_calls.truncate(1);
}
```

Root gets `single_action_mode: true`. Subagents keep `false` (they can parallel-call tools).

**C. Better error message** when parse fails (teaches model):
```
"Tool call failed: only one tool call per response. You sent multiple calls concatenated.
Call one tool, wait for the result, then call the next."
```

### Files
- `runtime/agent-runtime/src/llm/openai.rs` — garbled JSON recovery in parse error path
- `runtime/agent-runtime/src/executor.rs` — single_action_mode on ExecutorConfig + enforcement
- `gateway/gateway-execution/src/invoke/executor.rs` — set single_action_mode for root

---

## Change 2: Planner as Pre-Execution Module

### Problem
Planner-agent delegation adds 30-60s round-trip. Root sometimes ignores the "delegate to planner" instruction. The plan comes back as a delegation result buried in conversation history — subject to compaction.

### Solution
Run planning as a synchronous module (like intent analysis) BEFORE the executor starts. The plan is injected into the system prompt — root sees it from turn 1.

**Flow:**
```
User request
  → intent_analysis() → approach=graph
  → run_planner_module() → reads ward, produces structured plan
  → plan injected into system prompt
  → executor starts with plan already in context
  → root executes plan step by step
```

**The planner module:**
1. Reads ward AGENTS.md + memory-bank/core_docs.md + memory-bank/structure.md
2. Reads available agents + skills from intent analysis result
3. Makes ONE LLM call (same pattern as intent analysis — synchronous, non-streaming)
4. Prompt: structured system prompt with ward context + goal → outputs execution plan
5. Returns structured plan text

**Plan injection (appended to system prompt end for cache stability):**
```
<execution_plan>
Goal: Comprehensive NVDA analysis with trading signals, options plays, catalyst timeline, HTML report
Ward: financial-analysis
Subdirectory: nvda/

Step 1: Fetch OHLCV data [agent: code-agent] [depends: none] [output: nvda/data/]
Step 2: Calculate technical indicators [agent: code-agent] [depends: 1] [output: nvda/technicals.json] [reuse: core/indicators.py]
Step 3: Analyze options chain [agent: code-agent] [depends: 1] [output: nvda/options.json] [reuse: core/options.py]
Step 4: Research catalysts [agent: research-agent] [depends: none] [output: nvda/catalysts.json]
Step 5: Synthesize analysis [agent: data-analyst] [depends: 2,3,4] [output: nvda/summary.json]
Step 6: Generate HTML report [agent: code-agent] [depends: 5] [output: nvda/report.html]

Execute each step by delegating to the assigned agent. After each delegation completes, update plan.md in the ward.
</execution_plan>
```

**For simple tasks (approach=simple):** planner module doesn't run. No plan injected.

### Files
- `gateway/gateway-execution/src/middleware/intent_analysis.rs` — new `run_planner_module()` function
- `gateway/gateway-execution/src/runner.rs` — call planner module after intent analysis
- Remove planner-agent delegation from `format_intent_injection()`

---

## Change 3: Structured Prompt Sections

### Problem
Conversational markdown ("Do NOT call list_skills()") reads as suggestion. Model ignores.

### Solution
Use structured sections with XML-like tags. Each section has a clear purpose. Model treats tagged sections as constraints.

**Root system prompt structure:**
```
<agent_identity>
You are Jaffa, an orchestrator agent. You receive goals, delegate to specialist agents,
review results, and synthesize deliverables. You never do specialized work yourself.
</agent_identity>

<agent_loop>
Each turn, perform exactly ONE action:
1. Read the latest result in context
2. Decide the next action based on the execution plan
3. Call exactly one tool
4. Wait for the system to return the result
Repeat until all plan steps are complete, then call respond.
</agent_loop>

<available_agents>
| Agent | Use For |
|-------|---------|
| code-agent | Writing/running code, building data pipelines, ward-centric development |
| data-analyst | Interpreting existing data, generating insights |
| research-agent | Web search, gathering external information |
| writing-agent | Creating formatted documents from existing data |
</available_agents>

<first_actions>
1. memory(action="recall") — recall context for the user's request
2. set_session_title — concise title
3. ward(action="use") — enter the ward from the execution plan
4. Begin executing the plan — delegate step 1 to its assigned agent
</first_actions>

<rules>
- Execute the plan steps in order, respecting dependencies
- Delegate each step to the assigned agent with: goal, ward name, acceptance criteria
- Review each result before proceeding to the next step
- After each delegation, update plan.md in the ward with step status
- Call respond only when ALL steps are complete
</rules>
```

**Why tags work better than markdown:**
- Models are trained on XML/HTML — they parse tag boundaries as scope delimiters
- Rules inside `<rules>` are treated as harder constraints than rules in bold markdown
- Each section is self-contained — the model knows where to look for what

### Files
- `gateway/templates/shards/first_turn_protocol.md` — rewrite with tags
- `gateway/templates/shards/planning_autonomy.md` — rewrite with tags
- `gateway/templates/instructions_starter.md` — rewrite with tags
- `~/Documents/zbot/config/INSTRUCTIONS.md` — user config copy
- `~/Documents/zbot/config/shards/*.md` — user config copies

---

## Change 4: plan.md as Attention Mechanism

### Problem
Root loses track of the plan across continuations. The plan is in the planner's delegation result, which gets compacted. By continuation 3, root doesn't know what step it's on.

### Solution
Root creates `plan.md` in the ward as its first action after entering the ward. Updates it after each delegation completes. This file serves three purposes:

1. **Attention** — root reads it each continuation, reciting goals into recent context
2. **Continuity** — file persists across continuations (file-system-as-memory)
3. **Visibility** — user/UI can check progress

**Format:**
```markdown
# Execution Plan — NVDA Analysis

## Steps
- [x] Step 1: Fetch OHLCV data (code-agent) — ✓ 251 rows fetched
- [x] Step 2: Technical indicators (code-agent) — ✓ RSI=46, MACD bearish
- [ ] Step 3: Options chain (code-agent) — IN PROGRESS
- [ ] Step 4: Catalysts (research-agent) — PENDING
- [ ] Step 5: Synthesize (data-analyst) — PENDING
- [ ] Step 6: HTML report (code-agent) — PENDING
```

**Root's behavior:**
- After entering ward: create plan.md from execution plan
- After each delegation completes: update plan.md (mark done, add brief result)
- Each continuation: read plan.md first to know where it left off

**In the system prompt:**
```
<plan_management>
After entering the ward, create plan.md with the execution plan steps.
After each delegation completes, update plan.md — mark step done, note key result.
On continuation, read plan.md first to know your current position.
</plan_management>
```

### Files
- System prompt shards (instructions for root)
- No code changes needed — uses existing apply_patch/shell tools

---

## Change 5: Remove Tools Root Shouldn't Have

### Problem
Root calls `load_skill`, `list_skills`, `list_agents`, `apply_patch`, `shell` despite prompts saying "Do NOT." Prompt enforcement fails.

### Solution
Don't register these tools for root's executor. If the tool doesn't exist in the tool registry, the model physically cannot call it.

**Root keeps:**
- `memory` — recall context
- `set_session_title` — session management
- `ward` — workspace management
- `update_plan` — plan tracking (existing tool)
- `delegate_to_agent` — core orchestration
- `respond` — final response
- `grep` — read files when reviewing results
- `shell` — read-only commands for checking delegation results (restrict to read-only?)

**Root loses:**
- `load_skill` — subagents load their own
- `list_skills` — intent analysis provides
- `list_agents` — intent analysis provides
- `apply_patch` — root doesn't write files

**Implementation:**
In `gateway/gateway-execution/src/invoke/executor.rs`, when building the executor for root, filter the tool registry:

```rust
if is_root {
    // Root is an orchestrator — remove specialist tools
    let blocked_tools = ["load_skill", "list_skills", "list_agents", "apply_patch"];
    for tool_name in &blocked_tools {
        tool_registry.remove(tool_name);
    }
}
```

Or better: build a separate tool set for root vs specialists at registration time.

### Files
- `gateway/gateway-execution/src/invoke/executor.rs` — filter tool registry for root
- Or `runtime/agent-runtime/src/tools/mod.rs` — separate root vs specialist tool sets

---

## Change 6: Compress, Don't Delete — Fix Compaction

### Problem
`compact_messages()` drops messages wholesale. KEEP_RECENT=20 messages kept, everything else deleted with a "[N messages trimmed]" notice. The agent loses all context from dropped messages — file paths, decisions, intermediate results.

The ContextEditingMiddleware (with pattern-based compression from Phase 3) exists but is NEVER registered in the middleware pipeline. The SummarizationMiddleware is also dead code.

### Solution

**A. Activate the middleware pipeline.** Register ContextEditingMiddleware in the executor builder:

```rust
// In gateway/gateway-execution/src/invoke/executor.rs
let mut pipeline = MiddlewarePipeline::new();
pipeline.add_pre_process(Box::new(ContextEditingMiddleware::new(
    ContextEditingConfig {
        enabled: true,
        trigger_tokens: (context_window * 70 / 100) as usize, // trigger at 70%
        keep_tool_results: 10,
        min_reclaim: 1000,
        clear_tool_inputs: true,
        cascade_unload: true,
        skill_aware_placeholders: true,
        ..Default::default()
    }
)));
```

This activates:
- Tool result clearing (old tool outputs → placeholder)
- Skill-aware cascade unloading
- Pattern-based assistant message compression (our Phase 3 work)
- In-place editing (Phase 1 optimization)

**B. Improve compact_messages() — compress before dropping:**

Before dropping old messages, run compression:
1. Compress assistant messages to `[Turn N: tool1(file1), tool2(file2)]`
2. Clear tool result content but preserve tool_call_id pairing
3. Keep file paths and key values in compressed form
4. Only drop if still over budget after compression

```rust
fn compact_messages(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    const KEEP_RECENT: usize = 20;

    if messages.len() <= KEEP_RECENT + 2 {
        return messages;
    }

    // Phase 1: Compress old messages (don't drop yet)
    let compress_boundary = messages.len().saturating_sub(KEEP_RECENT);
    let mut messages = messages;
    compress_old_assistant_messages(&mut messages, KEEP_RECENT);

    // Phase 2: Clear old tool results (replace with placeholders)
    for i in 0..compress_boundary {
        if messages[i].role == "tool" {
            messages[i].content = "[result cleared — see compressed turn above]".to_string();
        }
    }

    // Phase 3: If still over budget, drop (existing logic)
    // ... but now the remaining messages are much smaller
}
```

**C. Restorable compression (Manus-inspired):**

When compressing, preserve URLs and file paths even when dropping content:
```
[Turn 3: write_file(core/data_fetcher.py), shell(python fetch.py) → created nvda/data/ohlcv.csv (251 rows)]
```

This keeps the essential information (what files exist, what was created) while dropping the reasoning and raw output.

### Files
- `gateway/gateway-execution/src/invoke/executor.rs` — register ContextEditingMiddleware
- `runtime/agent-runtime/src/executor.rs` — improve compact_messages() with compression-first
- `runtime/agent-runtime/src/middleware/context_editing.rs` — already has compression (Phase 3)

---

## Implementation Phases

### Phase A: Harness Enforcement (Rust — highest impact)
1. Garbled JSON recovery in openai.rs (parse `}{` concatenation)
2. Single-action mode on ExecutorConfig (root only)
3. Remove tools from root's registry (load_skill, list_skills, list_agents, apply_patch)
4. Register ContextEditingMiddleware in pipeline

### Phase B: Planner Module (Rust — second highest)
1. `run_planner_module()` function (reads ward, one LLM call, returns plan)
2. Integrate into runner.rs after intent analysis
3. Plan injection into system prompt
4. Remove planner-agent delegation from intent injection

### Phase C: Prompts & Attention (templates — no Rust)
1. Rewrite all shards with structured `<section>` tags
2. Add `<plan_management>` instructions for plan.md
3. Update INSTRUCTIONS.md for structured format
4. Update user config copies

### Phase D: Compaction (Rust — quality improvement)
1. Improve compact_messages() with compression-first strategy
2. Activate middleware pipeline with ContextEditingMiddleware
3. Restorable compression (preserve file paths and URLs)

### Phase E: Validation
1. Test stock analysis (full pipeline: plan → code → research → analysis → report)
2. Test non-coding task (homework, RFP response)
3. Test simple task (greeting — no planner, no delegation)
4. Test second run on same ward (core module reuse)
5. Test long session (compaction quality)

---

## Expected Outcomes

| Metric | Before | After |
|--------|--------|-------|
| Garbled tool call recovery | 0% (parse error) | ~90% (first JSON extracted) |
| Root loading skills | Yes (3-5 wasted turns) | Impossible (tool removed) |
| Root calling list_agents | Yes (1-2 wasted turns) | Impossible (tool removed) |
| Time to first delegation | 1.5-8 min (14 tool calls) | 30-60s (3-4 tool calls) |
| Planner round-trip | 30-60s (agent delegation) | 0s (module, pre-injected) |
| Plan visibility across continuations | Lost after compaction | Persistent in plan.md |
| Compaction quality | Drop messages blindly | Compress first, preserve file paths |
| Context editing middleware | Dead code | Active with compression |
