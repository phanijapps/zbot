# Orchestrator Prompt & State Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 5 issues after the goal-oriented orchestrator refactor: delegation completion state, SDLC for coding tasks, ward naming, list_* suppression, and sequential action guidance.

**Architecture:** Task 1 is a code change (executor tracks delegation-stop separately from respond-stop, runner skips `complete_execution` when delegation is pending). Tasks 2-5 are prompt/injection changes only. All changes are backward-compatible.

**Tech Stack:** Rust (agent-runtime, gateway-execution), prompt engineering (shards, intent_analysis.rs)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `runtime/agent-runtime/src/executor.rs` | Modify | Separate delegation stop from respond stop |
| `gateway/gateway-execution/src/runner.rs` | Modify | Skip complete_execution when delegation pending |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Modify | SDLC for coding, ward naming, format_intent_injection |
| `gateway/templates/shards/first_turn_protocol.md` | Modify | Sequential action guidance |
| `gateway/templates/shards/planning_autonomy.md` | Modify | list_* suppression, delegation structure |

---

### Task 1: Don't Mark Root Completed When Delegation Is Pending

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs:860,983,1192-1204`
- Modify: `gateway/gateway-execution/src/runner.rs:857-885`

- [ ] **Step 1: Add delegation tracking flag in executor loop**

In `execute_with_tools_loop`, alongside `should_stop_after_respond` (line 860), add a separate flag:

```rust
            let mut should_stop_after_respond = false;
            let mut stopped_for_delegation = false;
```

- [ ] **Step 2: Set delegation flag instead of reusing respond flag**

Replace the delegation block (line 980-984):

```rust
                                // Delegation claim is set atomically by the delegate tool via try_claim
                                // Stop executor loop — continuation callback will resume root
                                // when the subagent completes.
                                should_stop_after_respond = true;
                                tracing::debug!("Delegation detected, will stop after current tool batch");
```

With:

```rust
                                // Delegation claim is set atomically by the delegate tool via try_claim
                                // Stop executor loop — continuation callback will resume root
                                // when the subagent completes.
                                stopped_for_delegation = true;
                                tracing::debug!("Delegation detected, will stop after current tool batch");
```

- [ ] **Step 3: Break on delegation too**

At the loop exit check (line 1192-1196), add delegation:

```rust
            // If respond tool was called, stop the loop - agent has finished responding
            if should_stop_after_respond || stopped_for_delegation {
                tracing::debug!("Stopping execution loop — respond={} delegation={}",
                    should_stop_after_respond, stopped_for_delegation);
                break;
            }
```

- [ ] **Step 4: Skip Done event when stopped for delegation**

Replace the post-loop Done emission (line 1199-1204):

```rust
        // Emit done event
        on_event(StreamEvent::Done {
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            final_message: full_response.clone(),
            token_count: full_response.len(),
        });
```

With:

```rust
        // Emit done event — but NOT if we stopped for delegation.
        // When delegation is pending, the runner should NOT mark this execution
        // as completed. The continuation callback will resume it later.
        if !stopped_for_delegation {
            on_event(StreamEvent::Done {
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                final_message: full_response.clone(),
                token_count: full_response.len(),
            });
        } else {
            tracing::info!("Executor paused for delegation — skipping Done event");
        }
```

- [ ] **Step 5: Handle missing Done event in runner**

In `runner.rs`, after `execute_stream` returns `Ok(())` (line 857-885), the runner calls `complete_execution`. But if the executor didn't emit `Done` (delegation case), the runner still calls `complete_execution`. We need to check whether a delegation was started.

The runner already has access to `delegation_registry`. Check if the session has active delegations:

```rust
            match result {
                Ok(()) => {
                    // Check if this execution spawned delegations that are still active
                    let has_active_delegations = !delegation_registry
                        .get_by_session_id(&session_id)
                        .is_empty();

                    if has_active_delegations {
                        // Root paused for delegation — do NOT complete execution.
                        // The continuation callback will handle completion.
                        tracing::info!(
                            session_id = %session_id,
                            "Root paused for delegation — skipping execution completion"
                        );

                        // Request continuation so the session resumes when delegations complete
                        if let Err(e) = state_service.request_continuation(&session_id) {
                            tracing::warn!("Failed to request continuation: {}", e);
                        }

                        // Aggregate tokens so UI shows progress
                        if let Err(e) = state_service.aggregate_session_tokens(&session_id) {
                            tracing::warn!("Failed to aggregate session tokens: {}", e);
                        }
                    } else {
                        // Normal completion — no active delegations
                        complete_execution(
                            &state_service,
                            &log_service,
                            &event_bus,
                            &execution_id,
                            &session_id,
                            &agent_id,
                            &conversation_id,
                            Some(accumulated_response),
                            connector_registry.as_ref(),
                            respond_to.as_ref(),
                            thread_id.as_deref(),
                            bridge_registry.as_ref(),
                            bridge_outbox.as_ref(),
                        )
                        .await;

                        // Do NOT cancel delegations on successful completion.
                        // complete_execution() already requests continuation when
                        // pending delegations exist. Cancelling here would remove
                        // delegations from the registry and decrement the counter,
                        // preventing the continuation callback from firing when
                        // the subagent completes.
                    }
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p agent-runtime && cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs gateway/gateway-execution/src/runner.rs
git commit -m "fix: don't mark root completed when delegation is pending — skip Done event + complete_execution"
```

---

### Task 2: Restore SDLC for Coding Tasks Only

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs` (format_intent_injection function)

- [ ] **Step 1: Add SDLC block back, gated on coding skill**

In `format_intent_injection`, after the execution graph section and before the ward rule, add a conditional SDLC block:

```rust
    // SDLC guidance — only when coding is involved
    let has_coding = analysis.recommended_skills.iter().any(|s| s == "coding");
    if has_coding && es.approach == "graph" {
        out.push_str(r#"
**Coding Discipline (for code-agent delegations):**
When delegating coding work, provide the code-agent with:
- A clear goal and acceptance criteria
- The ward name and relevant specs directory
- Reference to AGENTS.md for existing core/ modules
- "Process tasks.json at specs/<path>/tasks.json using ralph.py" for multi-file tasks

For multi-file coding tasks, the code-agent should:
1. Read existing specs or write specs if none exist (one per module, under 3KB)
2. Create tasks.json with ordered tasks (create → run → verify)
3. Execute tasks sequentially using ralph.py
4. Respond with results summary

You do NOT need to write specs yourself — delegate that to code-agent as part of its task.
"#);
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "feat: restore SDLC guidance for coding tasks only — gated on coding skill"
```

---

### Task 3: Fix Ward Naming in Intent Analysis Prompt

**Files:**
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs` (INTENT_ANALYSIS_PROMPT constant)

- [ ] **Step 1: Strengthen ward naming rules**

In `INTENT_ANALYSIS_PROMPT` (line 90), replace:

```
- Wards are domain-level workspaces (e.g., "financial-analysis"), not task-specific. Reuse existing wards.
```

With:

```
- ward_name MUST be a reusable domain category, NEVER task-specific or ticker-specific.
  GOOD: "financial-analysis", "stock-analysis", "market-research", "personal-life", "homework"
  BAD: "amd-stock-analysis", "spy-options-trade", "math-homework-ch5"
  The ward is reused across many tasks in the same domain. Use subdirectory for task-specific paths.
- If an existing ward matches the domain, use action "use_existing" with that ward name.
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/middleware/intent_analysis.rs
git commit -m "fix: strengthen ward naming rules — domain-level only, never task-specific"
```

---

### Task 4: Suppress list_skills/list_agents + Sequential Action Guidance

**Files:**
- Modify: `gateway/templates/shards/first_turn_protocol.md`
- Modify: `gateway/templates/shards/planning_autonomy.md`

- [ ] **Step 1: Update first_turn_protocol.md**

Replace the current content with:

```markdown
GOAL-ORIENTED EXECUTION

You are an autonomous agent. When you receive a task, execute these steps ONE AT A TIME — complete each before starting the next:

1. **Recall context.** Call the memory tool to recall corrections, strategies, domain knowledge, and relevant skills/agents. This gives you targeted results via embeddings — do NOT call list_skills() or list_agents() separately.
2. **Set title.** Call set_session_title with a concise title (2-8 words).
3. **Set up workspace.** Switch to the appropriate ward based on recalled context and intent analysis.
4. **Understand the goal.** What does the user actually want achieved? Look beyond the literal request — infer the full scope, quality expectations, and implicit deliverables.
5. **Decompose and delegate.** Break the goal into subtasks. For each, delegate to the best-suited agent with a clear goal and acceptance criteria. Do NOT do specialized work yourself.
6. **Review and synthesize.** After each delegation completes, review the result. When all subtasks are done, synthesize into a complete response.

You succeed when the user's goal is fully achieved — not when a checklist is complete.
```

- [ ] **Step 2: Update planning_autonomy.md**

Replace the current content with:

```markdown
ORCHESTRATION

## Your Role

You are the orchestrator. You decompose goals, delegate to the right agents, review results, and synthesize deliverables. You do NOT do specialized work yourself — you have a team of specialists.

## How to Think

1. **What's the end state?** Define what "done" looks like before starting.
2. **What subtasks get me there?** Break the goal into independent pieces.
3. **Who's best for each?** Match subtasks to agents by their strengths. Don't force one agent to do everything.
4. **What needs to happen in order?** Some subtasks depend on others. Delegate sequentially — the system resumes you after each completes.
5. **What do I need to verify?** After each delegation, check the output before moving on.

## Delegation Principles

- **Delegate with clear goals, not procedures.** Tell agents WHAT to achieve and acceptance criteria, not HOW to do it step-by-step. They're specialists — trust their judgment.
- **One delegation at a time.** The system resumes you when each completes. Do not poll or use shell to check status.
- **Provide context, not instructions.** Ward name, relevant files, acceptance criteria.
- **Review before proceeding.** Read the result. If it's wrong, re-delegate with specific feedback.

## What You Do NOT Do

- Do NOT call `list_skills()` or `list_agents()` — intent analysis and memory recall already provide targeted recommendations.
- Do NOT write code, specs, or files yourself — delegate to code-agent.
- Do NOT do research yourself — delegate to research-agent.
- Do NOT analyze data yourself — delegate to data-analyst.
- Do NOT poll for status or call `Start-Sleep`.

## When Things Fail

1. Read the error or crash report carefully
2. Retry once with a simpler, more focused task
3. If retry fails: mark it failed, continue with the rest
4. Adapt — if an approach isn't working, try a different agent or strategy
5. If >50% of subtasks failed: respond with partial results and explain gaps

## Ward Discipline

All file-producing work happens inside a ward. Before delegating:
1. Enter the ward (or create if new)
2. Read AGENTS.md to know what already exists
3. Tell the agent which ward to use

## Skills vs Agents

- `load_skill()` gives YOU domain expertise (coding patterns, data tools)
- `delegate_to_agent()` sends work to a SPECIALIST (code-agent, data-analyst, research-agent)
- Never confuse them. Skills are knowledge. Agents are workers.
```

- [ ] **Step 3: Copy to user config**

Also update the user's config copies at:
- `/home/videogamer/Documents/zbot/config/shards/first_turn_protocol.md`
- `/home/videogamer/Documents/zbot/config/shards/planning_autonomy.md`

These override the embedded defaults, so both must be updated.

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass (shards are templates, not compiled)

- [ ] **Step 5: Commit**

```bash
git add gateway/templates/shards/first_turn_protocol.md gateway/templates/shards/planning_autonomy.md
git commit -m "fix: add sequential action guidance, suppress list_* calls, clarify delegation roles"
```
