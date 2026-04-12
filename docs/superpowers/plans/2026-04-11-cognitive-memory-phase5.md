# Cognitive Memory Phase 5 — Intelligent Micro-Recall Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add targeted, lightweight memory queries at key decision points — before delegation, on errors, on ward entry, on entity mentions — injected into working memory for the next iteration.

**Architecture:** Extend the existing `WorkingMemoryMiddleware` (Phase 2) with `MicroRecallTrigger` enum and async `micro_recall()` method on `WorkingMemory`. Each trigger performs a fast, targeted query (< 100ms) and adds results to working memory, deduplicated.

**Tech Stack:** Rust (gateway-execution), existing MemoryRepository/WardWikiRepository/ProcedureRepository/GraphStorage.

**Spec:** `docs/superpowers/specs/2026-04-11-cognitive-memory-system-design.md` — Section 9

**Branch:** `feature/sentient` (continuing from Phase 4)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| CREATE | `gateway/gateway-execution/src/invoke/micro_recall.rs` | MicroRecallTrigger enum, micro_recall() logic, trigger detection |
| MODIFY | `gateway/gateway-execution/src/invoke/mod.rs` | Export micro_recall module |
| MODIFY | `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` | Call micro_recall triggers after tool result processing |
| MODIFY | `gateway/gateway-execution/src/runner.rs` | Pass repos to middleware for micro-recall |

---

### Task 1: Micro-Recall Module + Tests

**Files:**
- Create: `gateway/gateway-execution/src/invoke/micro_recall.rs`
- Modify: `gateway/gateway-execution/src/invoke/mod.rs`

Create the micro-recall module with trigger detection, query execution, and working memory integration.

- [ ] **Step 1: Create micro_recall.rs**

```rust
//! Micro-Recall — targeted, lightweight memory queries at decision points.
//!
//! Each trigger performs a fast query (<100ms target) and injects results
//! into working memory for the next LLM iteration.

use super::working_memory::WorkingMemory;
use gateway_database::{MemoryRepository, ProcedureRepository, WardWikiRepository};
use knowledge_graph::GraphStorage;
use std::sync::Arc;

/// Decision points that trigger a targeted memory lookup.
#[derive(Debug)]
pub enum MicroRecallTrigger {
    /// Before delegate_to_agent — recall what works for this agent/task
    PreDelegation { agent_id: String, task: String },
    /// Tool returned an error — recall past corrections
    ToolError { tool_name: String, error_message: String },
    /// Ward tool used to enter a ward — recall ward knowledge
    WardEntry { ward_id: String },
    /// Entity mentioned in tool output but not yet in working memory
    EntityMention { entity_name: String },
}

/// Services needed for micro-recall queries.
pub struct MicroRecallContext {
    pub memory_repo: Option<Arc<MemoryRepository>>,
    pub procedure_repo: Option<Arc<ProcedureRepository>>,
    pub wiki_repo: Option<Arc<WardWikiRepository>>,
    pub graph_storage: Option<Arc<GraphStorage>>,
    pub agent_id: String,
}

/// Execute a micro-recall trigger and update working memory.
///
/// Each trigger is designed to be fast (<100ms) and targeted.
/// Results are deduplicated against existing working memory content.
pub async fn execute_micro_recall(
    wm: &mut WorkingMemory,
    trigger: MicroRecallTrigger,
    ctx: &MicroRecallContext,
    iteration: u32,
) {
    match trigger {
        MicroRecallTrigger::PreDelegation { agent_id, task } => {
            handle_pre_delegation(wm, &agent_id, &task, ctx, iteration).await;
        }
        MicroRecallTrigger::ToolError {
            tool_name,
            error_message,
        } => {
            handle_tool_error(wm, &tool_name, &error_message, ctx, iteration).await;
        }
        MicroRecallTrigger::WardEntry { ward_id } => {
            handle_ward_entry(wm, &ward_id, ctx, iteration).await;
        }
        MicroRecallTrigger::EntityMention { entity_name } => {
            handle_entity_mention(wm, &entity_name, ctx, iteration).await;
        }
    }
}

/// Detect micro-recall triggers from a tool result.
///
/// Returns triggers that should fire based on the tool name, result, and error.
pub fn detect_triggers(
    tool_name: &str,
    result: &str,
    error: Option<&str>,
    wm: &WorkingMemory,
) -> Vec<MicroRecallTrigger> {
    let mut triggers = Vec::new();

    // Tool error → recall corrections
    if let Some(err) = error {
        triggers.push(MicroRecallTrigger::ToolError {
            tool_name: tool_name.to_string(),
            error_message: truncate(err, 200),
        });
    }

    // delegate_to_agent → pre-delegation recall
    if tool_name == "delegate_to_agent" && error.is_none() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(result) {
            let agent = val
                .get("agent_id")
                .or(val.get("child_agent_id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let task = val
                .get("task")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if !agent.is_empty() {
                triggers.push(MicroRecallTrigger::PreDelegation {
                    agent_id: agent.to_string(),
                    task: task.to_string(),
                });
            }
        }
    }

    // ward tool → ward entry
    if tool_name == "ward" && error.is_none() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(result) {
            if let Some(ward_id) = val.get("ward_id").and_then(|v| v.as_str()) {
                triggers.push(MicroRecallTrigger::WardEntry {
                    ward_id: ward_id.to_string(),
                });
            }
        }
    }

    // Entity mentions — check for new entities in result text (not already in WM)
    if error.is_none() && !result.is_empty() && tool_name != "respond" {
        let scan = if result.len() > 1000 {
            &result[..1000]
        } else {
            result
        };
        for entity in extract_new_entities(scan, wm) {
            triggers.push(MicroRecallTrigger::EntityMention {
                entity_name: entity,
            });
        }
    }

    triggers
}

// ============================================================================
// Trigger handlers
// ============================================================================

async fn handle_pre_delegation(
    wm: &mut WorkingMemory,
    agent_id: &str,
    task: &str,
    ctx: &MicroRecallContext,
    iteration: u32,
) {
    // Recall procedures for this type of delegation
    if let Some(ref proc_repo) = ctx.procedure_repo {
        if let Ok(procedures) = proc_repo.list_procedures(agent_id, None) {
            for proc in procedures.iter().take(2) {
                if proc.success_count >= 2 {
                    wm.add_discovery(
                        &format!(
                            "Proven procedure for {}: {} ({}% success)",
                            agent_id,
                            proc.name,
                            (proc.success_count * 100)
                                / (proc.success_count + proc.failure_count).max(1),
                        ),
                        iteration,
                        "micro-recall:delegation",
                    );
                }
            }
        }
    }

    // Recall corrections for this agent
    if let Some(ref mem_repo) = ctx.memory_repo {
        if let Ok(facts) = mem_repo.get_corrections_for_agent(agent_id, 3) {
            for fact in &facts {
                wm.add_correction(&fact.content);
            }
        }
    }

    let _ = task; // task used for context but not queried separately
}

async fn handle_tool_error(
    wm: &mut WorkingMemory,
    tool_name: &str,
    error_message: &str,
    ctx: &MicroRecallContext,
    iteration: u32,
) {
    // Search memory for corrections related to this error
    if let Some(ref mem_repo) = ctx.memory_repo {
        let query = format!("{} {}", tool_name, &error_message[..error_message.len().min(100)]);
        if let Ok(facts) = mem_repo.search_corrections(&query, &ctx.agent_id, 3) {
            for fact in &facts {
                wm.add_correction(&format!("[from memory] {}", fact.content));
            }
        }
    }

    // Record the error as a discovery if not already present
    wm.add_discovery(
        &format!("{} error: {}", tool_name, truncate(error_message, 150)),
        iteration,
        "micro-recall:error",
    );
}

async fn handle_ward_entry(
    wm: &mut WorkingMemory,
    ward_id: &str,
    ctx: &MicroRecallContext,
    iteration: u32,
) {
    // Recall ward wiki index
    if let Some(ref wiki_repo) = ctx.wiki_repo {
        if let Ok(Some(index)) = wiki_repo.get_article(ward_id, "__index__") {
            // Add ward knowledge summary as a discovery
            let summary = truncate(&index.content, 300);
            wm.add_discovery(
                &format!("Ward {} knowledge: {}", ward_id, summary),
                iteration,
                "micro-recall:ward",
            );
        }
    }

    // Add ward as an entity
    wm.add_entity(ward_id, Some("ward"), "Active workspace", iteration);
}

async fn handle_entity_mention(
    wm: &mut WorkingMemory,
    entity_name: &str,
    ctx: &MicroRecallContext,
    iteration: u32,
) {
    // Look up entity in knowledge graph
    if let Some(ref graph) = ctx.graph_storage {
        if let Ok(Some(entity)) = graph.get_entity_by_name(entity_name).await {
            // Get 1-hop neighbors
            if let Ok(neighbors) = graph
                .get_neighbors(&entity.id, knowledge_graph::Direction::Both, 5)
                .await
            {
                let mut summary = format!("{} ({})", entity.name, entity.entity_type);
                for n in neighbors.iter().take(3) {
                    summary.push_str(&format!(
                        ", {} {}",
                        n.relationship_type, n.entity.name
                    ));
                }
                wm.add_entity(entity_name, Some(&entity.entity_type), &summary, iteration);
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract entity names from text that aren't already in working memory.
fn extract_new_entities(text: &str, wm: &WorkingMemory) -> Vec<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?:"([^"]{2,30})"|([A-Z][a-z]+(?:[A-Z][a-z]+)+)|(\b[A-Z]{3,}\b))"#)
            .unwrap_or_else(|_| Regex::new(".^").unwrap())
    });

    let mut new_entities = Vec::new();
    for cap in ENTITY_RE.captures_iter(text) {
        let name = cap
            .get(1)
            .or(cap.get(2))
            .or(cap.get(3))
            .map(|m| m.as_str().to_string());

        if let Some(name) = name {
            if name.len() >= 3
                && !is_common_word(&name)
                && !wm.has_entity(&name)
                && !new_entities.contains(&name)
            {
                new_entities.push(name);
                if new_entities.len() >= 3 {
                    break; // Cap at 3 entities per tool result
                }
            }
        }
    }
    new_entities
}

fn is_common_word(word: &str) -> bool {
    matches!(
        word.to_uppercase().as_str(),
        "THE" | "AND" | "FOR" | "NOT" | "THIS" | "THAT" | "WITH"
            | "FROM" | "HAVE" | "WILL" | "ARE" | "BUT" | "ALL"
            | "JSON" | "HTTP" | "URL" | "API" | "CSS" | "HTML"
            | "NONE" | "NULL" | "TRUE" | "FALSE" | "SELF"
            | "TODO" | "NOTE" | "INFO" | "WARN" | "DEBUG"
    )
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .take_while(|(i, _)| *i < max_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_len);
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ctx() -> MicroRecallContext {
        MicroRecallContext {
            memory_repo: None,
            procedure_repo: None,
            wiki_repo: None,
            graph_storage: None,
            agent_id: "root".to_string(),
        }
    }

    #[test]
    fn test_detect_tool_error_trigger() {
        let wm = WorkingMemory::new(5000);
        let triggers = detect_triggers("shell", "", Some("Connection refused"), &wm);
        assert!(triggers.iter().any(|t| matches!(t, MicroRecallTrigger::ToolError { .. })));
    }

    #[test]
    fn test_detect_delegation_trigger() {
        let wm = WorkingMemory::new(5000);
        let result = r#"{"agent_id": "research-agent", "task": "find data"}"#;
        let triggers = detect_triggers("delegate_to_agent", result, None, &wm);
        assert!(triggers.iter().any(|t| matches!(t, MicroRecallTrigger::PreDelegation { .. })));
    }

    #[test]
    fn test_detect_ward_entry_trigger() {
        let wm = WorkingMemory::new(5000);
        let result = r#"{"ward_id": "stock-analysis", "status": "entered"}"#;
        let triggers = detect_triggers("ward", result, None, &wm);
        assert!(triggers.iter().any(|t| matches!(t, MicroRecallTrigger::WardEntry { .. })));
    }

    #[test]
    fn test_detect_entity_mention() {
        let wm = WorkingMemory::new(5000);
        let result = "Using MultiIndex from DataFrame to analyze AAPL";
        let triggers = detect_triggers("shell", result, None, &wm);
        assert!(triggers.iter().any(|t| matches!(t, MicroRecallTrigger::EntityMention { .. })));
    }

    #[test]
    fn test_no_entity_trigger_if_already_in_wm() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_entity("MultiIndex", None, "known", 1);
        let result = "Using MultiIndex";
        let triggers = detect_triggers("shell", result, None, &wm);
        // MultiIndex already known — should not trigger
        assert!(!triggers.iter().any(|t| {
            matches!(t, MicroRecallTrigger::EntityMention { entity_name } if entity_name == "MultiIndex")
        }));
    }

    #[test]
    fn test_respond_tool_no_entity_triggers() {
        let wm = WorkingMemory::new(5000);
        let triggers = detect_triggers("respond", "Using MultiIndex", None, &wm);
        assert!(triggers.iter().all(|t| !matches!(t, MicroRecallTrigger::EntityMention { .. })));
    }

    #[tokio::test]
    async fn test_execute_tool_error_adds_discovery() {
        let mut wm = WorkingMemory::new(5000);
        let ctx = empty_ctx();
        execute_micro_recall(
            &mut wm,
            MicroRecallTrigger::ToolError {
                tool_name: "shell".into(),
                error_message: "Connection refused".into(),
            },
            &ctx,
            5,
        )
        .await;
        let output = wm.format_for_prompt();
        assert!(output.contains("shell error: Connection refused"));
    }

    #[tokio::test]
    async fn test_execute_ward_entry_adds_entity() {
        let mut wm = WorkingMemory::new(5000);
        let ctx = empty_ctx();
        execute_micro_recall(
            &mut wm,
            MicroRecallTrigger::WardEntry {
                ward_id: "stock-analysis".into(),
            },
            &ctx,
            3,
        )
        .await;
        let output = wm.format_for_prompt();
        assert!(output.contains("stock-analysis"));
        assert!(output.contains("ward"));
    }

    #[test]
    fn test_extract_new_entities_caps_at_3() {
        let wm = WorkingMemory::new(5000);
        let text = "Using MultiIndex DataFrame PascalCase AnotherThing MoreStuff";
        let entities = extract_new_entities(text, &wm);
        assert!(entities.len() <= 3);
    }

    #[test]
    fn test_truncate_multibyte_safe() {
        let s = "Hello 🌍 world";
        let t = truncate(s, 7);
        assert!(t.len() <= 15); // safe, no panic
    }
}
```

- [ ] **Step 2: Add has_entity() to WorkingMemory**

In `gateway/gateway-execution/src/invoke/working_memory.rs`, add a public method:

```rust
/// Check if an entity is already tracked (case-insensitive).
pub fn has_entity(&self, name: &str) -> bool {
    self.entities.contains_key(&name.to_lowercase())
}
```

- [ ] **Step 3: Add helper methods to MemoryRepository**

In `gateway/gateway-database/src/memory_repository.rs`, add:

```rust
/// Get correction facts for a specific agent (category = 'correction').
pub fn get_corrections_for_agent(&self, agent_id: &str, limit: usize) -> Result<Vec<MemoryFact>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT ... FROM memory_facts \
             WHERE agent_id = ?1 AND category = 'correction' AND valid_until IS NULL \
             ORDER BY confidence DESC LIMIT ?2"
        )?;
        // Map rows to MemoryFact, collect
    })
}

/// Search corrections by text similarity (FTS or LIKE).
pub fn search_corrections(&self, query: &str, agent_id: &str, limit: usize) -> Result<Vec<MemoryFact>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT ... FROM memory_facts \
             WHERE agent_id = ?1 AND category = 'correction' AND valid_until IS NULL \
             AND content LIKE '%' || ?2 || '%' \
             LIMIT ?3"
        )?;
        // Map rows, collect
    })
}
```

Note: Adapt the exact SELECT columns and row mapping to match existing methods in the file.

- [ ] **Step 4: Export micro_recall module**

In `gateway/gateway-execution/src/invoke/mod.rs`, add:

```rust
pub mod micro_recall;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --package gateway-execution -- micro_recall`
Expected: 10 tests pass.

- [ ] **Step 6: Quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-execution/src/invoke/micro_recall.rs gateway/gateway-execution/src/invoke/mod.rs gateway/gateway-execution/src/invoke/working_memory.rs gateway/gateway-database/src/memory_repository.rs
git commit -m "feat(micro-recall): trigger detection and execution for delegation, errors, wards, entities"
```

---

### Task 2: Integrate Micro-Recall into Execution Loop

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/working_memory_middleware.rs`
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Update working_memory_middleware to call micro-recall**

In `working_memory_middleware.rs`, update `process_tool_result` to also detect and execute micro-recall triggers:

```rust
use super::micro_recall::{self, MicroRecallContext, MicroRecallTrigger};

/// Process a tool result with micro-recall support.
pub async fn process_tool_result_with_recall(
    wm: &mut WorkingMemory,
    tool_name: &str,
    result: &str,
    error: Option<&str>,
    iteration: u32,
    recall_ctx: &MicroRecallContext,
) {
    // Existing processing (entity extraction, error recording)
    process_tool_result(wm, tool_name, result, error, iteration);

    // Detect and execute micro-recall triggers
    let triggers = micro_recall::detect_triggers(tool_name, result, error, wm);
    for trigger in triggers {
        micro_recall::execute_micro_recall(wm, trigger, recall_ctx, iteration).await;
    }
}
```

- [ ] **Step 2: Update runner to use micro-recall-aware processing**

In `runner.rs`, in the `spawn_execution_task` function, find where `working_memory_middleware::process_tool_result` is called. Build a `MicroRecallContext` and call the new `process_tool_result_with_recall` instead.

The `MicroRecallContext` needs repos that are available in the spawn scope. Check what's already cloned into the spawn — `memory_repo`, `graph_storage` etc. Build the context from those.

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace`

- [ ] **Step 4: Quality checks**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/invoke/working_memory_middleware.rs gateway/gateway-execution/src/runner.rs
git commit -m "feat(micro-recall): integrate into execution loop via working memory middleware"
```

---

### Task 3: Final Checks + Push

- [ ] **Step 1: Format, lint, test**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace --lib --bins --tests`

- [ ] **Step 2: Push**

```bash
git push
```
