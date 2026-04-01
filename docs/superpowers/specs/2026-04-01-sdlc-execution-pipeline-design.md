# SDLC Execution Pipeline — Design Spec

## Problem

Today the root agent receives an execution graph but doesn't follow it well. Subagents start without domain skills loaded. There's no review loop — code gets written and shipped without quality checks or domain validation. The root agent often does everything itself instead of delegating.

## Vision

Complex code tasks follow a mini SDLC: specs → code → review → domain validation → fix loop → final output. Each stage is a different agent with different expertise. Feedback loops via conditional graph edges drive quality. Subagents start pre-loaded with relevant skills for zero-discovery overhead.

## Design

### Phase B: Auto-Load Skills on Delegation

#### B1: Add `skills` parameter to `delegate_to_agent`

**File:** `runtime/agent-runtime/src/tools/delegate.rs`

Add `skills` to the tool's JSON schema as an optional array of skill names:

```json
"skills": {
    "type": "array",
    "items": { "type": "string" },
    "description": "Skills to pre-load for the subagent. These are loaded into the agent's context automatically. The agent can still discover and load other skills."
}
```

Parse into `DelegateAction`:

```rust
// In DelegateAction (zero-core/src/event.rs or wherever it lives)
pub skills: Vec<String>,
```

#### B2: Pre-load skills at spawn time

**File:** `gateway/gateway-execution/src/delegation/spawn.rs`

In the spawn function, after building the subagent's instructions but before executor creation:

1. Read `delegation.skills` (the list of skill names from the tool call)
2. For each skill name, read `{skills_dir}/{name}/SKILL.md`
3. Parse frontmatter + body (reuse `SkillService::get()` or just `fs::read_to_string`)
4. Prepend skill instructions to the subagent's system context as a `## Loaded Skills` section

```rust
// Pseudocode for spawn.rs
let mut skill_instructions = String::new();
for skill_name in &delegation.skills {
    if let Ok(skill) = skill_service.get(skill_name).await {
        skill_instructions.push_str(&format!(
            "\n## Skill: {}\n{}\n",
            skill.name, skill.instructions
        ));
    }
}
if !skill_instructions.is_empty() {
    agent.instructions.push_str(&format!(
        "\n# Pre-Loaded Skills\n{}\n",
        skill_instructions
    ));
}
```

**Performance:** `fs::read_to_string` on 1-3 files of ~2KB each. Sub-millisecond total.

**Not a filter:** The subagent can still call `load_skill` for any other skill. This is a boost, not a restriction.

### Phase C: Mini SDLC Execution

#### C1: Intent Analysis SDLC Pattern

**File:** `gateway/gateway-execution/src/middleware/intent_analysis.rs`

Add SDLC pattern guidance to `INTENT_ANALYSIS_PROMPT`, in the Rules section:

```
## SDLC Pattern (for code-heavy tasks)

When the task requires writing code that produces data, analysis, or reports, use this execution pattern:

1. **specs** (agent: root) — Write detailed implementation specs. MUST complete before any code.
2. **coding** (agent: code-agent, skills: [coding, ...domain skills]) — Build core/ modules and task scripts per spec. Test each module.
3. **code_review** (agent: code-agent, skills: [code-review]) — Review code against specs. Check quality, modularity, core/ usage. Run tests. Report APPROVED or DEFECTS.
4. **domain_validation** (agent: data-analyst or research-agent, skills: [domain-validation, ...domain skills]) — Run the code against live data. Evaluate output quality with domain expertise. Spot-check values. Report APPROVED or DEFECTS.
5. **output** (agent: writing-agent or root, skills: [premium-report or relevant]) — Produce final deliverable.

Use conditional edges for feedback loops:
- code_review → coding (when: "DEFECTS found — code needs fixes")
- code_review → domain_validation (when: "APPROVED — code is clean")
- domain_validation → coding (when: "DEFECTS found — output quality issues")
- domain_validation → output (when: "APPROVED — data quality verified")

Only use this pattern when the task genuinely involves code. Simple questions, text tasks, and quick lookups should use "simple" approach.
```

#### C2: Subagent Role Detection

**File:** `gateway/gateway-execution/src/delegation/spawn.rs`

After loading the agent but before injecting system context, detect the subagent's role from the delegation context:

```rust
enum SubagentRole {
    Executor,   // Write code, build things, run scripts
    Reviewer,   // Review code, validate output, evaluate quality
}

fn detect_role(agent_id: &str, task: &str) -> SubagentRole {
    let task_lower = task.to_lowercase();
    let review_signals = ["review", "validate", "verify", "evaluate", "check quality", "assess"];

    if review_signals.iter().any(|s| task_lower.contains(s)) {
        SubagentRole::Reviewer
    } else {
        SubagentRole::Executor
    }
}
```

Adjust the injected subagent rules based on role:

**Executor mode** (current behavior):
```
You are a specialist executing a specific task. Do NOT create complex plans.
Execute your task directly in as few tool calls as possible.
Use apply_patch for ALL file creation and editing.
If your task fails after 2 attempts, respond with what you accomplished and what failed.
```

**Reviewer mode** (new):
```
You are reviewing work produced by another agent. Think critically and independently.
1. Read the specs and the implementation carefully before forming opinions.
2. Run the code and examine actual output — don't trust claims.
3. Evaluate with domain expertise — are values reasonable? Is data complete?
4. Report your findings in structured format (see below).

## Report Format
End your response with EXACTLY one of:
RESULT: APPROVED
or
RESULT: DEFECTS
- {file_or_output}: {issue} (severity: high|medium|low)
- {file_or_output}: {issue} (severity: high|medium|low)
```

#### C3: Code Review Skill

**File:** `~/Documents/zbot/skills/code-review/SKILL.md`

```yaml
---
name: code-review
description: Review code against specs for quality, correctness, and adherence to ward conventions.
category: quality
---
```

Skill instructions teach the agent to:
1. Read the spec(s) from `specs/` that correspond to the code being reviewed
2. Read the actual code files
3. Check against spec: are all 8 mandatory sections implemented?
4. Check code quality: uses core/ modules? Max 100 lines per file? Proper error handling?
5. Run the code and verify it executes without errors
6. Verify outputs exist and have expected structure
7. Report structured: APPROVED or DEFECTS with file:line references

#### C4: Domain Validation Skill

**File:** `~/Documents/zbot/skills/domain-validation/SKILL.md`

```yaml
---
name: domain-validation
description: Run code against live data and evaluate output quality with domain expertise.
category: quality
---
```

Skill instructions teach the agent to:
1. Read the spec to understand expected outputs and validation criteria
2. Run the code (or re-run if already executed)
3. Read the output files
4. Evaluate with domain expertise:
   - Are values within expected ranges? (RSI 0-100, prices positive, etc.)
   - Is data complete? (Expected N rows, got N?)
   - Do calculations match known benchmarks? (Spot-check against public data)
   - Are there anomalies? (All zeros, NaN values, suspiciously round numbers)
5. Report structured: APPROVED or DEFECTS with specific issues

#### C5: Structured Callback Detection

**File:** `gateway/gateway-execution/src/delegation/callback.rs`

When formatting the callback message from subagent to root, detect if the response ends with a structured report (`RESULT: APPROVED` or `RESULT: DEFECTS`). If so, format the callback for fast root decision-making:

```rust
fn format_structured_callback(
    node_id: &str,
    agent_id: &str,
    response: &str,
) -> String {
    // Detect structured result at end of response
    if let Some(result_line) = extract_result_line(response) {
        if result_line.contains("APPROVED") {
            format!(
                "## [{}] ✅ APPROVED by {}\nProceed to next node.\n",
                node_id, agent_id
            )
        } else if result_line.contains("DEFECTS") {
            let defects = extract_defects_after_result(response);
            format!(
                "## [{}] ❌ DEFECTS found by {}\n{}\n\n**Action:** Re-delegate to coding agent with this defect list.\n",
                node_id, agent_id, defects
            )
        } else {
            // Unstructured — pass through as-is
            response.to_string()
        }
    } else {
        response.to_string()
    }
}
```

This makes root's continuation call near-instant — it sees "APPROVED → next node" or "DEFECTS → re-delegate with list" and acts mechanically.

## Files Changed

| File | Change |
|------|--------|
| `runtime/agent-runtime/src/tools/delegate.rs` | Add `skills` parameter to tool schema and parsing |
| `zero-core/src/event.rs` | Add `skills: Vec<String>` to `DelegateAction` |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Pre-load skills, detect role, adjust subagent rules |
| `gateway/gateway-execution/src/delegation/callback.rs` | Structured callback detection and formatting |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | SDLC pattern in prompt |
| `~/Documents/zbot/skills/code-review/SKILL.md` | New skill |
| `~/Documents/zbot/skills/domain-validation/SKILL.md` | New skill |

## Out of Scope

- Agent-to-agent delegation (Phase B optimization — future)
- Parallel subagent execution
- New agent creation (reuse existing code-agent, data-analyst, research-agent)
- UI changes for SDLC visualization
