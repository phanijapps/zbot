# Cognitive Memory System — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the knowledge graph a first-class citizen — give agents a graph query tool, enrich delegation with graph context, and add temporal fact tracking.

**Architecture:** Three sub-features: (1) `graph_query` tool registered for all agents, (2) `recall_for_delegation_with_graph()` that appends graph context to subagent startup, (3) temporal columns on `memory_facts` with supersession logic in distillation and scoring penalty in recall.

**Tech Stack:** Rust (gateway-execution, gateway-database, knowledge-graph, agent-tools), SQLite migrations, async Rust with tokio.

**Spec:** `docs/superpowers/specs/2026-04-11-cognitive-memory-system-design.md` — Sections 4.1, 4.2, 5.1, 5.2, 5.3

**Branch:** `feature/cognitive-memory-phase1`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| CREATE | `runtime/agent-tools/src/tools/graph_query.rs` | Graph query tool: search, neighbors, context actions |
| CREATE | `services/knowledge-graph/src/causal.rs` | Causal edge CRUD (kg_causal_edges table) |
| MODIFY | `runtime/agent-tools/src/tools/mod.rs` | Register GraphQueryTool export |
| MODIFY | `gateway/gateway-execution/src/invoke/executor.rs` | Add graph_query to tool registries |
| MODIFY | `gateway/gateway-database/src/schema.rs` | Migration v18: temporal columns + causal edges table |
| MODIFY | `services/knowledge-graph/src/storage.rs` | Add causal edge table creation to init |
| MODIFY | `services/knowledge-graph/src/lib.rs` | Export causal module |
| MODIFY | `gateway/gateway-execution/src/recall.rs` | `recall_for_delegation_with_graph()`, temporal scoring |
| MODIFY | `gateway/gateway-execution/src/delegation/spawn.rs` | Call new recall function |
| MODIFY | `gateway/gateway-execution/src/distillation.rs` | Populate temporal columns, extract causal edges |

---

### Task 1: Database Migration — Temporal Columns + Causal Edges Table

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs`

- [ ] **Step 1: Add migration for temporal columns and causal edges**

In `gateway/gateway-database/src/schema.rs`, increment `SCHEMA_VERSION` and add migration block:

```rust
// Change line ~9:
const SCHEMA_VERSION: i32 = 18;
```

Add inside `migrate_database()`, after the last `if version < 17` block:

```rust
if version < 18 {
    // Temporal columns on memory_facts
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN valid_from TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN valid_until TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE memory_facts ADD COLUMN superseded_by TEXT",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_facts_temporal ON memory_facts(valid_from, valid_until)",
        [],
    );

    // Causal edges table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kg_causal_edges (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            cause_entity_id TEXT NOT NULL,
            effect_entity_id TEXT NOT NULL,
            relationship TEXT NOT NULL,
            confidence REAL DEFAULT 0.7,
            session_id TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (cause_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
            FOREIGN KEY (effect_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_causal_cause ON kg_causal_edges(cause_entity_id);
        CREATE INDEX IF NOT EXISTS idx_causal_effect ON kg_causal_edges(effect_entity_id);",
    )?;
}
```

- [ ] **Step 2: Verify migration compiles**

Run: `cargo check --package gateway-database`
Expected: Clean compilation.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-database/src/schema.rs
git commit -m "feat(db): migration v18 — temporal fact columns + causal edges table"
```

---

### Task 2: Causal Edge CRUD in Knowledge Graph Service

**Files:**
- Create: `services/knowledge-graph/src/causal.rs`
- Modify: `services/knowledge-graph/src/lib.rs`
- Modify: `services/knowledge-graph/src/storage.rs`

- [ ] **Step 1: Write tests for causal edge operations**

Create `services/knowledge-graph/src/causal.rs`:

```rust
use rusqlite::params;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A causal relationship between two knowledge graph entities.
#[derive(Debug, Clone)]
pub struct CausalEdge {
    pub id: String,
    pub agent_id: String,
    pub cause_entity_id: String,
    pub effect_entity_id: String,
    pub relationship: String,
    pub confidence: f64,
    pub session_id: Option<String>,
    pub created_at: String,
}

/// CRUD operations for causal edges in the knowledge graph.
pub struct CausalEdgeStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl CausalEdgeStore {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    /// Store a causal edge. Skips if duplicate (same cause, effect, relationship).
    pub async fn store_edge(&self, edge: &CausalEdge) -> Result<(), String> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT OR IGNORE INTO kg_causal_edges \
             (id, agent_id, cause_entity_id, effect_entity_id, relationship, confidence, session_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                edge.id,
                edge.agent_id,
                edge.cause_entity_id,
                edge.effect_entity_id,
                edge.relationship,
                edge.confidence,
                edge.session_id,
                edge.created_at,
            ],
        )
        .map_err(|e| format!("Failed to store causal edge: {e}"))?;
        Ok(())
    }

    /// Get causal edges where the given entity is the cause.
    pub async fn get_effects(&self, entity_id: &str) -> Result<Vec<CausalEdge>, String> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, cause_entity_id, effect_entity_id, relationship, confidence, session_id, created_at \
                 FROM kg_causal_edges WHERE cause_entity_id = ?1",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let edges = stmt
            .query_map(params![entity_id], |row| {
                Ok(CausalEdge {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    cause_entity_id: row.get(2)?,
                    effect_entity_id: row.get(3)?,
                    relationship: row.get(4)?,
                    confidence: row.get(5)?,
                    session_id: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| format!("Failed to query causal edges: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect causal edges: {e}"))?;

        Ok(edges)
    }

    /// Get causal edges where the given entity is the effect.
    pub async fn get_causes(&self, entity_id: &str) -> Result<Vec<CausalEdge>, String> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                "SELECT id, agent_id, cause_entity_id, effect_entity_id, relationship, confidence, session_id, created_at \
                 FROM kg_causal_edges WHERE effect_entity_id = ?1",
            )
            .map_err(|e| format!("Failed to prepare query: {e}"))?;

        let edges = stmt
            .query_map(params![entity_id], |row| {
                Ok(CausalEdge {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    cause_entity_id: row.get(2)?,
                    effect_entity_id: row.get(3)?,
                    relationship: row.get(4)?,
                    confidence: row.get(5)?,
                    session_id: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })
            .map_err(|e| format!("Failed to query causal edges: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect causal edges: {e}"))?;

        Ok(edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    async fn setup_test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE kg_entities (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                name TEXT NOT NULL,
                properties TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                mention_count INTEGER DEFAULT 1
            );
            CREATE TABLE kg_causal_edges (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                cause_entity_id TEXT NOT NULL,
                effect_entity_id TEXT NOT NULL,
                relationship TEXT NOT NULL,
                confidence REAL DEFAULT 0.7,
                session_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (cause_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
                FOREIGN KEY (effect_entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
            );
            INSERT INTO kg_entities VALUES ('e1', 'root', 'pattern', 'rate_limiting', NULL, '2026-01-01', '2026-01-01', 1);
            INSERT INTO kg_entities VALUES ('e2', 'root', 'outcome', 'api_ban', NULL, '2026-01-01', '2026-01-01', 1);",
        )
        .unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[tokio::test]
    async fn test_store_and_get_effects() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let edge = CausalEdge {
            id: "ce1".to_string(),
            agent_id: "root".to_string(),
            cause_entity_id: "e1".to_string(),
            effect_entity_id: "e2".to_string(),
            relationship: "prevents".to_string(),
            confidence: 0.9,
            session_id: Some("sess-1".to_string()),
            created_at: "2026-04-11".to_string(),
        };

        store.store_edge(&edge).await.unwrap();
        let effects = store.get_effects("e1").await.unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].relationship, "prevents");
        assert_eq!(effects[0].effect_entity_id, "e2");
    }

    #[tokio::test]
    async fn test_get_causes() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let edge = CausalEdge {
            id: "ce1".to_string(),
            agent_id: "root".to_string(),
            cause_entity_id: "e1".to_string(),
            effect_entity_id: "e2".to_string(),
            relationship: "prevents".to_string(),
            confidence: 0.9,
            session_id: None,
            created_at: "2026-04-11".to_string(),
        };

        store.store_edge(&edge).await.unwrap();
        let causes = store.get_causes("e2").await.unwrap();
        assert_eq!(causes.len(), 1);
        assert_eq!(causes[0].cause_entity_id, "e1");
    }

    #[tokio::test]
    async fn test_duplicate_edge_ignored() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let edge = CausalEdge {
            id: "ce1".to_string(),
            agent_id: "root".to_string(),
            cause_entity_id: "e1".to_string(),
            effect_entity_id: "e2".to_string(),
            relationship: "prevents".to_string(),
            confidence: 0.9,
            session_id: None,
            created_at: "2026-04-11".to_string(),
        };

        store.store_edge(&edge).await.unwrap();
        // Duplicate insert should not fail
        store.store_edge(&edge).await.unwrap();
        let effects = store.get_effects("e1").await.unwrap();
        assert_eq!(effects.len(), 1);
    }

    #[tokio::test]
    async fn test_no_edges_returns_empty() {
        let conn = setup_test_db().await;
        let store = CausalEdgeStore::new(conn);

        let effects = store.get_effects("nonexistent").await.unwrap();
        assert!(effects.is_empty());
    }
}
```

- [ ] **Step 2: Export causal module**

In `services/knowledge-graph/src/lib.rs`, add:

```rust
pub mod causal;
pub use causal::{CausalEdge, CausalEdgeStore};
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package knowledge-graph -- causal`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add services/knowledge-graph/src/causal.rs services/knowledge-graph/src/lib.rs
git commit -m "feat(knowledge-graph): causal edge store with CRUD and tests"
```

---

### Task 3: Graph Query Tool

**Files:**
- Create: `runtime/agent-tools/src/tools/graph_query.rs`
- Modify: `runtime/agent-tools/src/tools/mod.rs`

- [ ] **Step 1: Create the graph query tool**

Create `runtime/agent-tools/src/tools/graph_query.rs`:

```rust
//! Graph query tool — lets agents search entities, explore neighbors,
//! and get contextual subgraphs from the knowledge graph.

use agent_runtime::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

/// Key for retrieving GraphStorage from ToolContext state.
pub const GRAPH_STORAGE_KEY: &str = "graph_storage";

pub struct GraphQueryTool;

impl GraphQueryTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GraphQueryTool {
    fn name(&self) -> &str {
        "graph_query"
    }

    fn description(&self) -> &str {
        "Query the knowledge graph to explore entities and their relationships. \
         Actions: search (find entities by name/type), neighbors (get connected entities), \
         context (get everything relevant to a topic)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "neighbors", "context"],
                    "description": "search: find entities by name/type. neighbors: get connected entities. context: get relevant subgraph."
                },
                "query": {
                    "type": "string",
                    "description": "Search query or entity name"
                },
                "entity_type": {
                    "type": "string",
                    "description": "Filter by entity type (optional)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["outgoing", "incoming", "both"],
                    "description": "Direction for neighbor traversal (default: both)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Traversal depth for neighbors (default: 1, max: 2)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default: 10, max: 25)"
                }
            },
            "required": ["action", "query"]
        })
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> ToolResult {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("search");
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::error("Missing required parameter: query"),
        };
        let entity_type = args.get("entity_type").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(25) as usize;

        // Get graph storage from context state
        let graph_storage = match ctx.state().get::<Arc<dyn GraphStorageAccess>>(GRAPH_STORAGE_KEY) {
            Some(gs) => gs.clone(),
            None => return ToolResult::error("Knowledge graph not available"),
        };

        match action {
            "search" => execute_search(graph_storage.as_ref(), query, entity_type, limit).await,
            "neighbors" => {
                let depth = args
                    .get("depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .min(2) as usize;
                let direction = args
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("both");
                execute_neighbors(graph_storage.as_ref(), query, direction, depth, limit).await
            }
            "context" => execute_context(graph_storage.as_ref(), query, limit).await,
            other => ToolResult::error(&format!("Unknown action: {other}. Use search, neighbors, or context.")),
        }
    }
}

/// Trait for graph storage access from tool context.
/// Implemented by the actual GraphStorage to avoid tight coupling.
#[async_trait]
pub trait GraphStorageAccess: Send + Sync {
    async fn search_entities_by_name(&self, query: &str, entity_type: Option<&str>, limit: usize) -> Result<Vec<EntityInfo>, String>;
    async fn get_entity_neighbors(&self, entity_name: &str, direction: &str, limit: usize) -> Result<Vec<NeighborInfo>, String>;
    async fn get_entity_by_name(&self, name: &str) -> Result<Option<EntityInfo>, String>;
}

#[derive(Debug, Clone)]
pub struct EntityInfo {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub mention_count: i64,
}

#[derive(Debug, Clone)]
pub struct NeighborInfo {
    pub entity: EntityInfo,
    pub relationship_type: String,
    pub direction: String,
}

async fn execute_search(
    gs: &dyn GraphStorageAccess,
    query: &str,
    entity_type: Option<&str>,
    limit: usize,
) -> ToolResult {
    match gs.search_entities_by_name(query, entity_type, limit).await {
        Ok(entities) if entities.is_empty() => {
            ToolResult::success(&format!("No entities found matching \"{query}\"."))
        }
        Ok(entities) => {
            let mut output = format!("## Entities matching \"{query}\"\n\n");
            for e in &entities {
                output.push_str(&format!(
                    "- **{}** ({}): mentions={}\n",
                    e.name, e.entity_type, e.mention_count
                ));
            }
            ToolResult::success(&output)
        }
        Err(e) => ToolResult::error(&format!("Graph search failed: {e}")),
    }
}

async fn execute_neighbors(
    gs: &dyn GraphStorageAccess,
    entity_name: &str,
    direction: &str,
    depth: usize,
    limit: usize,
) -> ToolResult {
    // First verify entity exists
    match gs.get_entity_by_name(entity_name).await {
        Ok(None) => {
            return ToolResult::success(&format!(
                "Entity \"{entity_name}\" not found in knowledge graph."
            ));
        }
        Err(e) => return ToolResult::error(&format!("Graph lookup failed: {e}")),
        Ok(Some(_)) => {}
    }

    match gs.get_entity_neighbors(entity_name, direction, limit).await {
        Ok(neighbors) if neighbors.is_empty() => {
            ToolResult::success(&format!(
                "Entity \"{entity_name}\" exists but has no {direction} connections (depth={depth})."
            ))
        }
        Ok(neighbors) => {
            let mut output = format!("## Neighbors of \"{entity_name}\" ({direction}, depth={depth})\n\n");
            for n in &neighbors {
                let arrow = match n.direction.as_str() {
                    "outgoing" => "-->",
                    "incoming" => "<--",
                    _ => "---",
                };
                output.push_str(&format!(
                    "- {entity_name} {arrow} **{}** ({}) via `{}`\n",
                    n.entity.name, n.entity.entity_type, n.relationship_type
                ));
            }
            ToolResult::success(&output)
        }
        Err(e) => ToolResult::error(&format!("Neighbor query failed: {e}")),
    }
}

async fn execute_context(
    gs: &dyn GraphStorageAccess,
    topic: &str,
    limit: usize,
) -> ToolResult {
    // Search for entities related to the topic
    let entities = match gs.search_entities_by_name(topic, None, 5).await {
        Ok(e) => e,
        Err(e) => return ToolResult::error(&format!("Context search failed: {e}")),
    };

    if entities.is_empty() {
        return ToolResult::success(&format!(
            "No knowledge graph context found for \"{topic}\"."
        ));
    }

    let mut output = format!("## Knowledge Graph Context: \"{topic}\"\n\n### Entities\n");
    for e in &entities {
        output.push_str(&format!(
            "- **{}** ({}): mentions={}\n",
            e.name, e.entity_type, e.mention_count
        ));
    }

    // Get neighbors for the top entity
    output.push_str("\n### Relationships\n");
    let per_entity_limit = limit / entities.len().max(1);
    for e in entities.iter().take(3) {
        if let Ok(neighbors) = gs.get_entity_neighbors(&e.name, "both", per_entity_limit).await {
            for n in &neighbors {
                let arrow = if n.direction == "outgoing" { "-->" } else { "<--" };
                output.push_str(&format!(
                    "- {} {} {} ({})\n",
                    e.name, arrow, n.entity.name, n.relationship_type
                ));
            }
        }
    }

    ToolResult::success(&output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockGraphStorage {
        entities: Vec<EntityInfo>,
        neighbors: Vec<NeighborInfo>,
    }

    #[async_trait]
    impl GraphStorageAccess for MockGraphStorage {
        async fn search_entities_by_name(&self, query: &str, _entity_type: Option<&str>, _limit: usize) -> Result<Vec<EntityInfo>, String> {
            Ok(self.entities.iter().filter(|e| e.name.contains(query)).cloned().collect())
        }
        async fn get_entity_neighbors(&self, _name: &str, _direction: &str, _limit: usize) -> Result<Vec<NeighborInfo>, String> {
            Ok(self.neighbors.clone())
        }
        async fn get_entity_by_name(&self, name: &str) -> Result<Option<EntityInfo>, String> {
            Ok(self.entities.iter().find(|e| e.name == name).cloned())
        }
    }

    fn test_storage() -> MockGraphStorage {
        MockGraphStorage {
            entities: vec![
                EntityInfo { id: "e1".into(), name: "yfinance".into(), entity_type: "module".into(), mention_count: 5 },
                EntityInfo { id: "e2".into(), name: "rate_limiting".into(), entity_type: "pattern".into(), mention_count: 3 },
            ],
            neighbors: vec![
                NeighborInfo {
                    entity: EntityInfo { id: "e2".into(), name: "rate_limiting".into(), entity_type: "pattern".into(), mention_count: 3 },
                    relationship_type: "requires".into(),
                    direction: "outgoing".into(),
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_search_finds_entities() {
        let gs = test_storage();
        let result = execute_search(&gs, "yfinance", None, 10).await;
        let content = result.output().unwrap_or_default();
        assert!(content.contains("yfinance"));
        assert!(content.contains("module"));
    }

    #[tokio::test]
    async fn test_search_empty() {
        let gs = test_storage();
        let result = execute_search(&gs, "nonexistent", None, 10).await;
        let content = result.output().unwrap_or_default();
        assert!(content.contains("No entities found"));
    }

    #[tokio::test]
    async fn test_neighbors_shows_connections() {
        let gs = test_storage();
        let result = execute_neighbors(&gs, "yfinance", "both", 1, 10).await;
        let content = result.output().unwrap_or_default();
        assert!(content.contains("rate_limiting"));
        assert!(content.contains("requires"));
    }

    #[tokio::test]
    async fn test_neighbors_entity_not_found() {
        let gs = test_storage();
        let result = execute_neighbors(&gs, "nonexistent", "both", 1, 10).await;
        let content = result.output().unwrap_or_default();
        assert!(content.contains("not found"));
    }

    #[tokio::test]
    async fn test_context_returns_entities_and_relationships() {
        let gs = test_storage();
        let result = execute_context(&gs, "yfinance", 10).await;
        let content = result.output().unwrap_or_default();
        assert!(content.contains("Entities"));
        assert!(content.contains("Relationships"));
        assert!(content.contains("yfinance"));
    }
}
```

- [ ] **Step 2: Register tool in mod.rs**

In `runtime/agent-tools/src/tools/mod.rs`, add:

```rust
mod graph_query;
pub use graph_query::{GraphQueryTool, GraphStorageAccess, EntityInfo, NeighborInfo, GRAPH_STORAGE_KEY};
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package agent-tools -- graph_query`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add runtime/agent-tools/src/tools/graph_query.rs runtime/agent-tools/src/tools/mod.rs
git commit -m "feat(tools): graph_query tool — search, neighbors, context actions"
```

---

### Task 4: Register Graph Query Tool in Executor

**Files:**
- Modify: `gateway/gateway-execution/src/invoke/executor.rs`

- [ ] **Step 1: Add GraphQueryTool to both root and delegated tool registries**

In `gateway/gateway-execution/src/invoke/executor.rs`:

Add to imports (~line 10):
```rust
use agent_tools::{GraphQueryTool, GRAPH_STORAGE_KEY};
```

In the tool registration block for delegated agents (~line 496-511), add after the existing tool list:
```rust
Arc::new(GraphQueryTool::new()),
```

In the tool registration block for root orchestrator (~line 514-547), add after the existing tool list:
```rust
Arc::new(GraphQueryTool::new()),
```

In the executor build section where tool context state is populated, add graph storage to the state if available:
```rust
// After existing state setup (search for where MemoryFactStore is set)
if let Some(ref graph_storage) = self.graph_storage {
    builder = builder.with_state_value(GRAPH_STORAGE_KEY, graph_storage.clone());
}
```

Note: The `ExecutorBuilder` needs a `graph_storage` field. Check if it already has one — if not, add `graph_storage: Option<Arc<dyn GraphStorageAccess>>` to the builder and thread it through from the runner. Follow the same pattern used for `fact_store`.

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package gateway-execution`
Expected: Clean compilation. If there are type mismatches with `GraphStorageAccess`, implement the trait on the actual `GraphStorage` struct from `knowledge-graph` crate.

- [ ] **Step 3: Implement `GraphStorageAccess` trait on actual GraphStorage**

If needed, add an adapter in `gateway/gateway-execution/src/invoke/executor.rs` or a new small file. The adapter wraps `knowledge_graph::GraphStorage` and implements `agent_tools::GraphStorageAccess`:

```rust
use agent_tools::{GraphStorageAccess, EntityInfo, NeighborInfo};
use knowledge_graph::GraphStorage;

#[async_trait]
impl GraphStorageAccess for GraphStorageAdapter {
    async fn search_entities_by_name(&self, query: &str, entity_type: Option<&str>, limit: usize) -> Result<Vec<EntityInfo>, String> {
        let entities = self.storage.search_entities(query, entity_type, limit).await?;
        Ok(entities.into_iter().map(|e| EntityInfo {
            id: e.id,
            name: e.name,
            entity_type: e.entity_type,
            mention_count: e.mention_count,
        }).collect())
    }
    // ... map other methods similarly
}
```

- [ ] **Step 4: Run workspace check**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/invoke/executor.rs
git commit -m "feat(executor): register graph_query tool for root and delegated agents"
```

---

### Task 5: Temporal Scoring in Recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`

- [ ] **Step 1: Add temporal penalty to fact scoring**

In `gateway/gateway-execution/src/recall.rs`, find the scoring section where `ScoredFact` scores are computed (look for where `confidence` or `score` is calculated in the hybrid search results).

Add after existing score calculation:

```rust
// Superseded facts get a 0.3x penalty — still retrievable for history but current facts preferred
if fact.valid_until.is_some() {
    score *= 0.3;
}
```

This requires the `memory_facts` query to include the `valid_until` column. Find the SQL query that fetches facts and add `valid_until` to the SELECT. The `MemoryFact` struct in `gateway-database/src/memory_repository.rs` needs the `valid_until` field added (if not already present).

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package gateway-execution`
Expected: Clean. If `MemoryFact` struct needs updating, modify `gateway-database/src/memory_repository.rs` to include `valid_from`, `valid_until`, `superseded_by` fields (all `Option<String>`).

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/gateway-database/src/memory_repository.rs
git commit -m "feat(recall): temporal scoring — superseded facts get 0.3x penalty"
```

---

### Task 6: Graph-Enriched Delegation Recall

**Files:**
- Modify: `gateway/gateway-execution/src/recall.rs`
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs`

- [ ] **Step 1: Add `recall_for_delegation_with_graph` function**

In `gateway/gateway-execution/src/recall.rs`, add a new public function after the existing `recall_for_delegation`:

```rust
/// Enhanced delegation recall that includes knowledge graph context.
/// Falls back to plain recall_for_delegation if graph is unavailable.
pub async fn recall_for_delegation_with_graph(
    &self,
    agent_id: &str,
    task: &str,
    ward_id: Option<&str>,
    limit: usize,
    graph_storage: Option<&knowledge_graph::GraphStorage>,
) -> Result<String, String> {
    // Start with existing recall
    let base_context = self.recall_for_delegation(agent_id, task, ward_id, limit).await?;

    // If no graph storage, return base context
    let graph = match graph_storage {
        Some(g) => g,
        None => return Ok(base_context),
    };

    // Extract potential entity names from task text
    let entity_names = extract_entity_candidates(task);
    if entity_names.is_empty() {
        return Ok(base_context);
    }

    // Look up entities and their neighbors
    let mut graph_lines: Vec<String> = Vec::new();
    let mut graph_tokens = 0usize;
    let graph_budget = 500; // ~500 tokens for graph context

    for name in entity_names.iter().take(5) {
        if graph_tokens >= graph_budget {
            break;
        }
        if let Ok(Some(entity)) = graph.get_entity_by_name(name).await {
            if let Ok(neighbors) = graph.get_neighbors(&entity.id, "both", 5).await {
                for neighbor in &neighbors {
                    let line = format!(
                        "- {} --{}-- {} ({})",
                        entity.name,
                        neighbor.relationship_type,
                        neighbor.entity.name,
                        neighbor.entity.entity_type,
                    );
                    graph_tokens += line.len() / 4;
                    graph_lines.push(line);
                }
            }
        }
    }

    if graph_lines.is_empty() {
        return Ok(base_context);
    }

    Ok(format!(
        "{base_context}\n\n## Related Knowledge Graph Context\n{}",
        graph_lines.join("\n")
    ))
}

/// Extract candidate entity names from text using simple heuristics.
/// Finds: PascalCase words, "quoted strings", ALLCAPS acronyms (3+ chars).
fn extract_entity_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    // Quoted strings
    for cap in regex::Regex::new(r#""([^"]+)""#).unwrap().captures_iter(text) {
        if let Some(m) = cap.get(1) {
            candidates.push(m.as_str().to_string());
        }
    }

    // PascalCase or multi-word capitalized
    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if clean.len() >= 3 {
            // ALLCAPS (3+ chars)
            if clean.chars().all(|c| c.is_uppercase() || c == '_') {
                candidates.push(clean.to_lowercase());
            }
            // PascalCase
            else if clean.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && clean.contains(|c: char| c.is_lowercase())
            {
                candidates.push(clean.to_string());
            }
        }
    }

    candidates.dedup();
    candidates
}
```

- [ ] **Step 2: Add unit tests for entity extraction**

Add to the existing `#[cfg(test)]` module in `recall.rs`:

```rust
#[test]
fn test_extract_entity_candidates_quoted() {
    let names = extract_entity_candidates(r#"Research "West Bengal" elections"#);
    assert!(names.contains(&"West Bengal".to_string()));
}

#[test]
fn test_extract_entity_candidates_allcaps() {
    let names = extract_entity_candidates("Analyze SPY and AAPL stocks");
    assert!(names.contains(&"spy".to_string()));
    assert!(names.contains(&"aapl".to_string()));
}

#[test]
fn test_extract_entity_candidates_pascal() {
    let names = extract_entity_candidates("Use MultiIndex from pandas DataFrame");
    assert!(names.contains(&"MultiIndex".to_string()));
    assert!(names.contains(&"DataFrame".to_string()));
}

#[test]
fn test_extract_entity_candidates_empty() {
    let names = extract_entity_candidates("do something simple");
    assert!(names.is_empty());
}
```

- [ ] **Step 3: Update delegation spawn to use new recall**

In `gateway/gateway-execution/src/delegation/spawn.rs`, find the `recall_for_delegation` call (~line 320) and replace with:

```rust
// Replace:
// recall.recall_for_delegation(&child_agent_id, &task, ward_id, 8)
// With:
recall.recall_for_delegation_with_graph(
    &child_agent_id,
    &task,
    ward_id,
    8,
    graph_storage.as_deref(),
)
```

Thread `graph_storage` through from the runner's `self.graph_storage` field to the delegation spawn context.

- [ ] **Step 4: Run tests**

Run: `cargo test --package gateway-execution -- extract_entity`
Expected: 4 tests pass.

Run: `cargo check --workspace`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-execution/src/recall.rs gateway/gateway-execution/src/delegation/spawn.rs
git commit -m "feat(recall): graph-enriched delegation with entity extraction"
```

---

### Task 7: Temporal Fact Supersession in Distillation

**Files:**
- Modify: `gateway/gateway-execution/src/distillation.rs`

- [ ] **Step 1: Update fact upsert to handle temporal supersession**

In `gateway/gateway-execution/src/distillation.rs`, find the fact upsert loop (where extracted facts are saved to `memory_repo`).

Before the existing upsert call, add supersession logic:

```rust
// Set valid_from on new facts
fact.valid_from = Some(session_started_at.clone());

// Check for existing fact with same key
if let Ok(Some(existing)) = memory_repo.get_fact_by_key(
    &fact.agent_id,
    &fact.scope,
    &fact.ward_id,
    &fact.key,
) {
    // If content differs, supersede the old fact
    if existing.content != fact.content {
        let _ = memory_repo.supersede_fact(
            &existing.id,
            &fact.id,
        );
        tracing::debug!(
            key = %fact.key,
            "Superseded old fact with new version"
        );
    }
}
```

- [ ] **Step 2: Add `supersede_fact` to memory repository**

In `gateway/gateway-database/src/memory_repository.rs`, add:

```rust
/// Mark a fact as superseded by a newer fact.
pub fn supersede_fact(&self, old_id: &str, new_id: &str) -> Result<(), String> {
    self.db.with_connection(|conn| {
        conn.execute(
            "UPDATE memory_facts SET valid_until = datetime('now'), superseded_by = ?1 WHERE id = ?2",
            params![new_id, old_id],
        )
        .map_err(|e| format!("Failed to supersede fact: {e}"))?;
        Ok(())
    })
}

/// Get a fact by its unique key (agent_id, scope, ward_id, key).
pub fn get_fact_by_key(
    &self,
    agent_id: &str,
    scope: &str,
    ward_id: &str,
    key: &str,
) -> Result<Option<MemoryFact>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT * FROM memory_facts \
                 WHERE agent_id = ?1 AND scope = ?2 AND ward_id = ?3 AND key = ?4 \
                 AND valid_until IS NULL \
                 LIMIT 1",
            )
            .map_err(|e| format!("Failed to prepare: {e}"))?;

        let fact = stmt
            .query_row(params![agent_id, scope, ward_id, key], |row| {
                // Map row to MemoryFact — follow existing pattern in this file
                Ok(self.row_to_fact(row)?)
            })
            .optional()
            .map_err(|e| format!("Failed to query: {e}"))?;

        Ok(fact)
    })
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace`
Expected: Clean.

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-execution/src/distillation.rs gateway/gateway-database/src/memory_repository.rs
git commit -m "feat(distillation): temporal fact supersession — old facts get valid_until on conflict"
```

---

### Task 8: Final Checks — fmt, clippy, tests

- [ ] **Step 1: Format all Rust code**

Run: `cargo fmt --all`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: Clean (0 warnings).

- [ ] **Step 3: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass, including new ones from Tasks 2-6.

- [ ] **Step 4: Run UI checks**

Run: `cd apps/ui && npm run build && npm run lint`
Expected: Clean (no UI changes in Phase 1, but verify no regressions).

- [ ] **Step 5: Final commit if any formatting changes**

```bash
git add -A
git commit -m "chore: cargo fmt, clippy clean"
```

- [ ] **Step 6: Push branch**

```bash
git push -u origin feature/cognitive-memory-phase1
```
