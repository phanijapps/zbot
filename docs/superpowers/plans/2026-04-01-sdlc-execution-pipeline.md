# SDLC Execution Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-load skills on delegation (Phase B), add SDLC graph patterns with role-based subagent rules, code-review and domain-validation skills, and structured callbacks (Phase C).

**Architecture:** The `skills` field threads through DelegateAction → StreamEvent::ActionDelegate → DelegationRequest → spawn.rs where skill instructions are pre-loaded. Role detection in spawn.rs adjusts subagent rules (executor vs reviewer). Intent analysis learns SDLC graph patterns. Structured callbacks enable fast root orchestration decisions.

**Tech Stack:** Rust (zero-core, agent-runtime, gateway-execution), YAML skills on disk

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `framework/zero-core/src/event.rs` | Modify | Add `skills: Vec<String>` to `DelegateAction` |
| `runtime/agent-runtime/src/tools/delegate.rs` | Modify | Add `skills` to tool schema + parse into DelegateAction |
| `runtime/agent-runtime/src/types/events.rs` | Modify | Add `skills` to `StreamEvent::ActionDelegate` |
| `runtime/agent-runtime/src/executor.rs` | Modify | Thread `skills` from DelegateAction to StreamEvent |
| `gateway/gateway-execution/src/delegation/context.rs` | Modify | Add `skills` to `DelegationRequest` |
| `gateway/gateway-execution/src/invoke/stream.rs` | Modify | Thread `skills` from ActionDelegate to DelegationRequest |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Modify | Pre-load skills + role detection + adjusted rules |
| `gateway/gateway-execution/src/invoke/setup.rs` | Modify | Role-aware subagent rules |
| `gateway/gateway-execution/src/delegation/callback.rs` | Modify | Structured callback detection |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Modify | SDLC pattern in prompt |
| `~/Documents/zbot/skills/code-review/SKILL.md` | Create | Code review skill |
| `~/Documents/zbot/skills/domain-validation/SKILL.md` | Create | Domain validation skill |

---

### Task 1: Thread `skills` Through Delegation Pipeline

**Files:**
- Modify: `framework/zero-core/src/event.rs:159-182`
- Modify: `runtime/agent-runtime/src/tools/delegate.rs:58-90,92-214`
- Modify: `runtime/agent-runtime/src/types/events.rs:129-137`
- Modify: `runtime/agent-runtime/src/executor.rs:738-747`
- Modify: `gateway/gateway-execution/src/delegation/context.rs:22-48`
- Modify: `gateway/gateway-execution/src/invoke/stream.rs:235-291,309-318`

- [ ] **Step 1: Add `skills` to `DelegateAction`**

In `framework/zero-core/src/event.rs`, add to `DelegateAction` after `output_schema`:

```rust
    /// Skills to pre-load for the subagent.
    /// These skill instructions are injected into the subagent's context at spawn.
    #[serde(default)]
    pub skills: Vec<String>,
```

- [ ] **Step 2: Add `skills` to tool schema and parsing in delegate.rs**

In `runtime/agent-runtime/src/tools/delegate.rs`, add to `parameters_schema()` (after `output_schema` property):

```rust
                "skills": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Skills to pre-load for the subagent. These are loaded into the agent's context automatically."
                }
```

In the `execute` method, parse skills after `output_schema` (after line 137):

```rust
        let skills: Vec<String> = args
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
```

In the `DelegateAction` construction (line 207-214), add:

```rust
            skills,
```

- [ ] **Step 3: Add `skills` to `StreamEvent::ActionDelegate`**

In `runtime/agent-runtime/src/types/events.rs`, add to `ActionDelegate` variant (after `output_schema`):

```rust
        skills: Vec<String>,
```

- [ ] **Step 4: Thread `skills` in executor.rs**

In `runtime/agent-runtime/src/executor.rs` (line 738-747), add `skills` to the ActionDelegate construction:

```rust
                        if let Some(delegate) = &actions.delegate {
                            on_event(StreamEvent::ActionDelegate {
                                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                                agent_id: delegate.agent_id.clone(),
                                task: delegate.task.clone(),
                                context: delegate.context.clone(),
                                wait_for_result: delegate.wait_for_result,
                                max_iterations: delegate.max_iterations,
                                output_schema: delegate.output_schema.clone(),
                                skills: delegate.skills.clone(),
                            });
```

- [ ] **Step 5: Add `skills` to `DelegationRequest`**

In `gateway/gateway-execution/src/delegation/context.rs`, add to `DelegationRequest` (after `output_schema`):

```rust
    /// Skills to pre-load for the subagent.
    pub skills: Vec<String>,
```

- [ ] **Step 6: Thread `skills` in stream.rs**

In `gateway/gateway-execution/src/invoke/stream.rs`, update the `ActionDelegate` destructuring (line 309-316) to include `skills`:

```rust
    if let StreamEvent::ActionDelegate {
        agent_id: child_agent,
        task,
        context,
        max_iterations,
        output_schema,
        skills,
        ..
    } = event
    {
        handle_delegation(ctx, child_agent, task, context, *max_iterations, output_schema, skills);
    }
```

Update `handle_delegation` signature (line 235-241) to accept `skills`:

```rust
pub fn handle_delegation(
    ctx: &StreamContext,
    child_agent: &str,
    task: &str,
    context: &Option<serde_json::Value>,
    max_iterations: Option<u32>,
    output_schema: &Option<serde_json::Value>,
    skills: &Vec<String>,
)
```

Add `skills` to the `DelegationRequest` construction (line 281-291):

```rust
    let _ = ctx.delegation_tx.send(DelegationRequest {
        parent_agent_id: ctx.agent_id.clone(),
        session_id: ctx.session_id.clone(),
        parent_execution_id: ctx.execution_id.clone(),
        child_agent_id: child_agent.to_string(),
        child_execution_id,
        task: task.to_string(),
        context: context.clone(),
        max_iterations,
        output_schema: output_schema.clone(),
        skills: skills.clone(),
    });
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 8: Commit**

```bash
git add framework/zero-core/src/event.rs runtime/agent-runtime/src/tools/delegate.rs runtime/agent-runtime/src/types/events.rs runtime/agent-runtime/src/executor.rs gateway/gateway-execution/src/delegation/context.rs gateway/gateway-execution/src/invoke/stream.rs
git commit -m "feat(delegation): thread skills parameter through delegation pipeline"
```

---

### Task 2: Pre-Load Skills at Spawn + Role Detection

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs:140-222`
- Modify: `gateway/gateway-execution/src/invoke/setup.rs:257-280`

- [ ] **Step 1: Add role detection function to setup.rs**

In `gateway/gateway-execution/src/invoke/setup.rs`, add before `append_system_context`:

```rust
/// Subagent execution role — determines which rules are injected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SubagentRole {
    /// Write code, build things, run scripts. Strict rules.
    Executor,
    /// Review code, validate output, evaluate quality. Relaxed rules.
    Reviewer,
}

/// Detect subagent role from agent name and task description.
pub fn detect_subagent_role(agent_id: &str, task: &str) -> SubagentRole {
    let task_lower = task.to_lowercase();
    let review_signals = [
        "review", "validate", "verify", "evaluate",
        "check quality", "assess", "qa", "audit",
    ];

    if review_signals.iter().any(|s| task_lower.contains(s)) {
        SubagentRole::Reviewer
    } else {
        SubagentRole::Executor
    }
}

/// Get subagent rules for the given role.
pub fn subagent_rules(role: SubagentRole) -> &'static str {
    match role {
        SubagentRole::Executor => "\n\n# --- SUBAGENT RULES ---\n\
            You are a specialist executing a specific task. Do NOT create complex plans.\n\
            Execute your task directly in as few tool calls as possible.\n\
            Use apply_patch for ALL file creation and editing.\n\
            If your task fails after 2 attempts, respond with what you accomplished and what failed.\n",
        SubagentRole::Reviewer => "\n\n# --- SUBAGENT RULES ---\n\
            You are reviewing work produced by another agent. Think critically and independently.\n\
            1. Read the specs and the implementation carefully before forming opinions.\n\
            2. Run the code and examine actual output — don't trust claims.\n\
            3. Evaluate with domain expertise — are values reasonable? Is data complete?\n\
            4. Report your findings in structured format.\n\n\
            ## Report Format\n\
            End your response with EXACTLY one of:\n\
            RESULT: APPROVED\n\
            or\n\
            RESULT: DEFECTS\n\
            - {file_or_output}: {issue} (severity: high|medium|low)\n",
    }
}
```

- [ ] **Step 2: Update `append_system_context` to accept role**

Replace the existing `append_system_context` function signature and body:

```rust
fn append_system_context(instructions: &str, paths: &SharedVaultPaths, role: SubagentRole) -> String {
    let os_context = std::fs::read_to_string(paths.vault_dir().join("config").join("OS.md"))
        .unwrap_or_default();

    let tooling = gateway_templates::Templates::get("shards/tooling_skills.md")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_default();

    let memory_shard = gateway_templates::Templates::get("shards/memory_learning.md")
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
        .unwrap_or_default();

    let rules = subagent_rules(role);

    format!(
        "{}\n\n# --- SYSTEM CONTEXT ---\n\n{}\n\n{}\n\n{}{}",
        instructions, os_context, tooling, memory_shard, rules
    )
}
```

- [ ] **Step 3: Update callers of `append_system_context`**

Search for all calls to `append_system_context` in setup.rs and update to pass `SubagentRole::Executor` (the default). The actual role-aware call will come from spawn.rs.

- [ ] **Step 4: Pre-load skills and detect role in spawn.rs**

In `gateway/gateway-execution/src/delegation/spawn.rs`, after loading the agent (line 143) and before building the executor (line 201), add skill pre-loading and role detection:

```rust
    // Pre-load requested skills into agent instructions
    if !request.skills.is_empty() {
        let mut skill_sections = String::new();
        for skill_name in &request.skills {
            match skill_service.get(skill_name).await {
                Ok(skill) => {
                    skill_sections.push_str(&format!(
                        "\n## Skill: {}\n{}\n",
                        skill.name, skill.instructions
                    ));
                }
                Err(e) => {
                    tracing::warn!(skill = %skill_name, error = %e, "Failed to pre-load skill for subagent");
                }
            }
        }
        if !skill_sections.is_empty() {
            agent.instructions.push_str(&format!(
                "\n# Pre-Loaded Skills\n{}\n",
                skill_sections
            ));
        }
    }

    // Detect subagent role for rule injection
    let role = gateway_execution::invoke::setup::detect_subagent_role(
        &request.child_agent_id,
        &request.task,
    );
    tracing::info!(
        child_agent = %request.child_agent_id,
        role = ?role,
        skills_loaded = request.skills.len(),
        "Subagent spawn: role={:?}, skills pre-loaded={}",
        role, request.skills.len()
    );
```

Then pass the `role` to the system context builder. This requires making `append_system_context` (or equivalent) accept the role. The `ExecutorBuilder.build()` calls `append_system_context` internally — so we need to pass role through the builder.

The simplest approach: add a `with_subagent_role` method to `ExecutorBuilder`:

In `gateway/gateway-execution/src/invoke/executor.rs`, add:

```rust
    pub fn with_subagent_role(mut self, role: crate::invoke::setup::SubagentRole) -> Self {
        self.subagent_role = Some(role);
        self
    }
```

And use it in spawn.rs:

```rust
    let mut builder = ExecutorBuilder::new(paths.vault_dir().clone(), tool_settings)
        .with_workspace_cache(workspace_cache)
        .with_model_registry(model_registry)
        .with_delegated(true)
        .with_subagent_role(role);
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 6: Run tests**

Run: `cargo test --workspace 2>&1 | grep FAILED`
Expected: No failures (except pre-existing zero-core doctest)

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-execution/src/delegation/spawn.rs gateway/gateway-execution/src/invoke/setup.rs gateway/gateway-execution/src/invoke/executor.rs
git commit -m "feat(spawn): pre-load skills and detect subagent role at spawn time"
```

---

### Task 3: Structured Callback Detection

**Files:**
- Modify: `gateway/gateway-execution/src/delegation/callback.rs:32-63`

- [ ] **Step 1: Add structured result extraction**

In `callback.rs`, add helper functions before `format_callback_message`:

```rust
/// Extract the RESULT line from a subagent response.
/// Looks for "RESULT: APPROVED" or "RESULT: DEFECTS" near the end.
fn extract_result_line(response: &str) -> Option<&str> {
    response
        .lines()
        .rev()
        .take(20) // Only check last 20 lines
        .find(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("RESULT: APPROVED") || trimmed.starts_with("RESULT: DEFECTS")
        })
}

/// Extract defect lines after RESULT: DEFECTS.
fn extract_defects(response: &str) -> String {
    let mut in_defects = false;
    let mut defects = Vec::new();

    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("RESULT: DEFECTS") {
            in_defects = true;
            continue;
        }
        if in_defects && trimmed.starts_with("- ") {
            defects.push(trimmed.to_string());
        }
    }

    defects.join("\n")
}
```

- [ ] **Step 2: Update `format_callback_message` to detect structured results**

Replace `format_callback_message`:

```rust
pub fn format_callback_message(
    agent_id: &str,
    response: &str,
    conversation_id: &str,
) -> String {
    let agent_display_name = format_agent_display_name(agent_id);

    let response_content = if response.is_empty() {
        "_No response generated._".to_string()
    } else if response.trim().starts_with('{') || response.trim().starts_with('[') {
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(response) {
            format!(
                "```json\n{}\n```",
                serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| response.to_string())
            )
        } else {
            response.to_string()
        }
    } else {
        response.to_string()
    };

    // Check for structured review result
    let action_hint = if let Some(result_line) = extract_result_line(response) {
        if result_line.contains("APPROVED") {
            "\n\n**Action:** This node APPROVED. Proceed to the next node in the execution plan.".to_string()
        } else if result_line.contains("DEFECTS") {
            let defects = extract_defects(response);
            format!(
                "\n\n**Action:** DEFECTS found. Re-delegate to coding agent with these defects:\n{}",
                defects
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    format!(
        "## From {}\n\n{}{}\n\n---\n_Conversation: `{}`_\n\n\
         [Recall] Delegation completed. Consider recalling to absorb any new learnings.",
        agent_display_name, response_content, action_hint, conversation_id
    )
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/delegation/callback.rs
git commit -m "feat(callback): detect structured RESULT/DEFECTS in subagent responses"
```

---

### Task 4: SDLC Pattern in Intent Analysis Prompt

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs:90-144`

- [ ] **Step 1: Add SDLC pattern to INTENT_ANALYSIS_PROMPT**

In the `## Rules` section of `INTENT_ANALYSIS_PROMPT` (around line 106-116), add this block:

```rust
// Add this text to the INTENT_ANALYSIS_PROMPT raw string, in the Rules section:

r#"
- SDLC Pattern (use when the task involves writing code that produces data, analysis, or reports):
  Node sequence: specs → coding → code_review → domain_validation → output
  - specs (agent: root, skills: [coding]): Write detailed implementation specs in specs/<domain>/*.md
  - coding (agent: code-agent, skills: [coding, ...domain skills]): Build core/ modules + task scripts per spec. Test each module.
  - code_review (agent: code-agent, skills: [code-review]): Review code against specs. Run tests. Report RESULT: APPROVED or RESULT: DEFECTS.
  - domain_validation (agent: data-analyst or research-agent, skills: [domain-validation, ...domain skills]): Run code, evaluate output quality. Report RESULT: APPROVED or RESULT: DEFECTS.
  - output (agent: writing-agent or root, skills: [premium-report or relevant]): Produce final deliverable.
  Use conditional edges for feedback loops:
    code_review → coding (when: "DEFECTS found")
    code_review → domain_validation (when: "APPROVED")
    domain_validation → coding (when: "DEFECTS found")
    domain_validation → output (when: "APPROVED")
"#
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gateway-execution`
Expected: No errors

- [ ] **Step 3: Run intent analysis tests**

Run: `cargo test -p gateway-execution --test intent_analysis_tests`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat(intent): add SDLC pattern to intent analysis prompt"
```

---

### Task 5: Create Code Review Skill

**Files:**
- Create: `~/Documents/zbot/skills/code-review/SKILL.md`

- [ ] **Step 1: Create skill directory and SKILL.md**

```bash
mkdir -p ~/Documents/zbot/skills/code-review
```

Write `~/Documents/zbot/skills/code-review/SKILL.md`:

```yaml
---
name: code-review
description: Review code against specs for quality, correctness, modularity, and adherence to ward conventions.
category: quality
---
```

Instructions body:

```markdown
# Code Review Protocol

You are reviewing code written by another agent. Be thorough and critical.

## Step 1: Read the Spec

Find the spec(s) in `specs/` that correspond to the code being reviewed.
Check all 8 mandatory sections: Purpose, Inputs, Outputs, Algorithm, Dependencies, Error handling, Validation, Core module candidates.

## Step 2: Read the Code

Read every file the coding agent created or modified.
For each file, check:
- Does it match the spec's algorithm and data flow?
- Does it use core/ modules where they exist? (check AGENTS.md for available modules)
- Is it under 100 lines?
- Is error handling present for API calls, file I/O, missing data?
- Are there hardcoded values that should be parameters?

## Step 3: Run the Code

Execute the scripts and verify:
- No runtime errors
- Output files are created at the paths specified in the spec
- Output format matches the spec's schema

## Step 4: Verify Output Structure

Read the output files. Check:
- JSON files parse correctly and have all expected keys
- CSV files have expected columns and reasonable row counts
- Values are within expected ranges (no NaN, no zeros where there shouldn't be)

## Step 5: Report

End your response with EXACTLY one of:

RESULT: APPROVED

or

RESULT: DEFECTS
- {file}: {issue} (severity: high|medium|low)
- {file}: {issue} (severity: high|medium|low)

Severity guide:
- high: Wrong algorithm, missing functionality, runtime errors, data loss
- medium: Missing error handling, hardcoded values, no validation
- low: Style issues, could be more modular, minor optimization
```

- [ ] **Step 2: Commit**

```bash
git add ~/Documents/zbot/skills/code-review/SKILL.md
git commit -m "feat: add code-review skill for SDLC pipeline"
```

---

### Task 6: Create Domain Validation Skill

**Files:**
- Create: `~/Documents/zbot/skills/domain-validation/SKILL.md`

- [ ] **Step 1: Create skill directory and SKILL.md**

```bash
mkdir -p ~/Documents/zbot/skills/domain-validation
```

Write `~/Documents/zbot/skills/domain-validation/SKILL.md`:

```yaml
---
name: domain-validation
description: Run code against live data and evaluate output quality with domain expertise. Spot-check values and verify completeness.
category: quality
---
```

Instructions body:

```markdown
# Domain Validation Protocol

You are validating the output of code written by another agent. Your job is to evaluate data quality and correctness, not code style.

## Step 1: Understand Expected Output

Read the spec(s) in `specs/` to understand:
- What data should be produced
- Expected schemas and value ranges
- Validation criteria from the spec

## Step 2: Run the Code

Execute the scripts that produce output. If they've already been run, re-run to verify reproducibility.

## Step 3: Evaluate Output Quality

For each output file:

### Completeness
- Are all expected fields/columns present?
- Is the data volume reasonable? (e.g., 252 trading days for 1-year daily data)
- Are there missing values or empty sections?

### Correctness
- Spot-check values against known benchmarks (public data, common sense)
- Are calculated values within expected ranges? (RSI: 0-100, prices: positive, percentages: -100 to +100)
- Do aggregations match source data? (sum of weights = 100%, moving averages are actually averages)

### Anomalies
- All zeros where there should be variation
- NaN or null values in required fields
- Suspiciously round numbers (all values ending in .00)
- Dates outside expected range
- Duplicate records

### Domain-Specific Checks
- Financial: Do options prices decrease with distance from ATM? Is put-call parity roughly held?
- Statistical: Are standard deviations positive? Are correlations between -1 and 1?
- Time series: Are dates monotonically increasing? No gaps on trading days?

## Step 4: Report

End your response with EXACTLY one of:

RESULT: APPROVED

or

RESULT: DEFECTS
- {output_file}: {issue} (severity: high|medium|low)
- {output_file}: {issue} (severity: high|medium|low)

Severity guide:
- high: Wrong values, missing critical data, calculations don't match spec
- medium: Incomplete data, values at boundary of expected range, minor gaps
- low: Formatting issues, unnecessary precision, non-critical missing fields
```

- [ ] **Step 2: Commit**

```bash
git add ~/Documents/zbot/skills/domain-validation/SKILL.md
git commit -m "feat: add domain-validation skill for SDLC pipeline"
```

---

### Task 7: Final Integration Verification

- [ ] **Step 1: Full workspace compilation**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 2: Full test suite**

Run: `cargo test --workspace 2>&1 | grep -E "(test result|FAILED)"`
Expected: All pass (except pre-existing zero-core doctest)

- [ ] **Step 3: Verify skill files exist**

Run: `ls ~/Documents/zbot/skills/code-review/SKILL.md ~/Documents/zbot/skills/domain-validation/SKILL.md`
Expected: Both exist

- [ ] **Step 4: Verify coding skill has ward_setup**

Run: `grep ward_setup ~/Documents/zbot/skills/coding/SKILL.md`
Expected: Found

- [ ] **Step 5: Commit any remaining changes**

```bash
git add -A
git commit -m "feat: SDLC execution pipeline — complete implementation"
```
