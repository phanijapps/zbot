# Knowledge Graph Evolution — Phase 6d Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Close the loop. Add (1) real-time tool result extraction so entities from `web_fetch`, `shell`, and similar tools enter the graph as they happen, and (2) MAGMA-style multi-view query modes so agents can query the graph through semantic, temporal, entity, or hybrid lenses.

**Architecture:** Two independent deliverables:
1. `ToolResultExtractor` — schema-aware parser that walks tool outputs post-call and emits entity extractions tagged with `epistemic_class = archival` + episode provenance.
2. Graph query views — extend `graph_query` tool with a `view` parameter routing to `semantic`, `temporal`, `entity`, or `hybrid` traversal modes.

**Spec:** `docs/superpowers/specs/2026-04-12-knowledge-graph-evolution-design.md` — Phase 6d

**Branch:** `feature/sentient` (continues from Phase 6c)

---

## Task 1: ToolResultExtractor Module

**Files:**
- Create: `gateway/gateway-execution/src/tool_result_extractor.rs`
- Modify: `gateway/gateway-execution/src/lib.rs`

- [ ] **Step 1: Create the module**

```rust
//! ToolResultExtractor — parses structured tool outputs and emits entity
//! extractions in real time (post-tool-result, pre-next-iteration).
//!
//! Unlike distillation (end-of-session, LLM-based), this runs during
//! execution with zero LLM cost using schema-aware parsers per tool.
//!
//! Each extraction produces an `Episode` with `source_type = tool_result`
//! and `source_ref = tool_call_id`, enabling drill-down from graph to
//! the exact tool invocation that produced it.

use gateway_database::{EpisodeSource, KgEpisode, KgEpisodeRepository};
use knowledge_graph::{Entity, EntityType, ExtractedKnowledge, GraphStorage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Extract entities from a tool result and persist them with episode provenance.
///
/// Dispatches to a tool-specific parser. Tool outputs that don't match any
/// known schema produce zero extractions (silent no-op).
///
/// Errors are logged at warn level and never propagate — extraction is
/// best-effort and must never block the execution loop.
pub async fn extract_and_persist(
    tool_name: &str,
    tool_call_id: &str,
    result_text: &str,
    session_id: &str,
    agent_id: &str,
    episode_repo: &KgEpisodeRepository,
    graph: &Arc<GraphStorage>,
) {
    let entities = extract_from_tool(tool_name, result_text);
    if entities.is_empty() {
        return;
    }

    let episode_id = match ensure_episode(
        episode_repo,
        tool_call_id,
        result_text,
        session_id,
        agent_id,
    ) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(tool = %tool_name, error = %e, "Failed to create tool-result episode");
            return;
        }
    };

    let stamped = entities
        .into_iter()
        .map(|e| stamp_provenance(e, &episode_id, tool_call_id))
        .collect::<Vec<_>>();

    let knowledge = ExtractedKnowledge {
        entities: stamped,
        relationships: Vec::new(),
    };
    if let Err(e) = graph.store_knowledge(agent_id, knowledge).await {
        tracing::warn!(tool = %tool_name, error = %e, "Failed to persist tool-result entities");
    }
}

/// Dispatch table: route tool name to its specific extractor.
fn extract_from_tool(tool_name: &str, result_text: &str) -> Vec<Entity> {
    // Parse JSON envelope once if possible (tools typically return JSON strings)
    let parsed = serde_json::from_str::<Value>(result_text).ok();

    match tool_name {
        "web_fetch" | "web-fetch" | "webfetch" => extract_web_fetch(parsed.as_ref(), result_text),
        "shell" => extract_shell(parsed.as_ref(), result_text),
        "multimodal_analyze" | "multimodal" => {
            extract_multimodal(parsed.as_ref(), result_text)
        }
        _ => Vec::new(),
    }
}

/// Extract entities from a web_fetch result.
/// Known fields: url, title, description, content, publish_date.
fn extract_web_fetch(parsed: Option<&Value>, _raw: &str) -> Vec<Entity> {
    let Some(value) = parsed else {
        return Vec::new();
    };
    let obj = value.as_object().unwrap_or_else(|| panic_empty());
    let mut entities = Vec::new();

    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        let mut document = Entity::new(
            "__global__".to_string(),
            EntityType::Document,
            url.to_string(),
        );
        if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
            document.properties.insert(
                "title".to_string(),
                Value::String(title.to_string()),
            );
        }
        if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
            document
                .properties
                .insert("description".to_string(), Value::String(desc.to_string()));
        }
        if let Some(date) = obj
            .get("publish_date")
            .or(obj.get("publishDate"))
            .and_then(|v| v.as_str())
        {
            document
                .properties
                .insert("publication_date".to_string(), Value::String(date.to_string()));
        }
        entities.push(document);
    }
    entities
}

/// Extract from shell tool output: currently just captures file paths mentioned
/// in stdout (useful for grep/find/ls outputs) as File entities.
fn extract_shell(parsed: Option<&Value>, _raw: &str) -> Vec<Entity> {
    let Some(value) = parsed else {
        return Vec::new();
    };
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };

    // Only extract when the shell command succeeded
    let success = obj
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !success {
        return Vec::new();
    }

    let stdout = obj
        .get("stdout")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Capture file-path-looking tokens (starting with / or ./, 3+ segments)
    let paths = extract_file_paths(stdout);
    paths
        .into_iter()
        .take(10) // cap to avoid grep floods
        .map(|path| {
            let name = path.clone();
            let mut e = Entity::new("__global__".to_string(), EntityType::File, name);
            e.properties
                .insert("path".to_string(), Value::String(path));
            e
        })
        .collect()
}

/// Extract from multimodal_analyze result — look for named entities in the
/// analysis output field.
fn extract_multimodal(parsed: Option<&Value>, _raw: &str) -> Vec<Entity> {
    let Some(value) = parsed else {
        return Vec::new();
    };
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    // Heuristic: pull a top-level "entities" array if present
    let arr = match obj.get("entities").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|item| {
            let o = item.as_object()?;
            let name = o.get("name").and_then(|v| v.as_str())?;
            let type_str = o
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("concept");
            let mut entity = Entity::new(
                "__global__".to_string(),
                EntityType::from_str(type_str),
                name.to_string(),
            );
            for (k, v) in o {
                if k != "name" && k != "type" {
                    entity.properties.insert(k.clone(), v.clone());
                }
            }
            Some(entity)
        })
        .collect()
}

fn extract_file_paths(text: &str) -> Vec<String> {
    // Very conservative path matcher: tokens starting with / or ./ with
    // at least one more /. Skip URLs (contain ://).
    text.split_whitespace()
        .filter(|tok| {
            let t = tok.trim_end_matches([',', ':', ';', ')', ']', '"', '\'']);
            (t.starts_with('/') || t.starts_with("./"))
                && t.matches('/').count() >= 1
                && !t.contains("://")
                && t.len() < 300
        })
        .map(|t| {
            t.trim_end_matches([',', ':', ';', ')', ']', '"', '\''])
                .to_string()
        })
        .collect()
}

fn stamp_provenance(mut entity: Entity, episode_id: &str, tool_call_id: &str) -> Entity {
    entity.properties.insert(
        "_source_episode_id".to_string(),
        Value::String(episode_id.to_string()),
    );
    entity.properties.insert(
        "_source_ref".to_string(),
        Value::String(format!("tool_call:{}", tool_call_id)),
    );
    entity.properties.insert(
        "_epistemic_class".to_string(),
        Value::String("archival".to_string()),
    );
    entity
}

fn ensure_episode(
    repo: &KgEpisodeRepository,
    tool_call_id: &str,
    content: &str,
    session_id: &str,
    agent_id: &str,
) -> Result<String, String> {
    let content_hash = hash_content(content);
    // Dedup: if we've seen this exact tool output before, reuse the episode
    if let Ok(Some(existing)) =
        repo.get_by_content_hash(&content_hash, EpisodeSource::ToolResult.as_str())
    {
        return Ok(existing.id);
    }
    let ep = KgEpisode {
        id: format!("ep-{}", uuid::Uuid::new_v4()),
        source_type: EpisodeSource::ToolResult.as_str().to_string(),
        source_ref: tool_call_id.to_string(),
        content_hash,
        session_id: Some(session_id.to_string()),
        agent_id: agent_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    repo.upsert_episode(&ep)?;
    Ok(ep.id)
}

fn hash_content(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("{:x}", h.finalize())
}

// Silent fallback: used when serde_json returns an object we don't expect.
// Returns an empty map that lives for 'static.
fn panic_empty() -> &'static serde_json::Map<String, Value> {
    use std::sync::OnceLock;
    static EMPTY: OnceLock<serde_json::Map<String, Value>> = OnceLock::new();
    EMPTY.get_or_init(serde_json::Map::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_fetch_extracts_document_entity() {
        let result = r#"{"url": "https://example.com/article", "title": "Test Article", "description": "A test"}"#;
        let entities = extract_from_tool("web_fetch", result);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "https://example.com/article");
        assert!(matches!(entities[0].entity_type, EntityType::Document));
        assert_eq!(
            entities[0].properties.get("title").unwrap().as_str(),
            Some("Test Article")
        );
    }

    #[test]
    fn web_fetch_without_url_extracts_nothing() {
        let result = r#"{"title": "orphan"}"#;
        let entities = extract_from_tool("web_fetch", result);
        assert!(entities.is_empty());
    }

    #[test]
    fn shell_success_extracts_file_paths() {
        let result = r#"{"success": true, "exit_code": 0, "stdout": "/tmp/foo.rs\n./src/bar.rs\n"}"#;
        let entities = extract_from_tool("shell", result);
        assert_eq!(entities.len(), 2);
        assert!(entities.iter().any(|e| e.name == "/tmp/foo.rs"));
    }

    #[test]
    fn shell_failure_extracts_nothing() {
        let result = r#"{"success": false, "stdout": "/tmp/foo.rs"}"#;
        let entities = extract_from_tool("shell", result);
        assert!(entities.is_empty());
    }

    #[test]
    fn shell_skips_urls_in_stdout() {
        let result = r#"{"success": true, "stdout": "https://example.com/"}"#;
        let entities = extract_from_tool("shell", result);
        assert!(entities.is_empty());
    }

    #[test]
    fn multimodal_extracts_entities_array() {
        let result = r#"{"entities": [{"name": "Eiffel Tower", "type": "location", "city": "Paris"}]}"#;
        let entities = extract_from_tool("multimodal_analyze", result);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Eiffel Tower");
        assert!(matches!(entities[0].entity_type, EntityType::Location));
        assert_eq!(
            entities[0].properties.get("city").unwrap().as_str(),
            Some("Paris")
        );
    }

    #[test]
    fn unknown_tool_returns_empty() {
        let entities = extract_from_tool("mystery_tool", r#"{"any": "thing"}"#);
        assert!(entities.is_empty());
    }

    #[test]
    fn non_json_returns_empty() {
        let entities = extract_from_tool("web_fetch", "not json at all");
        assert!(entities.is_empty());
    }

    #[test]
    fn stamp_provenance_adds_three_markers() {
        let e = Entity::new("root".to_string(), EntityType::Concept, "x".to_string());
        let stamped = stamp_provenance(e, "ep-1", "call-42");
        assert!(stamped.properties.contains_key("_source_episode_id"));
        assert!(stamped.properties.contains_key("_source_ref"));
        assert_eq!(
            stamped.properties.get("_epistemic_class").unwrap().as_str(),
            Some("archival")
        );
    }

    #[test]
    fn file_path_extractor_caps_at_ten() {
        let stdout = (0..25)
            .map(|i| format!("/tmp/file{}.txt", i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = format!(r#"{{"success": true, "stdout": "{}"}}"#, stdout);
        let entities = extract_from_tool("shell", &result);
        assert!(entities.len() <= 10);
    }
}
```

- [ ] **Step 2: Export from lib.rs**

```rust
pub mod tool_result_extractor;
```

- [ ] **Step 3: Verify + commit**

Run: `cargo test --package gateway-execution -- tool_result_extractor`
Expected: 10 tests pass.

Run: `cargo fmt --all && cargo clippy --package gateway-execution -- -D warnings`

```bash
git add gateway/gateway-execution/src/tool_result_extractor.rs gateway/gateway-execution/src/lib.rs
git commit -m "feat(kg): ToolResultExtractor — real-time entity extraction from web_fetch, shell, multimodal outputs"
```

---

## Task 2: Wire Extractor Into Tool Result Path

**Files:** `gateway/gateway-execution/src/runner.rs`

In the execution loop where `StreamEvent::ToolResult` is handled (Phase 2 added working memory middleware here; we'll add tool_result_extractor alongside).

- [ ] **Step 1: Thread repos into spawn closure**

The spawn closure already has `kg_episode_repo` and `graph_storage` clones (from Phase 6a). Reuse those.

- [ ] **Step 2: Call extractor on tool results**

In the tool result handling branch (near where `process_tool_result` is called), add a fire-and-forget spawn:

```rust
// Phase 6d: real-time graph extraction from tool output.
// Runs async so it never blocks the execution loop.
if let (Some(ep_repo), Some(graph)) = (
    kg_episode_repo_for_tool_extract.as_ref(),
    graph_storage_for_tool_extract.as_ref(),
) {
    let tool_name_cl = current_tool_name.clone();
    let tool_id_cl = tool_id.clone();
    let result_cl = result.clone();
    let session_id_cl = session_id_inner.clone();
    let agent_id_cl = agent_id_inner.clone();
    let ep_repo_cl = ep_repo.clone();
    let graph_cl = graph.clone();
    tokio::spawn(async move {
        crate::tool_result_extractor::extract_and_persist(
            &tool_name_cl,
            &tool_id_cl,
            &result_cl,
            &session_id_cl,
            &agent_id_cl,
            ep_repo_cl.as_ref(),
            &graph_cl,
        )
        .await;
    });
}
```

Name the clones `*_for_tool_extract` (or reuse existing clones if the names already work). Follow the existing pattern used for other post-tool-result clones in the spawn.

- [ ] **Step 3: Verify + commit**

Run: `cargo check --workspace`
Run: `cargo test --workspace --lib --bins --tests`
Expected: All pass.

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat(kg): wire ToolResultExtractor into execution loop (post-tool-result, non-blocking)"
```

---

## Task 3: Multi-View Query Modes for graph_query Tool

**Files:**
- Modify: `runtime/agent-tools/src/tools/graph_query.rs`
- Modify: `services/knowledge-graph/src/service.rs`

The existing `graph_query` tool has 3 actions: `search`, `neighbors`, `context`. We add a `view` parameter that picks the lens through which results are ranked/filtered.

- [ ] **Step 1: Add query views to GraphService**

In `services/knowledge-graph/src/service.rs`, add four methods (or refactor search_entities to accept a view enum):

```rust
#[derive(Debug, Clone, Copy)]
pub enum GraphView {
    Semantic,   // order by mention_count (current default)
    Temporal,   // order by most-recently-seen entities + events
    Entity,     // order by connection count (most-connected first)
    Hybrid,     // combine all three with reranking
}

impl Default for GraphView {
    fn default() -> Self { GraphView::Semantic }
}

pub async fn search_entities_view(
    &self,
    agent_id: &str,
    query: &str,
    view: GraphView,
    limit: usize,
) -> GraphResult<Vec<Entity>> {
    match view {
        GraphView::Semantic => self.search_entities(agent_id, query, limit).await,
        GraphView::Temporal => self.search_entities_temporal(agent_id, query, limit).await,
        GraphView::Entity => self.search_entities_by_connections(agent_id, query, limit).await,
        GraphView::Hybrid => self.search_entities_hybrid(agent_id, query, limit).await,
    }
}

async fn search_entities_temporal(
    &self,
    agent_id: &str,
    query: &str,
    limit: usize,
) -> GraphResult<Vec<Entity>> {
    // Same as search_entities but ORDER BY last_seen_at DESC
    self.storage
        .search_entities_order_by(agent_id, query, "last_seen_at DESC", limit)
        .await
}

async fn search_entities_by_connections(
    &self,
    agent_id: &str,
    query: &str,
    limit: usize,
) -> GraphResult<Vec<Entity>> {
    // Search, then count relationships per result, sort by count desc.
    let candidates = self.storage.search_entities(agent_id, query).await?;
    let mut scored: Vec<(Entity, i64)> = Vec::with_capacity(candidates.len());
    for e in candidates {
        let count = self.storage.count_relationships_for(&e.id).await?;
        scored.push((e, count));
    }
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(scored.into_iter().take(limit).map(|(e, _)| e).collect())
}

async fn search_entities_hybrid(
    &self,
    agent_id: &str,
    query: &str,
    limit: usize,
) -> GraphResult<Vec<Entity>> {
    // Reranking combination: take 2x results from semantic + temporal +
    // by-connections, dedup, score by rank-reciprocal, truncate.
    let wide = limit.saturating_mul(2).max(10);
    let semantic = self.search_entities(agent_id, query, wide).await.unwrap_or_default();
    let temporal = self.search_entities_temporal(agent_id, query, wide).await.unwrap_or_default();
    let by_conn = self.search_entities_by_connections(agent_id, query, wide).await.unwrap_or_default();

    let merged = merge_by_reciprocal_rank(&[semantic, temporal, by_conn]);
    Ok(merged.into_iter().take(limit).collect())
}
```

Add `search_entities_order_by` and `count_relationships_for` helpers to `storage.rs`:

```rust
// in storage.rs
pub async fn search_entities_order_by(
    &self,
    agent_id: &str,
    query: &str,
    order_clause: &str,   // e.g., "last_seen_at DESC"
    limit: usize,
) -> GraphResult<Vec<Entity>> {
    // Whitelist order_clause to prevent SQL injection
    let safe_order = match order_clause {
        "last_seen_at DESC" | "mention_count DESC" | "first_seen_at DESC" => order_clause,
        _ => "mention_count DESC",
    };
    // ... similar to existing search_entities but with the whitelisted ORDER BY
}

pub async fn count_relationships_for(&self, entity_id: &str) -> GraphResult<i64> {
    // SELECT COUNT(*) FROM kg_relationships WHERE source_entity_id = ?1 OR target_entity_id = ?1
}
```

Add `merge_by_reciprocal_rank` as a private helper function in service.rs.

- [ ] **Step 2: Add view parameter to graph_query tool**

In `runtime/agent-tools/src/tools/graph_query.rs`, update the JSON schema:

```json
{
  "view": {
    "type": "string",
    "enum": ["semantic", "temporal", "entity", "hybrid"],
    "description": "Query view: semantic (by similarity, default), temporal (most recent first), entity (most connected first), hybrid (reranked combination)"
  }
}
```

Parse the view in the `search` and `context` actions. Default to `semantic`. Route to the new `search_entities_view` on GraphStorageAccess.

Add method to `GraphStorageAccess` trait:
```rust
async fn search_entities_with_view(
    &self,
    query: &str,
    entity_type: Option<&str>,
    view: &str,
    limit: usize,
) -> Result<Vec<EntityInfo>, String>;
```

Update `GraphStorageAdapter` in `gateway-execution` to implement it by calling `GraphService::search_entities_view`.

- [ ] **Step 3: Unit tests for view logic**

In `service.rs` tests (or a new test file), add tests for `merge_by_reciprocal_rank`:

```rust
#[test]
fn reciprocal_rank_merges_duplicates() {
    let a = vec![entity("x"), entity("y")];  // x=rank1, y=rank2
    let b = vec![entity("y"), entity("x")];  // y=rank1, x=rank2
    let merged = merge_by_reciprocal_rank(&[a, b]);
    // x: 1/1 + 1/2 = 1.5; y: 1/2 + 1/1 = 1.5 — tied, but both present exactly once
    assert_eq!(merged.len(), 2);
}

#[test]
fn reciprocal_rank_preserves_order() {
    let a = vec![entity("a"), entity("b"), entity("c")];
    let b = vec![entity("a"), entity("b"), entity("c")];
    let merged = merge_by_reciprocal_rank(&[a, b]);
    assert_eq!(merged[0].name, "a");
    assert_eq!(merged[1].name, "b");
    assert_eq!(merged[2].name, "c");
}
```

Tests for `count_relationships_for` in storage.rs tests using an in-memory SQLite setup.

- [ ] **Step 4: Verify + commit**

Run: `cargo test --workspace --lib --bins --tests`
Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

```bash
git add runtime/agent-tools/src/tools/graph_query.rs services/knowledge-graph/src/service.rs services/knowledge-graph/src/storage.rs gateway/gateway-execution/src/invoke/graph_adapter.rs
git commit -m "feat(kg): MAGMA-style multi-view queries (semantic/temporal/entity/hybrid) for graph_query tool"
```

---

## Task 4: Final Checks + Push

- [ ] **Step 1: All tests**

Run: `cargo test --workspace --lib --bins --tests`
Expected: 37+ suites pass.

- [ ] **Step 2: fmt + clippy + complexity**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

Run: `cargo clippy --package gateway-execution --package knowledge-graph --package agent-tools --lib --tests -- -W clippy::cognitive_complexity 2>&1 | grep cognitive`
Expected: no new flags on Phase 6d functions.

- [ ] **Step 3: Push**

```bash
git push
```
