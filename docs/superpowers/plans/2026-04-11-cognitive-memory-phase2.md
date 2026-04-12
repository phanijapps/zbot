# Cognitive Memory Phase 2 — Working Memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a live, mutable working memory layer that updates each iteration — agents maintain evolving context instead of static session-start recall.

**Architecture:** A `WorkingMemory` struct tracks active entities, session discoveries, corrections, and delegation status. A middleware callback processes tool results after each iteration, extracting entities and learnings. The working memory is formatted as a `## Working Memory` system message injected before each LLM call.

**Tech Stack:** Rust (gateway-execution), IndexMap for ordered entity tracking, regex for lightweight entity extraction.

**Spec:** `docs/superpowers/specs/2026-04-11-cognitive-memory-system-design.md` — Section 6

**Branch:** `feature/sentient` (continuing from Phase 1)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| CREATE | `gateway/gateway-execution/src/invoke/working_memory.rs` | WorkingMemory struct, entity/discovery/correction tracking, budget eviction, prompt formatting |
| CREATE | `gateway/gateway-execution/src/invoke/working_memory_middleware.rs` | Post-tool-result processing: entity extraction, error pattern matching, delegation summaries |
| MODIFY | `gateway/gateway-execution/src/invoke/mod.rs` | Export new modules |
| MODIFY | `gateway/gateway-execution/src/runner.rs` | Initialize WorkingMemory in spawn_execution_task, inject before each LLM call, call middleware after tool results |
| MODIFY | `gateway/gateway-execution/Cargo.toml` | Add `indexmap` dependency (if not already present) |

---

### Task 1: WorkingMemory Data Structure + Tests

**Files:**
- Create: `gateway/gateway-execution/src/invoke/working_memory.rs`
- Modify: `gateway/gateway-execution/src/invoke/mod.rs`
- Modify: `gateway/gateway-execution/Cargo.toml` (if indexmap not present)

- [ ] **Step 1: Check if indexmap is already a dependency**

Run: `grep indexmap gateway/gateway-execution/Cargo.toml`

If not found, add `indexmap = { workspace = true }` under `[dependencies]`. If `indexmap` is not in workspace `Cargo.toml`, add it there first with a recent version (e.g., `indexmap = "2"`).

- [ ] **Step 2: Create working_memory.rs with struct and core methods**

Create `gateway/gateway-execution/src/invoke/working_memory.rs`:

```rust
//! Working Memory — live, mutable context that evolves during execution.
//!
//! Tracks active entities, session discoveries, corrections, and delegation
//! status. Injected as a system message before each LLM iteration.

use indexmap::IndexMap;

/// An entity actively tracked in working memory.
#[derive(Debug, Clone)]
pub struct WorkingEntity {
    pub name: String,
    pub entity_type: Option<String>,
    pub summary: String,
    pub last_referenced_iteration: u32,
}

/// A discovery made during the session.
#[derive(Debug, Clone)]
pub struct Discovery {
    pub content: String,
    pub iteration: u32,
    pub source: String,
}

/// Summary of a delegation's status and findings.
#[derive(Debug, Clone)]
pub struct DelegationSummary {
    pub agent_id: String,
    pub task_summary: String,
    pub key_findings: Vec<String>,
    pub status: String,
}

/// Live working memory that updates each iteration.
///
/// Budget-managed: when total tokens exceed `token_budget`,
/// least-recently-referenced entities are evicted first.
pub struct WorkingMemory {
    entities: IndexMap<String, WorkingEntity>,
    discoveries: Vec<Discovery>,
    corrections: Vec<String>,
    delegations: Vec<DelegationSummary>,
    token_budget: usize,
}

impl WorkingMemory {
    /// Create a new working memory with the given token budget.
    pub fn new(token_budget: usize) -> Self {
        Self {
            entities: IndexMap::new(),
            discoveries: Vec::new(),
            corrections: Vec::new(),
            delegations: Vec::new(),
            token_budget,
        }
    }

    /// Add or update an entity in working memory.
    pub fn add_entity(&mut self, name: &str, entity_type: Option<&str>, summary: &str, iteration: u32) {
        let key = name.to_lowercase();
        if let Some(existing) = self.entities.get_mut(&key) {
            existing.summary = summary.to_string();
            existing.last_referenced_iteration = iteration;
            if entity_type.is_some() {
                existing.entity_type = entity_type.map(|s| s.to_string());
            }
        } else {
            self.entities.insert(
                key,
                WorkingEntity {
                    name: name.to_string(),
                    entity_type: entity_type.map(|s| s.to_string()),
                    summary: summary.to_string(),
                    last_referenced_iteration: iteration,
                },
            );
        }
        self.evict_if_over_budget();
    }

    /// Record a session discovery.
    pub fn add_discovery(&mut self, content: &str, iteration: u32, source: &str) {
        // Avoid duplicate discoveries
        if self.discoveries.iter().any(|d| d.content == content) {
            return;
        }
        self.discoveries.push(Discovery {
            content: content.to_string(),
            iteration,
            source: source.to_string(),
        });
        self.evict_if_over_budget();
    }

    /// Record an active correction.
    pub fn add_correction(&mut self, correction: &str) {
        if !self.corrections.contains(&correction.to_string()) {
            self.corrections.push(correction.to_string());
        }
    }

    /// Update delegation status and findings.
    pub fn update_delegation(&mut self, agent_id: &str, status: &str, findings: Vec<String>) {
        if let Some(d) = self.delegations.iter_mut().find(|d| d.agent_id == agent_id) {
            d.status = status.to_string();
            if !findings.is_empty() {
                d.key_findings = findings;
            }
        } else {
            self.delegations.push(DelegationSummary {
                agent_id: agent_id.to_string(),
                task_summary: String::new(),
                key_findings: findings,
                status: status.to_string(),
            });
        }
    }

    /// Set the task summary for a delegation (called when delegation starts).
    pub fn set_delegation_task(&mut self, agent_id: &str, task: &str) {
        if let Some(d) = self.delegations.iter_mut().find(|d| d.agent_id == agent_id) {
            d.task_summary = task.to_string();
        } else {
            self.delegations.push(DelegationSummary {
                agent_id: agent_id.to_string(),
                task_summary: task.to_string(),
                key_findings: Vec::new(),
                status: "running".to_string(),
            });
        }
    }

    /// Estimated token count (chars / 4).
    pub fn token_count(&self) -> usize {
        self.format_for_prompt().len() / 4
    }

    /// Evict least-recently-referenced entities until under budget.
    pub fn evict_if_over_budget(&mut self) {
        while self.token_count() > self.token_budget && !self.entities.is_empty() {
            // Find entity with lowest last_referenced_iteration
            let lru_key = self
                .entities
                .iter()
                .min_by_key(|(_, e)| e.last_referenced_iteration)
                .map(|(k, _)| k.clone());

            if let Some(key) = lru_key {
                self.entities.shift_remove(&key);
            } else {
                break;
            }
        }

        // Also evict old discoveries if still over budget
        while self.token_count() > self.token_budget && !self.discoveries.is_empty() {
            self.discoveries.remove(0); // Remove oldest first
        }
    }

    /// Format working memory as markdown for system prompt injection.
    pub fn format_for_prompt(&self) -> String {
        let mut output = String::from("## Working Memory (auto-updated)\n");

        if !self.entities.is_empty() {
            output.push_str("\n### Active Entities\n");
            for entity in self.entities.values() {
                let type_label = entity
                    .entity_type
                    .as_deref()
                    .map(|t| format!(" ({})", t))
                    .unwrap_or_default();
                output.push_str(&format!("- **{}**{}: {}\n", entity.name, type_label, entity.summary));
            }
        }

        if !self.discoveries.is_empty() {
            output.push_str("\n### Session Discoveries\n");
            for d in &self.discoveries {
                output.push_str(&format!("- {} [iter {}, {}]\n", d.content, d.iteration, d.source));
            }
        }

        if !self.corrections.is_empty() {
            output.push_str("\n### Active Corrections\n");
            for c in &self.corrections {
                output.push_str(&format!("- {}\n", c));
            }
        }

        if !self.delegations.is_empty() {
            output.push_str("\n### Delegation Status\n");
            for d in &self.delegations {
                let task = if d.task_summary.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", truncate_str(&d.task_summary, 80))
                };
                if d.key_findings.is_empty() {
                    output.push_str(&format!("- {}: {}{}\n", d.agent_id, d.status, task));
                } else {
                    let findings = d.key_findings.join("; ");
                    output.push_str(&format!(
                        "- {}: {}{} — {}\n",
                        d.agent_id,
                        d.status,
                        task,
                        truncate_str(&findings, 120)
                    ));
                }
            }
        }

        output
    }

    /// Whether working memory has any content worth injecting.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
            && self.discoveries.is_empty()
            && self.corrections.is_empty()
            && self.delegations.is_empty()
    }
}

/// Truncate a string to max_len, appending "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_entity_and_format() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_entity("yfinance", Some("module"), "Python library for stock data", 1);
        let output = wm.format_for_prompt();
        assert!(output.contains("**yfinance** (module): Python library for stock data"));
    }

    #[test]
    fn test_add_entity_updates_existing() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_entity("SPY", None, "S&P 500 ETF", 1);
        wm.add_entity("SPY", None, "S&P 500 ETF. Price: $523", 3);
        let output = wm.format_for_prompt();
        assert!(output.contains("Price: $523"));
        // Should only appear once
        assert_eq!(output.matches("SPY").count(), 1);
    }

    #[test]
    fn test_add_discovery_deduplicates() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_discovery("API is paginated", 5, "shell");
        wm.add_discovery("API is paginated", 6, "shell");
        assert_eq!(wm.discoveries.len(), 1);
    }

    #[test]
    fn test_add_correction() {
        let mut wm = WorkingMemory::new(5000);
        wm.add_correction("Use plotly not matplotlib");
        wm.add_correction("Use plotly not matplotlib"); // dup
        let output = wm.format_for_prompt();
        assert!(output.contains("Use plotly not matplotlib"));
        assert_eq!(output.matches("plotly").count(), 1);
    }

    #[test]
    fn test_delegation_lifecycle() {
        let mut wm = WorkingMemory::new(5000);
        wm.set_delegation_task("research-agent", "fetch stock data");
        wm.update_delegation("research-agent", "completed", vec!["found 8 sources".into()]);
        let output = wm.format_for_prompt();
        assert!(output.contains("research-agent: completed"));
        assert!(output.contains("found 8 sources"));
    }

    #[test]
    fn test_eviction_removes_lru_entity() {
        // Very small budget to force eviction
        let mut wm = WorkingMemory::new(50);
        wm.add_entity("old_entity", None, "should be evicted because LRU", 1);
        wm.add_entity("new_entity", None, "should survive because recent", 10);
        // After eviction, old_entity should be gone
        let output = wm.format_for_prompt();
        assert!(!output.contains("old_entity"));
    }

    #[test]
    fn test_is_empty() {
        let wm = WorkingMemory::new(5000);
        assert!(wm.is_empty());
    }

    #[test]
    fn test_format_empty() {
        let wm = WorkingMemory::new(5000);
        let output = wm.format_for_prompt();
        assert!(output.contains("Working Memory"));
        // No sections rendered
        assert!(!output.contains("Active Entities"));
    }
}
```

- [ ] **Step 3: Export working_memory module**

In `gateway/gateway-execution/src/invoke/mod.rs`, add:

```rust
pub mod working_memory;
```

And add to exports:

```rust
pub use working_memory::WorkingMemory;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-execution -- working_memory`
Expected: 8 tests pass.

- [ ] **Step 5: Run quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add gateway/gateway-execution/src/invoke/working_memory.rs gateway/gateway-execution/src/invoke/mod.rs gateway/gateway-execution/Cargo.toml
git commit -m "feat(working-memory): WorkingMemory struct with entity tracking, budget eviction, and prompt formatting"
```

---

### Task 2: Working Memory Middleware — Entity Extraction + Tool Processing

**Files:**
- Create: `gateway/gateway-execution/src/invoke/working_memory_middleware.rs`
- Modify: `gateway/gateway-execution/src/invoke/mod.rs`

- [ ] **Step 1: Create the middleware module**

Create `gateway/gateway-execution/src/invoke/working_memory_middleware.rs`:

```rust
//! Working Memory Middleware — processes tool results to update working memory.
//!
//! Extracts entities from tool output, records discoveries from errors,
//! and tracks delegation status changes.

use super::working_memory::WorkingMemory;
use regex::Regex;
use std::sync::LazyLock;

/// Regex for extracting entity candidates from text.
/// Matches: "quoted strings", PascalCase words, ALLCAPS (3+ chars).
static ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:"([^"]{2,30})"|([A-Z][a-z]+(?:[A-Z][a-z]+)+)|(\b[A-Z]{3,}\b))"#)
        .unwrap_or_else(|_| Regex::new(".^").unwrap()) // safe fallback: never matches
});

/// Process a tool result and update working memory.
pub fn process_tool_result(
    wm: &mut WorkingMemory,
    tool_name: &str,
    result: &str,
    error: Option<&str>,
    iteration: u32,
) {
    // Record errors as discoveries
    if let Some(err) = error {
        let msg = truncate(err, 200);
        wm.add_discovery(
            &format!("{tool_name} error: {msg}"),
            iteration,
            tool_name,
        );
        return;
    }

    // Tool-specific processing
    match tool_name {
        "delegate_to_agent" => handle_delegation_result(wm, result),
        "respond" => {} // Final response — nothing to extract
        "set_session_title" => {} // Metadata — skip
        _ => {
            // Extract entities from tool output (for shell, read, grep, etc.)
            extract_and_add_entities(wm, result, iteration, tool_name);
        }
    }
}

/// Process a delegation start event.
pub fn process_delegation_started(
    wm: &mut WorkingMemory,
    agent_id: &str,
    task: &str,
) {
    wm.set_delegation_task(agent_id, task);
}

/// Process a delegation completion event.
pub fn process_delegation_completed(
    wm: &mut WorkingMemory,
    agent_id: &str,
    result: &str,
) {
    let findings = extract_key_lines(result, 3);
    wm.update_delegation(agent_id, "completed", findings);
}

/// Extract key lines from a delegation result (first N non-empty lines).
fn extract_key_lines(text: &str, max_lines: usize) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && l.len() > 10)
        .take(max_lines)
        .map(|l| truncate(l, 100).to_string())
        .collect()
}

/// Extract entity candidates from text and add to working memory.
fn extract_and_add_entities(
    wm: &mut WorkingMemory,
    text: &str,
    iteration: u32,
    source: &str,
) {
    // Only scan first 2000 chars to keep it fast
    let scan_text = if text.len() > 2000 { &text[..2000] } else { text };

    for cap in ENTITY_RE.captures_iter(scan_text) {
        let name = cap
            .get(1)
            .or(cap.get(2))
            .or(cap.get(3))
            .map(|m| m.as_str().to_string());

        if let Some(name) = name {
            // Skip very short or very common words
            if name.len() < 3 || is_common_word(&name) {
                continue;
            }
            // Extract a brief context snippet around the match
            let snippet = extract_context_snippet(scan_text, &name);
            wm.add_entity(&name, None, &snippet, iteration);
        }
    }

    // Check for error-like patterns as discoveries
    if text.contains("Error:") || text.contains("error:") || text.contains("FAILED") {
        if let Some(error_line) = text.lines().find(|l| {
            l.contains("Error:") || l.contains("error:") || l.contains("FAILED")
        }) {
            wm.add_discovery(
                &truncate(error_line.trim(), 150),
                iteration,
                source,
            );
        }
    }
}

/// Handle delegation tool result — parse agent_id from result JSON.
fn handle_delegation_result(wm: &mut WorkingMemory, result: &str) {
    // delegate_to_agent returns JSON with delegation info
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(result) {
        let agent_id = value
            .get("agent_id")
            .or(value.get("child_agent_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let task = value
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        wm.set_delegation_task(agent_id, task);
    }
}

/// Extract a brief context snippet around a name in text.
fn extract_context_snippet(text: &str, name: &str) -> String {
    if let Some(pos) = text.find(name) {
        let start = pos.saturating_sub(20);
        let end = (pos + name.len() + 40).min(text.len());
        let snippet = &text[start..end];
        // Clean up: take the line containing the match
        snippet
            .lines()
            .find(|l| l.contains(name))
            .map(|l| truncate(l.trim(), 80).to_string())
            .unwrap_or_else(|| truncate(snippet.trim(), 80).to_string())
    } else {
        format!("mentioned in {}", truncate(text, 40))
    }
}

/// Check if a word is too common to be a useful entity.
fn is_common_word(word: &str) -> bool {
    matches!(
        word.to_uppercase().as_str(),
        "THE" | "AND" | "FOR" | "NOT" | "THIS" | "THAT" | "WITH"
            | "FROM" | "HAVE" | "WILL" | "ARE" | "BUT" | "ALL"
            | "CAN" | "HAS" | "HER" | "WAS" | "ONE" | "OUR"
            | "OUT" | "YOU" | "HAD" | "HOT" | "HIS" | "GET"
            | "LET" | "SAY" | "SHE" | "TOO" | "USE" | "WAY"
            | "WHO" | "DID" | "ITS" | "SET" | "TRY" | "ASK"
            | "MEN" | "RUN" | "GOT" | "OLD" | "END" | "NOW"
            | "PUT" | "BOX" | "ROW" | "COL" | "KEY" | "MAP"
            | "JSON" | "HTTP" | "URL" | "API" | "CSS" | "HTML"
            | "NONE" | "NULL" | "TRUE" | "FALSE" | "SELF"
            | "TODO" | "NOTE" | "INFO" | "WARN" | "DEBUG"
    )
}

/// Truncate a string to max_len.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_extraction_quoted() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, r#"Using "West Bengal" data source"#, 1, "shell");
        assert!(!wm.is_empty());
        let output = wm.format_for_prompt();
        assert!(output.contains("West Bengal"));
    }

    #[test]
    fn test_entity_extraction_pascal_case() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, "Use MultiIndex from DataFrame", 1, "shell");
        let output = wm.format_for_prompt();
        assert!(output.contains("MultiIndex"));
    }

    #[test]
    fn test_entity_extraction_allcaps() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, "Analyzing AAPL and TSLA stocks", 1, "shell");
        let output = wm.format_for_prompt();
        assert!(output.contains("AAPL"));
        assert!(output.contains("TSLA"));
    }

    #[test]
    fn test_common_words_filtered() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, "THE JSON HTTP API", 1, "shell");
        // All common words — nothing should be added
        assert!(wm.is_empty());
    }

    #[test]
    fn test_error_recorded_as_discovery() {
        let mut wm = WorkingMemory::new(5000);
        process_tool_result(&mut wm, "shell", "", Some("Connection refused"), 3);
        let output = wm.format_for_prompt();
        assert!(output.contains("shell error: Connection refused"));
    }

    #[test]
    fn test_delegation_started() {
        let mut wm = WorkingMemory::new(5000);
        process_delegation_started(&mut wm, "research-agent", "fetch stock data");
        let output = wm.format_for_prompt();
        assert!(output.contains("research-agent"));
        assert!(output.contains("running"));
    }

    #[test]
    fn test_delegation_completed() {
        let mut wm = WorkingMemory::new(5000);
        process_delegation_started(&mut wm, "research-agent", "fetch stock data");
        process_delegation_completed(&mut wm, "research-agent", "Found 8 news sources\nSaved to ward\nAnalysis complete");
        let output = wm.format_for_prompt();
        assert!(output.contains("completed"));
        assert!(output.contains("Found 8 news sources"));
    }

    #[test]
    fn test_extract_key_lines() {
        let text = "First short\nA longer line that has real content here\nAnother meaningful line with data\nShort";
        let lines = extract_key_lines(text, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("longer line"));
    }

    #[test]
    fn test_respond_tool_skipped() {
        let mut wm = WorkingMemory::new(5000);
        process_tool_result(&mut wm, "respond", "Here is the final answer", None, 10);
        assert!(wm.is_empty());
    }
}
```

- [ ] **Step 2: Export middleware module**

In `gateway/gateway-execution/src/invoke/mod.rs`, add:

```rust
pub mod working_memory_middleware;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package gateway-execution -- working_memory_middleware`
Expected: 9 tests pass.

- [ ] **Step 4: Run quality checks**

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/invoke/working_memory_middleware.rs gateway/gateway-execution/src/invoke/mod.rs
git commit -m "feat(working-memory): middleware for entity extraction and tool result processing"
```

---

### Task 3: Integrate Working Memory into Execution Loop

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

This is the critical integration task. Working memory needs to be:
1. Initialized in `spawn_execution_task()`
2. Updated after each tool result
3. Formatted and injected before each LLM call

- [ ] **Step 1: Read the current execution loop structure**

Read `gateway/gateway-execution/src/runner.rs` lines 865-1020 (the `spawn_execution_task` function and its event processing loop). Understand:
- Where `history` is created and passed to executor
- Where tool results are processed (the `StreamEvent::ToolResult` match arm)
- Where `process_stream_event` is called
- Where delegation events are handled

- [ ] **Step 2: Add working memory initialization**

Inside `spawn_execution_task()`, after the `stream_ctx` is created (~line 903), add:

```rust
use crate::invoke::working_memory::WorkingMemory;
use crate::invoke::working_memory_middleware;

// Initialize working memory (1500 token budget)
let mut working_memory = WorkingMemory::new(1500);
```

- [ ] **Step 3: Seed working memory from recalled corrections**

After the working memory is created, seed it with any corrections from the history (system messages containing "Recalled Context"):

```rust
// Seed working memory from recalled context (corrections)
for msg in &history {
    if msg.role == "system" && msg.text_content().contains("Recalled") {
        for line in msg.text_content().lines() {
            let trimmed = line.trim().trim_start_matches("- ");
            if trimmed.starts_with("[correction]") || trimmed.starts_with("[pattern]") {
                working_memory.add_correction(trimmed);
            }
        }
    }
}
```

- [ ] **Step 4: Update working memory after tool results**

In the `StreamEvent::ToolResult` match arm (~line 958), after the existing tool result processing, add:

```rust
// Update working memory with tool result
working_memory_middleware::process_tool_result(
    &mut working_memory,
    &tool_acc.current_tool_name().unwrap_or_default(),
    result,
    error.as_deref(),
    handle.current_iteration(),
);
```

Note: You'll need to check what method gives you the current tool name from `tool_acc`. Read the `ToolCallAccumulator` in `stream.rs` to find it. If there's no such method, track the tool name in a local variable set during `ToolCallStart`.

- [ ] **Step 5: Handle delegation events in working memory**

Find where delegation started/completed events are detected in the stream processing. These may be in `process_stream_event` or in the event match arms. Add calls:

```rust
// On DelegationStarted:
working_memory_middleware::process_delegation_started(
    &mut working_memory,
    &child_agent_id,
    &task,
);

// On DelegationCompleted:
working_memory_middleware::process_delegation_completed(
    &mut working_memory,
    &child_agent_id,
    &result,
);
```

If delegation events aren't directly visible in `spawn_execution_task`, they may be in the `process_stream_event` return value or in the gateway event. In that case, check the `StreamEvent` enum for delegation variants and handle them in the match arm at ~line 943.

- [ ] **Step 6: Inject working memory before each LLM iteration**

This is the trickiest part. The executor runs internally in a loop — we can't directly inject per-iteration. Instead, use the existing history that's passed to the executor.

The approach: the working memory content is injected as a system message that gets UPDATED each iteration. Since the executor uses `execute_stream` which takes history + callback, we need to add the working memory as a mutable system message.

Look for where `history` is passed to `executor.execute_stream()`. Before that call, append the working memory:

```rust
// Inject working memory into history before execution
if !working_memory.is_empty() {
    history.push(ChatMessage::system(working_memory.format_for_prompt()));
}
```

For subsequent iterations (after tool results update working memory), the executor's internal history already includes prior messages. The working memory system message will be part of the initial context. For updates to take effect mid-execution, we'd need to modify the executor — but for Phase 2, injecting at session start and updating between continuation turns is sufficient.

The working memory already benefits from being seeded with corrections and populated during tool processing — even if the LLM only sees it on the first iteration, the corrections are there from the start.

- [ ] **Step 7: Verify compilation**

Run: `cargo check --workspace`
Expected: Clean.

- [ ] **Step 8: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat(working-memory): integrate into execution loop — seed, update on tool results, inject into history"
```

---

### Task 4: Final Checks

- [ ] **Step 1: Format and lint**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: Clean.

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: All pass.

- [ ] **Step 3: UI checks**

Run: `cd apps/ui && npm run build && npm run lint`
Expected: Clean (no UI changes in Phase 2).

- [ ] **Step 4: Commit any fixes**

```bash
git add -A && git commit -m "chore: cargo fmt, clippy clean" || echo "nothing to commit"
```

- [ ] **Step 5: Push**

```bash
git push
```
