# Harness Enforcement Phase A — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce agent behavior through the runtime harness, not prompts — recover garbled JSON, limit root to single tool call per turn, remove tools root shouldn't have, activate context editing middleware.

**Architecture:** Four independent changes to the harness: (1) openai.rs recovers concatenated JSON, (2) executor.rs enforces single-action mode via config flag, (3) executor builder filters root's tool registry, (4) executor builder registers ContextEditingMiddleware in the pipeline.

**Tech Stack:** Rust (agent-runtime, gateway-execution crates), serde_json

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `runtime/agent-runtime/src/llm/openai.rs` | Modify | Recover first JSON object from garbled `}{` concatenation |
| `runtime/agent-runtime/src/executor.rs` | Modify | Add `single_action_mode` to ExecutorConfig, enforce in loop |
| `gateway/gateway-execution/src/invoke/executor.rs` | Modify | Set single_action_mode for root, filter tool registry, register middleware |

---

### Task 1: Recover Garbled JSON Concatenation

**Files:**
- Modify: `runtime/agent-runtime/src/llm/openai.rs:503-518`

- [ ] **Step 1: Write failing test**

Add to the test module at the bottom of openai.rs (or create one if none exists). Since the parse logic is inside `chat_stream` which is hard to unit test, test the recovery logic as a standalone function. Add near the top of the file:

```rust
/// Attempt to recover the first JSON object from a concatenated string like `{"a":"b"}{"c":"d"}`.
/// Returns Some(Value) if recovery succeeds, None otherwise.
fn recover_first_json(raw: &str) -> Option<serde_json::Value> {
    if let Some(pos) = raw.find("}{") {
        let first = &raw[..pos + 1];
        serde_json::from_str(first).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod json_recovery_tests {
    use super::*;

    #[test]
    fn test_recover_concatenated_json() {
        let raw = r#"{"action":"recall","query":"test"}{"title":"My Title"}{"action":"use"}"#;
        let result = recover_first_json(raw);
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["action"], "recall");
        assert_eq!(val["query"], "test");
    }

    #[test]
    fn test_recover_single_json_returns_none() {
        let raw = r#"{"action":"recall","query":"test"}"#;
        let result = recover_first_json(raw);
        assert!(result.is_none(), "Single JSON should not trigger recovery");
    }

    #[test]
    fn test_recover_invalid_json_returns_none() {
        let raw = r#"not json at all"#;
        let result = recover_first_json(raw);
        assert!(result.is_none());
    }

    #[test]
    fn test_recover_nested_braces() {
        let raw = r#"{"args":{"nested":"value"}}{"second":"obj"}"#;
        let result = recover_first_json(raw);
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["args"]["nested"], "value");
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p agent-runtime -- json_recovery_tests`
Expected: All 4 pass (we're writing the function and tests together since the function is standalone)

- [ ] **Step 3: Integrate recovery into the parse error path**

In `openai.rs`, find the parse error path (lines 506-516):

```rust
                        Err(e) => {
                            tracing::warn!(
                                "Failed to parse tool call arguments for '{}': {} — raw: {}",
                                acc.name, e, acc.arguments
                            );
                            json!({
                                "__error__": "PARSE_ERROR",
                                "__message__": format!("JSON parse error: {}", e),
                                "__truncated__": false
                            })
                        }
```

Replace with:

```rust
                        Err(e) => {
                            // Attempt to recover first JSON from concatenated objects
                            // Model sometimes outputs {"a":"b"}{"c":"d"} — extract first
                            if let Some(recovered) = recover_first_json(&acc.arguments) {
                                tracing::info!(
                                    "Recovered first JSON object from concatenated tool call '{}'. \
                                     Original had trailing data after first object.",
                                    acc.name
                                );
                                recovered
                            } else {
                                tracing::warn!(
                                    "Failed to parse tool call arguments for '{}': {} — raw: {}",
                                    acc.name, e, &acc.arguments[..acc.arguments.len().min(200)]
                                );
                                json!({
                                    "__error__": "PARSE_ERROR",
                                    "__message__": "Only one tool call per response. Send one tool call, wait for the result, then call the next.",
                                    "__truncated__": false
                                })
                            }
                        }
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add runtime/agent-runtime/src/llm/openai.rs
git commit -m "fix: recover first JSON object from garbled concatenated tool calls"
```

---

### Task 2: Single Action Mode on ExecutorConfig

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs:80-220` (ExecutorConfig + new())
- Modify: `runtime/agent-runtime/src/executor.rs:844-851` (tool call processing)
- Modify: `runtime/agent-runtime/src/lib.rs` (no new re-exports needed, field is on existing struct)

- [ ] **Step 1: Write failing test**

Add to `hook_tests` module:

```rust
    #[test]
    fn test_single_action_mode_default_false() {
        let config = ExecutorConfig::new("a".into(), "p".into(), "m".into());
        assert!(!config.single_action_mode, "single_action_mode should default to false");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-runtime -- hook_tests::test_single_action_mode`
Expected: FAIL — `single_action_mode` doesn't exist

- [ ] **Step 3: Add field to ExecutorConfig**

Add to `ExecutorConfig` struct (after `complexity` field, around line 178):

```rust
    /// When true, only the first tool call per LLM response is executed.
    /// Extra tool calls are dropped with a log message.
    /// Default: false. Set true for orchestrator agents (root).
    pub single_action_mode: bool,
```

Add to `ExecutorConfig::new()` defaults:

```rust
            single_action_mode: false,
```

Update the manual Debug impl to include `single_action_mode`.

- [ ] **Step 4: Add enforcement in executor loop**

In `executor.rs`, find where tool_calls are extracted (around line 844):

```rust
            let tool_calls = response.tool_calls.clone().unwrap_or_default();
```

Replace with:

```rust
            let mut tool_calls = response.tool_calls.clone().unwrap_or_default();

            // Single-action mode: execute only the first tool call, drop extras.
            // This prevents the model from batching multiple actions into one response.
            if self.config.single_action_mode && tool_calls.len() > 1 {
                tracing::info!(
                    "Single-action mode: executing '{}', dropping {} extra tool calls",
                    tool_calls[0].name, tool_calls.len() - 1
                );
                tool_calls.truncate(1);
            }
```

- [ ] **Step 5: Run all tests**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs
git commit -m "feat: single_action_mode on ExecutorConfig — enforce one tool call per turn"
```

---

### Task 3: Remove Tools Root Shouldn't Have

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/executor.rs:283-312` (build_tool_registry)

- [ ] **Step 1: Write failing test**

Add to `e2e_ward_pipeline_tests.rs`:

```rust
#[test]
fn test_root_tool_registry_excludes_specialist_tools() {
    // Root should NOT have load_skill, list_skills, or apply_patch
    // These are specialist tools that root should delegate, not use
    let root_blocked = ["load_skill", "list_skills"];
    for tool in &root_blocked {
        // This test documents the expectation — actual enforcement is in build_tool_registry
        assert!(
            !tool.is_empty(),
            "Root should not have '{}' tool", tool
        );
    }
}
```

Actually, we can't easily unit test the tool registry building without the full setup. Instead, make the change directly and verify via integration test.

- [ ] **Step 2: Modify build_tool_registry for root**

In `gateway/gateway-execution/src/invoke/executor.rs`, find the root tool registration (lines 297-308):

```rust
        } else {
            // Root agent gets the full tool set
            tool_registry.register_all(core_tools(fs_context.clone(), self.fact_store.clone()));
            tool_registry.register_all(optional_tools(fs_context, &self.tool_settings));
            tool_registry.register(Arc::new(RespondTool::new()));
            tool_registry.register(Arc::new(DelegateTool::new()));
            tool_registry.register(Arc::new(ListAgentsTool::new()));
```

Replace with:

```rust
        } else {
            // Root agent: orchestrator tools only.
            // Root delegates, it doesn't do specialist work.
            // Excluded: load_skill, list_skills (intent analysis provides),
            //           apply_patch (root doesn't write files)

            // Memory, ward, plan, session title, grep — orchestrator essentials
            tool_registry.register(Arc::new(ShellTool::new()));
            tool_registry.register(Arc::new(MemoryTool::new(fs_context.clone(), self.fact_store.clone())));
            tool_registry.register(Arc::new(WardTool::new(fs_context.clone(), self.fact_store.clone())));
            tool_registry.register(Arc::new(UpdatePlanTool::new()));
            tool_registry.register(Arc::new(SetSessionTitleTool::new()));
            tool_registry.register(Arc::new(GrepTool));

            // Orchestration tools
            tool_registry.register(Arc::new(RespondTool::new()));
            tool_registry.register(Arc::new(DelegateTool::new()));

            // Optional tools that root may need
            if self.tool_settings.file_tools {
                tool_registry.register(Arc::new(ReadTool));
                tool_registry.register(Arc::new(GlobTool));
            }

            // Connector query (if provider available)
            if let Some(provider) = &self.connector_provider {
                tool_registry.register(Arc::new(QueryResourceTool::new(provider.clone())));
            }
```

This removes from root: `ApplyPatchTool`, `LoadSkillTool`, `ListSkillsTool`, `ListAgentsTool`, `ExecutionGraphTool`, `WriteTool`, `EditTool`, `PythonTool`, `TodoTool`.

Root keeps: `ShellTool` (for reading results), `MemoryTool`, `WardTool`, `UpdatePlanTool`, `SetSessionTitleTool`, `GrepTool`, `RespondTool`, `DelegateTool`, `ReadTool`, `GlobTool`.

- [ ] **Step 3: Set single_action_mode for root**

In the same file, find where `executor_config` is built (around line 255-271). Add after `executor_config.mcps = agent.mcps.clone();`:

```rust
        // Root is an orchestrator — enforce single action per turn
        if !self.is_delegated {
            executor_config.single_action_mode = true;
        }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 5: Run agent-runtime tests too**

Run: `cargo test -p agent-runtime`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/invoke/executor.rs
git commit -m "fix: root gets orchestrator-only tools + single_action_mode — no load_skill, apply_patch, list_*"
```

---

### Task 4: Activate Context Editing Middleware

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/executor.rs:252-253`

- [ ] **Step 1: Add ContextEditingMiddleware to the pipeline**

Find the empty middleware pipeline creation (line 252-253):

```rust
        // Create empty middleware pipeline
        let middleware_pipeline = Arc::new(MiddlewarePipeline::new());
```

Replace with:

```rust
        // Create middleware pipeline with context editing
        let mut middleware_pipeline = MiddlewarePipeline::new();

        // Context editing: clears old tool results, compresses assistant messages,
        // skill-aware cascade unloading. Triggers at 70% of context window.
        let context_window = executor_config.context_window_tokens;
        if context_window > 0 {
            middleware_pipeline.add_pre_process(Box::new(
                agent_runtime::ContextEditingMiddleware::new(
                    agent_runtime::ContextEditingConfig {
                        enabled: true,
                        trigger_tokens: (context_window as usize * 70) / 100,
                        keep_tool_results: 8,
                        min_reclaim: 500,
                        clear_tool_inputs: true,
                        cascade_unload: true,
                        skill_aware_placeholders: true,
                        ..Default::default()
                    }
                )
            ));
        }

        let middleware_pipeline = Arc::new(middleware_pipeline);
```

Note: `executor_config.context_window_tokens` is set at line 261 AFTER this code. Move the middleware setup to AFTER the context window resolution:

Actually, let me check the ordering. The context_window is set at line 261. The middleware pipeline is created at line 253. We need to move the pipeline creation to after line 265 (after context_window is set).

Move the pipeline creation to just before `AgentExecutor::new()` at line 273:

```rust
        // Create middleware pipeline with context editing (after context_window is resolved)
        let middleware_pipeline = {
            let mut pipeline = MiddlewarePipeline::new();
            if executor_config.context_window_tokens > 0 {
                pipeline.add_pre_process(Box::new(
                    agent_runtime::ContextEditingMiddleware::new(
                        agent_runtime::ContextEditingConfig {
                            enabled: true,
                            trigger_tokens: (executor_config.context_window_tokens as usize * 70) / 100,
                            keep_tool_results: 8,
                            min_reclaim: 500,
                            clear_tool_inputs: true,
                            cascade_unload: true,
                            skill_aware_placeholders: true,
                            ..Default::default()
                        }
                    )
                ));
            }
            Arc::new(pipeline)
        };
```

- [ ] **Step 2: Add necessary imports**

Ensure the file imports `ContextEditingMiddleware` and `ContextEditingConfig`. Check if `agent_runtime::` prefix works or if explicit `use` is needed. The types are re-exported from `agent_runtime::middleware`.

- [ ] **Step 3: Verify the middleware `process` method gets called**

Check that the executor actually calls the middleware pipeline. Search for `middleware_pipeline.process` in executor.rs — if it's never called, the middleware won't run even if registered.

Read the middleware execution code in executor.rs to confirm.

- [ ] **Step 4: Run tests**

Run: `cargo test -p agent-runtime && cargo test -p gateway-execution`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/invoke/executor.rs
git commit -m "feat: activate ContextEditingMiddleware — compress old messages, clear tool results at 70% context"
```

---

### Task 5: Final Verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|test result" | grep -v "zero-core.*doc"`
Expected: No failures

- [ ] **Step 2: Run e2e tests**

Run: `cargo test -p gateway-execution --test e2e_ward_pipeline_tests`
Expected: 16 tests pass

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: Clean build
