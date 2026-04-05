# Memory Brain Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform z-Bot's memory from passive storage into an active cognitive layer that saves tokens, improves accuracy, and learns from every session.

**Architecture:** Five memory loops (intent+memory, subagent priming, graph recall, predictive recall, mid-session injection) plus an accuracy layer (fact verification, entity normalization, relationship dedup). Each phase is independently deployable and testable. Phase 0 fixes graph correctness, Phase 1 delivers the biggest token savings, Phases 2-4 are incremental improvements.

**Tech Stack:** Rust (gateway, knowledge-graph, agent-runtime), SQLite (rusqlite), fastembed/OpenAI embeddings, serde_json

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `services/knowledge-graph/src/storage.rs` | Modify | Relationship dedup (unique index), entity normalization |
| `gateway/gateway-database/src/schema.rs` | Modify | Migration v14: dedup existing relationships, add unique index |
| `gateway/gateway-execution/src/recall.rs` | Modify | `recall_for_intent()`, `recall_for_delegation()`, predictive boost |
| `gateway/gateway-execution/src/middleware/intent_analysis.rs` | Modify | Memory query before intent LLM call |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Modify | Enhanced priming context for subagents |
| `gateway/gateway-execution/src/runner.rs` | Modify | Switch to graph-powered recall for first message |
| `gateway/gateway-execution/src/distillation.rs` | Modify | Fact verification, entity normalization before storage |
| `runtime/agent-runtime/src/executor.rs` | Modify | Mid-session recall injection |

---

### Task 1: Graph Relationship Dedup (Phase 0)

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs:1023-1050`
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Fix `store_relationship()` to upsert on the (source, target, type) triple**

In `services/knowledge-graph/src/storage.rs`, replace the `store_relationship` function (line 1023):

```rust
/// Store a relationship (upsert based on source + target + type — NOT id)
fn store_relationship(conn: &Connection, agent_id: &str, relationship: Relationship) -> GraphResult<()> {
    let rel_type_str = relationship.relationship_type.as_str();
    let properties_json = serde_json::to_string(&relationship.properties)
        .unwrap_or_else(|_| "".to_string());

    conn.execute(
        "INSERT INTO kg_relationships (id, agent_id, source_entity_id, target_entity_id, relationship_type, properties, first_seen_at, last_seen_at, mention_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(source_entity_id, target_entity_id, relationship_type) DO UPDATE SET
            last_seen_at = excluded.last_seen_at,
            mention_count = mention_count + 1,
            properties = excluded.properties",
        params![
            relationship.id,
            agent_id,
            relationship.source_entity_id,
            relationship.target_entity_id,
            rel_type_str,
            properties_json,
            relationship.first_seen_at.to_rfc3339(),
            relationship.last_seen_at.to_rfc3339(),
            relationship.mention_count,
        ],
    ).map_err(|e| GraphError::Database(e))?;

    Ok(())
}
```

- [ ] **Step 2: Add unique index to schema initialization**

In `services/knowledge-graph/src/storage.rs`, in the `initialize_schema()` function, after the existing relationship indexes (around line 895), add:

```rust
conn.execute(
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_kg_rel_unique
     ON kg_relationships(source_entity_id, target_entity_id, relationship_type)",
    [],
).map_err(GraphError::Database)?;
```

- [ ] **Step 3: Add migration v14 to dedup existing relationships and add unique index**

In `gateway/gateway-database/src/schema.rs`, increment `CURRENT_VERSION` to 14, and add after the `version < 13` block:

```rust
if version < 14 {
    tracing::info!("Migrating database: v13 → v14 (knowledge graph relationship dedup)");

    // Dedup existing relationships: keep the one with lowest rowid per triple
    conn.execute_batch("
        DELETE FROM kg_relationships WHERE rowid NOT IN (
            SELECT MIN(rowid) FROM kg_relationships
            GROUP BY source_entity_id, target_entity_id, relationship_type
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_kg_rel_unique
        ON kg_relationships(source_entity_id, target_entity_id, relationship_type);
    ").map_err(|e| anyhow::anyhow!("v14 migration failed: {}", e))?;
}
```

Note: The knowledge graph has its own DB file (`knowledge_graph.db`). Check if the migration runs against the right DB. If the knowledge graph schema is managed in `services/knowledge-graph/src/storage.rs::initialize_schema()` (not in `gateway-database/src/schema.rs`), then do the dedup + index creation there instead. Read both files to confirm which manages `kg_relationships`.

- [ ] **Step 4: Build and verify**

```bash
cargo build -p knowledge-graph -p gateway 2>&1 | grep "^error"
```
Expected: No errors.

- [ ] **Step 5: Test dedup manually**

```bash
# Before: check for duplicates
python3 -c "
import sqlite3
kg = sqlite3.connect('$HOME/Documents/zbot/data/knowledge_graph.db')
dupes = kg.execute('''
    SELECT s.name, r.relationship_type, t.name, COUNT(*) as cnt
    FROM kg_relationships r
    JOIN kg_entities s ON r.source_entity_id = s.id
    JOIN kg_entities t ON r.target_entity_id = t.id
    GROUP BY r.source_entity_id, r.target_entity_id, r.relationship_type
    HAVING cnt > 1
''').fetchall()
print(f'Duplicate relationships: {len(dupes)}')
for d in dupes: print(f'  {d}')
"
```

After restarting the daemon (which triggers migration), verify duplicates are gone.

- [ ] **Step 6: Commit**

```bash
git add services/knowledge-graph/src/storage.rs gateway/gateway-database/src/schema.rs
git commit -m "fix: dedup graph relationships with unique index on (source, target, type)"
```

---

### Task 2: Entity Normalization in Distillation (Phase 0)

**Files:**
- Modify: `services/knowledge-graph/src/storage.rs`
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add `normalize_entity_name()` helper to storage.rs**

In `services/knowledge-graph/src/storage.rs`, add before the `store_entity` function:

```rust
/// Normalize entity name for dedup matching.
/// Strips path prefixes for files, lowercases for comparison.
fn normalize_entity_name(name: &str, entity_type: &str) -> String {
    let mut normalized = name.trim().to_string();

    // For file entities, strip path prefixes for matching
    // Keep the full path as the canonical name, but match on basename
    if entity_type == "file" {
        if let Some(basename) = normalized.rsplit('/').next() {
            if !basename.is_empty() {
                normalized = basename.to_string();
            }
        }
    }

    normalized
}
```

- [ ] **Step 2: Update `store_entity()` to use normalized names for matching**

Replace the entity dedup lookup in `store_entity()` (line 953-954):

```rust
// Check for existing entity with same normalized name + type across ALL agents
let normalized = normalize_entity_name(&entity.name, entity_type_str);
if let Some(existing_id) = find_entity_by_name_global(conn, &normalized, entity_type_str)? {
    // Bump existing entity — dedup
    // Also store the full name as an alias in properties if different
    let mut props: serde_json::Value = serde_json::from_str(&properties_json).unwrap_or(serde_json::json!({}));
    if entity.name != normalized {
        let aliases = props.as_object_mut().unwrap();
        let alias_list = aliases.entry("aliases").or_insert(serde_json::json!([]));
        if let Some(arr) = alias_list.as_array_mut() {
            let full_name = serde_json::Value::String(entity.name.clone());
            if !arr.contains(&full_name) {
                arr.push(full_name);
            }
        }
    }
    let updated_props = serde_json::to_string(&props).unwrap_or_default();
    conn.execute(
        "UPDATE kg_entities SET mention_count = mention_count + 1, last_seen_at = ?1, properties = ?2 WHERE id = ?3",
        params![entity.last_seen_at.to_rfc3339(), updated_props, existing_id],
    ).map_err(GraphError::Database)?;
    return Ok(existing_id);
}
```

- [ ] **Step 3: Update `find_entity_by_name_global()` for normalized matching**

Also add a fallback check that compares basename of file entities:

```rust
fn find_entity_by_name_global(conn: &Connection, name: &str, entity_type: &str) -> GraphResult<Option<String>> {
    // Exact match first (case-insensitive)
    let mut stmt = conn.prepare(
        "SELECT id FROM kg_entities WHERE name = ?1 COLLATE NOCASE AND entity_type = ?2 LIMIT 1"
    ).map_err(GraphError::Database)?;

    match stmt.query_row(params![name, entity_type], |row| row.get::<_, String>(0)) {
        Ok(id) => return Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => {},
        Err(e) => return Err(GraphError::Database(e)),
    }

    // For file entities, also try matching basename against full paths
    if entity_type == "file" {
        let like_pattern = format!("%/{}", name);
        let mut stmt2 = conn.prepare(
            "SELECT id FROM kg_entities WHERE name LIKE ?1 COLLATE NOCASE AND entity_type = ?2 LIMIT 1"
        ).map_err(GraphError::Database)?;

        match stmt2.query_row(params![like_pattern, entity_type], |row| row.get::<_, String>(0)) {
            Ok(id) => return Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {},
            Err(e) => return Err(GraphError::Database(e)),
        }
    }

    Ok(None)
}
```

- [ ] **Step 4: Build and verify**

```bash
cargo build -p knowledge-graph -p gateway 2>&1 | grep "^error"
```

- [ ] **Step 5: Commit**

```bash
git add services/knowledge-graph/src/storage.rs gateway/gateway-execution/src/distillation.rs
git commit -m "feat: entity normalization — file basename matching, alias tracking"
```

---

### Task 3: Recall for Intent Analysis (Phase 1a — Loop 1)

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`

- [ ] **Step 1: Add `recall_for_intent()` method to MemoryRecall**

In `gateway/gateway-execution/src/recall.rs`, add a new public method after `recall_with_graph()`:

```rust
/// Lightweight recall for intent analysis — returns formatted memory context
/// for injection into the intent analysis prompt. Faster than full recall
/// (skips episode search, limits to 5 facts + 1-hop graph).
pub async fn recall_for_intent(
    &self,
    user_message: &str,
    limit: usize,
) -> Result<String, String> {
    // Use __global__ agent for cross-agent recall
    let agent_id = "__global__";

    // 1. Hybrid search — top facts relevant to the message
    let facts = self.recall(agent_id, user_message, limit, None).await?;

    if facts.is_empty() {
        return Ok(String::new());
    }

    let mut sections: Vec<String> = Vec::new();

    // 2. Format recalled facts by category
    let corrections: Vec<&ScoredFact> = facts.iter().filter(|f| f.fact.category == "correction").collect();
    let strategies: Vec<&ScoredFact> = facts.iter().filter(|f| f.fact.category == "strategy").collect();
    let domain: Vec<&ScoredFact> = facts.iter().filter(|f| !["correction", "strategy"].contains(&f.fact.category.as_str())).collect();

    if !corrections.is_empty() {
        let items: Vec<String> = corrections.iter().map(|f| format!("- [correction] {}", f.fact.content)).collect();
        sections.push(format!("## Corrections (MUST follow)\n{}", items.join("\n")));
    }
    if !strategies.is_empty() {
        let items: Vec<String> = strategies.iter().map(|f| format!("- [strategy] {}", f.fact.content)).collect();
        sections.push(format!("## Proven Strategies\n{}", items.join("\n")));
    }
    if !domain.is_empty() {
        let items: Vec<String> = domain.iter().map(|f| format!("- [{}] {}", f.fact.category, f.fact.content)).collect();
        sections.push(format!("## Domain Knowledge\n{}", items.join("\n")));
    }

    // 3. Graph context — extract entity names from facts, get 1-hop neighbors
    if let Some(ref graph) = self.graph_service {
        let entity_names: Vec<String> = facts.iter()
            .flat_map(|f| extract_entity_names(&f.fact.content))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if !entity_names.is_empty() {
            if let Ok(neighbors) = graph.search_entities_by_names(&entity_names, 5).await {
                if !neighbors.is_empty() {
                    let items: Vec<String> = neighbors.iter()
                        .map(|e| format!("- {} ({})", e.name, e.entity_type.as_str()))
                        .collect();
                    sections.push(format!("## Related Entities\n{}", items.join("\n")));
                }
            }
        }
    }

    // 4. Episode context — top 3 similar past sessions
    if let Some(ref episode_repo) = self.episode_repo {
        if let Some(ref embedding_client) = self.embedding_client {
            if let Ok(embeddings) = embedding_client.embed(&[user_message]).await {
                if let Some(query_embedding) = embeddings.first() {
                    if let Ok(episodes) = episode_repo.search_by_similarity(query_embedding, 3).await {
                        if !episodes.is_empty() {
                            let items: Vec<String> = episodes.iter().map(|ep| {
                                let strategy = ep.strategy_used.as_deref().unwrap_or("unknown");
                                format!("- \"{}\" → {}, strategy: {}", ep.task_summary, ep.outcome, strategy)
                            }).collect();
                            sections.push(format!("## Similar Past Sessions\n{}", items.join("\n")));
                        }
                    }
                }
            }
        }
    }

    if sections.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("<memory_context>\n{}\n</memory_context>", sections.join("\n\n")))
}
```

Note: `extract_entity_names()` and `graph.search_entities_by_names()` may need to be implemented or the function may already exist. Check `recall.rs` for existing entity name extraction helpers. If `search_entities_by_names` doesn't exist on `GraphService`, use `search_entities()` with individual name lookups, or traverse the graph via `GraphTraversal::connected_entities()`.

- [ ] **Step 2: Wire memory recall into `analyze_intent()`**

In `gateway/gateway-execution/src/middleware/intent_analysis.rs`, modify the `analyze_intent()` function signature to accept an optional `MemoryRecall` reference:

```rust
pub async fn analyze_intent(
    llm_client: &dyn LlmClient,
    user_message: &str,
    fact_store: &dyn MemoryFactStore,
    memory_recall: Option<&MemoryRecall>,
) -> Result<IntentAnalysis, String> {
```

Before the LLM call (around line 335), add memory context:

```rust
    // Query memory for context before intent analysis
    let memory_context = if let Some(recall) = memory_recall {
        recall.recall_for_intent(user_message, 5).await.unwrap_or_default()
    } else {
        String::new()
    };

    // Build messages with memory context
    let user_content = if memory_context.is_empty() {
        format_user_template(user_message, &results.skills, &results.agents, &results.wards)
    } else {
        format!(
            "{}\n\n{}\n",
            memory_context,
            format_user_template(user_message, &results.skills, &results.agents, &results.wards)
        )
    };

    let messages = vec![
        ChatMessage::system(INTENT_ANALYSIS_PROMPT.to_string()),
        ChatMessage::user(user_content),
    ];
```

- [ ] **Step 3: Update all callers of `analyze_intent` to pass `memory_recall`**

Search for all call sites of `analyze_intent` and add the new parameter. The main caller is in `runner.rs`. Pass `self.memory_recall.as_deref()` or `memory_recall.as_ref().map(|r| r.as_ref())`.

- [ ] **Step 4: Build and verify**

```bash
cargo build -p gateway-execution -p gateway 2>&1 | grep "^error"
```

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/gateway-execution/src/middleware/intent_analysis.rs gateway/gateway-execution/src/runner.rs
git commit -m "feat: intent analysis queries memory — facts, graph, episodes before planning"
```

---

### Task 4: Enhanced Subagent Priming (Phase 1b — Loop 2)

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs`

- [ ] **Step 1: Add `recall_for_delegation()` method to MemoryRecall**

In `gateway/gateway-execution/src/recall.rs`, add:

```rust
/// Recall facts specifically for a delegated agent task.
/// Focuses on: corrections for this agent, ward-scoped domain knowledge,
/// skill recommendations, and prior work context from graph.
pub async fn recall_for_delegation(
    &self,
    agent_id: &str,
    task: &str,
    ward_id: Option<&str>,
    limit: usize,
) -> Result<String, String> {
    // 1. Recall facts relevant to the task, scoped to ward
    let facts = self.recall(agent_id, task, limit, ward_id).await?;

    // 2. Also fetch corrections specifically for this agent
    let agent_corrections = self.memory_repo
        .search_memory_facts_fts(agent_id, "correction", 5)
        .map_err(|e| format!("Correction recall failed: {}", e))?;

    let mut sections: Vec<String> = Vec::new();

    // Corrections first (highest priority)
    let mut all_corrections: Vec<String> = agent_corrections.iter()
        .map(|f| format!("- {}", f.content))
        .collect();
    for f in facts.iter().filter(|f| f.fact.category == "correction") {
        let line = format!("- {}", f.fact.content);
        if !all_corrections.contains(&line) {
            all_corrections.push(line);
        }
    }
    if !all_corrections.is_empty() {
        sections.push(format!("## Corrections (MUST follow)\n{}", all_corrections.join("\n")));
    }

    // Skill recommendations
    let skill_facts: Vec<&ScoredFact> = facts.iter()
        .filter(|f| f.fact.category == "skill" || f.fact.content.contains("skill"))
        .collect();
    if !skill_facts.is_empty() {
        let items: Vec<String> = skill_facts.iter().map(|f| format!("- {}", f.fact.content)).collect();
        sections.push(format!("## Recommended Skills\n{}", items.join("\n")));
    }

    // Domain knowledge
    let domain: Vec<&ScoredFact> = facts.iter()
        .filter(|f| !["correction", "skill"].contains(&f.fact.category.as_str()))
        .collect();
    if !domain.is_empty() {
        let items: Vec<String> = domain.iter().map(|f| format!("- {}", f.fact.content)).collect();
        sections.push(format!("## Context\n{}", items.join("\n")));
    }

    // Graph: what files/entities exist in this ward
    if let (Some(ref graph), Some(ward)) = (&self.graph_service, ward_id) {
        if let Ok(entities) = graph.search_entities("", Some(ward), 10).await {
            let file_entities: Vec<&_> = entities.iter()
                .filter(|e| e.entity_type.as_str() == "file")
                .collect();
            if !file_entities.is_empty() {
                let items: Vec<String> = file_entities.iter()
                    .map(|e| format!("- {}", e.name))
                    .collect();
                sections.push(format!("## Files in Ward\n{}", items.join("\n")));
            }
        }
    }

    if sections.is_empty() {
        return Ok(String::new());
    }

    Ok(format!("<primed_context>\n{}\n</primed_context>", sections.join("\n\n")))
}
```

Note: The `search_memory_facts_fts` and `graph.search_entities` method signatures may differ from what's shown. Read the actual method signatures in `MemoryRepository` and `GraphService` and adjust accordingly. The key point is: (1) get corrections for this agent, (2) get domain facts for the task, (3) get file entities from the graph for the ward.

- [ ] **Step 2: Enhance subagent priming in spawn.rs**

In `gateway/gateway-execution/src/delegation/spawn.rs`, the memory recall already happens at lines 310-340. Enhance it to use the new `recall_for_delegation()`:

Replace the existing recall block (lines 310-340) with:

```rust
let initial_history = if let Some(recall) = &memory_recall {
    let ward_id = session_ward_id.as_deref();
    match recall.recall_for_delegation(
        &request.child_agent_id,
        &request.task,
        ward_id,
        8,  // more facts for richer priming
    ).await {
        Ok(context) if !context.is_empty() => {
            tracing::info!(
                agent = %request.child_agent_id,
                context_len = context.len(),
                "Primed subagent with recalled memory context"
            );
            vec![ChatMessage::system(context)]
        }
        Ok(_) => Vec::new(),
        Err(e) => {
            tracing::warn!(
                agent = %request.child_agent_id,
                error = %e,
                "Delegation recall failed, proceeding without priming"
            );
            Vec::new()
        }
    }
} else {
    Vec::new()
};
```

- [ ] **Step 3: Build and verify**

```bash
cargo build -p gateway-execution -p gateway 2>&1 | grep "^error"
```

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/gateway-execution/src/delegation/spawn.rs
git commit -m "feat: enhanced subagent priming — corrections, skills, ward files from memory"
```

---

### Task 5: Activate Graph-Powered Recall for First Message (Phase 2a — Loop 3)

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`

- [ ] **Step 1: Find where recall happens for first messages**

The first message recall currently happens differently from continuation recall. Search `runner.rs` for where the initial invoke builds the conversation history. The continuation recall (lines 1575-1604) already uses `recall_with_graph()`. The first message path may use a simpler `recall()` or may not recall at all.

Read `runner.rs` around the `invoke()` method to find the first-message path and ensure it uses `recall_with_graph()` with the same pattern as the continuation path.

- [ ] **Step 2: If first-message recall is missing or basic, add graph-powered recall**

In the first-message invoke path, add:

```rust
// Recall for first message (same as continuation pattern)
if let Some(recall) = &self.memory_recall {
    let user_message = message.as_str();
    match recall.recall_with_graph(
        &config.agent_id,
        user_message,
        5,
        None, // no ward yet on first message
        None, // no session yet
    ).await {
        Ok(result) if !result.facts.is_empty() || !result.episodes.is_empty() => {
            // Inject as first system message in history
            initial_messages.insert(0, ChatMessage::system(result.formatted));
            tracing::info!(
                facts = result.facts.len(),
                episodes = result.episodes.len(),
                "Recalled memory context for first message"
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("First-message recall failed: {}", e),
    }
}
```

- [ ] **Step 3: Build and verify**

```bash
cargo build -p gateway-execution -p gateway 2>&1 | grep "^error"
```

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs
git commit -m "feat: graph-powered recall on first message — episodes, entities, facts"
```

---

### Task 6: Predictive Recall Boost (Phase 2b — Loop 4)

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Add predictive boost step to `recall()` method**

In `gateway/gateway-execution/src/recall.rs`, after the ward affinity boost step (around line 219) and before temporal decay, add predictive recall:

```rust
// Predictive boost: facts that were recalled in similar past sessions
// get a score boost (they'll likely be needed again)
if self.config.predictive_recall.enabled {
    if let (Some(ref episode_repo), Some(ref recall_log_repo), Some(ref embedding_client)) =
        (&self.episode_repo, &self.recall_log_repo, &self.embedding_client)
    {
        // Find similar past sessions
        if let Ok(embeddings) = embedding_client.embed(&[user_message]).await {
            if let Some(query_emb) = embeddings.first() {
                if let Ok(similar_episodes) = episode_repo.search_by_similarity(
                    query_emb,
                    self.config.predictive_recall.min_similar_sessions,
                ).await {
                    // Get fact keys recalled in those sessions
                    let session_ids: Vec<String> = similar_episodes.iter()
                        .map(|ep| ep.session_id.clone())
                        .collect();

                    if let Ok(predicted_keys) = recall_log_repo.get_keys_for_sessions(&session_ids) {
                        let predicted_set: std::collections::HashSet<&str> = predicted_keys.iter()
                            .map(|k| k.as_str())
                            .collect();

                        // Boost facts that were recalled in similar sessions
                        for scored in &mut scored_facts {
                            if predicted_set.contains(scored.fact.key.as_str()) {
                                scored.score *= self.config.predictive_recall.boost;
                                tracing::debug!(
                                    key = %scored.fact.key,
                                    boost = self.config.predictive_recall.boost,
                                    "Predictive boost applied"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
```

Note: Check the actual field names on `MemoryRecall` — `self.episode_repo`, `self.recall_log_repo`, `self.embedding_client` may have different names or be nested differently. Read the struct definition at the top of `recall.rs`.

- [ ] **Step 2: Build and verify**

```bash
cargo build -p gateway-execution 2>&1 | grep "^error"
```

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs
git commit -m "feat: predictive recall — boost facts from similar past sessions"
```

---

### Task 7: Fact Verification in Distillation (Phase 4 — Accuracy)

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Add `verify_fact_confidence()` function**

In `gateway/gateway-execution/src/distillation.rs`, add before the main distillation function:

```rust
/// Verify a distilled fact against the session transcript's tool outputs.
/// Returns adjusted confidence:
/// - Grounded in tool output: keep original confidence
/// - Not grounded but plausible: confidence × 0.6
/// - Contradicts tool output: 0.0 (discard)
fn verify_fact_confidence(
    fact: &ExtractedFact,
    tool_outputs: &[String],
) -> f64 {
    let content_lower = fact.content.to_lowercase();

    // Extract key terms from the fact (words > 3 chars, not stopwords)
    let key_terms: Vec<&str> = fact.content.split_whitespace()
        .filter(|w| w.len() > 3)
        .filter(|w| !["that", "this", "with", "from", "have", "been", "were", "will", "should"].contains(w))
        .collect();

    if key_terms.is_empty() {
        return fact.confidence * 0.6;
    }

    // Check if key terms appear in tool outputs
    let mut matches = 0;
    for term in &key_terms {
        let term_lower = term.to_lowercase();
        for output in tool_outputs {
            if output.to_lowercase().contains(&term_lower) {
                matches += 1;
                break;
            }
        }
    }

    let match_ratio = matches as f64 / key_terms.len() as f64;

    if match_ratio >= 0.5 {
        // Well-grounded in tool outputs
        fact.confidence
    } else if match_ratio > 0.0 {
        // Partially grounded
        fact.confidence * 0.8
    } else {
        // Not grounded — reduce confidence
        fact.confidence * 0.6
    }
}
```

- [ ] **Step 2: Extract tool outputs from transcript**

In the distillation function, before fact storage, extract tool outputs from the transcript:

```rust
// Collect tool outputs from transcript for fact verification
let tool_outputs: Vec<String> = transcript.iter()
    .filter(|(role, _)| role == "tool")
    .map(|(_, content)| content.clone())
    .collect();
```

- [ ] **Step 3: Apply verification to each fact before storage**

In the fact storage loop (around line 240), apply verification:

```rust
let verified_confidence = verify_fact_confidence(&ef, &tool_outputs);
if verified_confidence < 0.1 {
    tracing::debug!(key = %ef.key, "Discarding ungrounded fact");
    continue; // Skip facts with near-zero confidence
}

let fact = MemoryFact {
    // ... existing fields ...
    confidence: verified_confidence, // Use verified confidence instead of ef.confidence
    // ...
};
```

- [ ] **Step 4: Build and verify**

```bash
cargo build -p gateway-execution 2>&1 | grep "^error"
```

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs
git commit -m "feat: fact verification — ground distilled facts against tool outputs"
```

---

### Task 8: Mid-Session Memory Injection (Phase 3 — Loop 5)

**Files:**
- Modify: `runtime/agent-runtime/src/executor.rs`

- [ ] **Step 1: Add mid-session recall hook to executor**

In `runtime/agent-runtime/src/executor.rs`, the main executor loop iterates through LLM calls. Find the iteration counter and add a recall check.

After each assistant turn (after tool execution, before the next LLM call), add:

```rust
// Mid-session recall: inject new relevant facts every N turns
if iteration > 0 && iteration % mid_session_interval == 0 {
    if let Some(ref recall_fn) = self.config.mid_session_recall_fn {
        // Collect recent message content for recall query
        let recent_content: String = self.messages.iter()
            .rev()
            .take(3)
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join(" ");

        if let Ok(context) = recall_fn(&recent_content).await {
            if !context.is_empty() {
                self.messages.push(ChatMessage::system(format!(
                    "[Memory Update] New relevant context:\n{}",
                    context
                )));
                tracing::debug!(iteration, "Injected mid-session memory recall");
            }
        }
    }
}
```

Note: The executor may not have direct access to `MemoryRecall`. The recall function needs to be passed in via `ExecutorConfig` as a callback or trait object. Check how `beforeToolCall` hooks are currently passed — use the same pattern.

The `mid_session_interval` should come from `RecallConfig.mid_session_recall.every_n_turns` (default: 5).

- [ ] **Step 2: Wire the callback from the runner**

In `gateway/gateway-execution/src/invoke/executor.rs` or wherever the executor is built, pass the recall callback:

```rust
// Only wire mid-session recall if configured
if recall_config.mid_session_recall.enabled {
    if let Some(recall) = &memory_recall {
        let recall_clone = recall.clone();
        let agent_id = agent_id.to_string();
        let ward_id = ward_id.map(String::from);

        executor_config.mid_session_recall_fn = Some(Box::new(move |query: &str| {
            let recall = recall_clone.clone();
            let agent = agent_id.clone();
            let ward = ward_id.clone();
            Box::pin(async move {
                recall.recall_for_delegation(&agent, query, ward.as_deref(), 3)
                    .await
                    .unwrap_or_default()
            })
        }));
    }
}
```

- [ ] **Step 3: Build and verify**

```bash
cargo build -p agent-runtime -p gateway-execution -p gateway 2>&1 | grep "^error"
```

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-runtime/src/executor.rs gateway/gateway-execution/src/invoke/executor.rs
git commit -m "feat: mid-session memory injection — recall new facts every N turns"
```

---

### Task 9: Integration Test — Memory Brain End-to-End

- [ ] **Step 1: Verify graph dedup works**

```bash
# Restart daemon to trigger migration
# Then check graph for duplicates
python3 -c "
import sqlite3
kg = sqlite3.connect('$HOME/Documents/zbot/data/knowledge_graph.db')
dupes = kg.execute('''
    SELECT COUNT(*) FROM (
        SELECT source_entity_id, target_entity_id, relationship_type, COUNT(*) as cnt
        FROM kg_relationships
        GROUP BY source_entity_id, target_entity_id, relationship_type
        HAVING cnt > 1
    )
''').fetchone()
print(f'Duplicate relationship groups: {dupes[0]}')
assert dupes[0] == 0, 'Duplicates still exist!'
print('✓ Graph dedup working')
"
```

- [ ] **Step 2: Test intent analysis with memory**

Send a message related to a previous session topic (e.g., stock analysis). Check daemon logs for:
```
"Recalled memory context for intent analysis"
```

Verify the intent analysis output includes ward/skill/agent recommendations informed by past sessions.

- [ ] **Step 3: Test subagent priming**

Trigger a delegation. Check daemon logs for:
```
"Primed subagent with recalled memory context"
```

Verify the subagent's first action is NOT reading AGENTS.md/ward.md (it should already have the context).

- [ ] **Step 4: Test predictive recall**

Run a session similar to a past one. Check daemon logs for:
```
"Predictive boost applied"
```

- [ ] **Step 5: Commit test results as documentation**

```bash
git commit --allow-empty -m "test: memory brain integration verified — dedup, intent, priming, prediction"
```
