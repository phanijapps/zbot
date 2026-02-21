# Plan: Graph Query API for Knowledge Graph

**Status**: Planned
**Priority**: High
**Depends On**: Existing `kg_entities`, `kg_relationships` tables

---

## Problem

The knowledge graph stores entities and relationships but has **no query capabilities**:
- Cannot query "neighbors of entity X"
- Cannot find "path from A to B"
- Cannot traverse relationships
- Cannot use graph data in memory recall

**Current state**: Write-only storage (entities/relationships are stored but never queried)

---

## Solution

Add a **Graph Query API** that supports:
1. **Neighbor queries** - Get entities connected to a target
2. **Pathfinding** - Find paths between entities
3. **Pattern matching** - Match relationship patterns
4. **Integration with recall** - Use graph context in memory retrieval

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Graph Query Layer                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ Neighbors   │  │ Pathfinder  │  │ Pattern Matcher         │ │
│  │ Query       │  │ (BFS/DFS)   │  │ (Cypher-like subset)    │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                    Graph Repository                             │
│  - load_neighbors(entity_id)                                    │
│  - load_relationships(entity_ids)                               │
│  - breadth_first_search(source, target, max_depth)             │
│  - find_paths(source, target, algorithm)                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SQLite Storage                              │
│  kg_entities ────────── kg_relationships                        │
│  (id, name, type)       (source_id, target_id, type)            │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Graph Repository

### 1.1 Data Structures

**File:** `services/knowledge-graph/src/graph.rs`

```rust
/// Graph node representation
pub struct GraphNode {
    pub entity: Entity,
    pub incoming: Vec<Relationship>,
    pub outgoing: Vec<Relationship>,
}

/// Path between entities
pub struct GraphPath {
    pub nodes: Vec<Entity>,
    pub edges: Vec<Relationship>,
    pub total_weight: f64,
}

/// Neighbor query options
pub struct NeighborQuery {
    pub entity_id: String,
    pub direction: Direction,      // Incoming, Outgoing, Both
    pub relationship_types: Option<Vec<RelationshipType>>,
    pub entity_types: Option<Vec<EntityType>>,
    pub limit: Option<usize>,
}

/// Path query options
pub struct PathQuery {
    pub source_id: String,
    pub target_id: String,
    pub max_depth: usize,
    pub algorithm: PathAlgorithm,  // BFS, Dijkstra
    pub relationship_filter: Option<Vec<RelationshipType>>,
}

pub enum Direction {
    Incoming,  // Edges pointing TO entity
    Outgoing,  // Edges pointing FROM entity
    Both,
}

pub enum PathAlgorithm {
    BFS,       // Shortest path (unweighted)
    Dijkstra,  // Shortest path (weighted by mention_count)
}
```

### 1.2 Repository Methods

**File:** `services/knowledge-graph/src/repository.rs`

```rust
impl KnowledgeGraphRepository {
    /// Get neighbors of an entity
    pub fn get_neighbors(&self, query: NeighborQuery) -> Result<Vec<GraphNode>>;

    /// Find all paths between two entities (up to max_depth)
    pub fn find_paths(&self, query: PathQuery) -> Result<Vec<GraphPath>>;

    /// Get subgraph around an entity (all entities within N hops)
    pub fn get_subgraph(&self, entity_id: &str, depth: usize) -> Result<Subgraph>;

    /// Check if path exists between entities
    pub fn path_exists(&self, source: &str, target: &str, max_depth: usize) -> Result<bool>;

    /// Get entity degree (number of connections)
    pub fn get_degree(&self, entity_id: &str) -> Result<EntityDegree>;
}

pub struct Subgraph {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
    pub center_entity: String,
    pub depth: usize,
}

pub struct EntityDegree {
    pub entity_id: String,
    pub incoming: usize,
    pub outgoing: usize,
    pub total: usize,
}
```

### 1.3 SQL Queries

```sql
-- Get outgoing neighbors
SELECT e.*, r.*
FROM kg_entities e
JOIN kg_relationships r ON r.target_entity_id = e.id
WHERE r.source_entity_id = ? AND r.agent_id = ?
ORDER BY r.mention_count DESC
LIMIT ?;

-- Get incoming neighbors
SELECT e.*, r.*
FROM kg_entities e
JOIN kg_relationships r ON r.source_entity_id = e.id
WHERE r.target_entity_id = ? AND r.agent_id = ?
ORDER BY r.mention_count DESC
LIMIT ?;

-- Get all relationships for subgraph
SELECT * FROM kg_relationships
WHERE agent_id = ?
AND (source_entity_id IN (?) OR target_entity_id IN (?));
```

---

## Phase 2: Graph Service

### 2.1 Service Layer

**File:** `services/knowledge-graph/src/service.rs`

```rust
pub struct GraphService {
    repo: Arc<KnowledgeGraphRepository>,
}

impl GraphService {
    /// Get entities related to a target (1-hop neighbors)
    pub async fn get_related_entities(
        &self,
        agent_id: &str,
        entity_name: &str,
        options: NeighborOptions,
    ) -> Result<Vec<RelatedEntity>>;

    /// Find connection path between two entities
    pub async fn find_connection(
        &self,
        agent_id: &str,
        source_name: &str,
        target_name: &str,
        max_depth: usize,
    ) -> Result<Option<ConnectionPath>>;

    /// Get entity context for memory recall
    pub async fn get_entity_context(
        &self,
        agent_id: &str,
        entity_names: &[&str],
        hops: usize,
    ) -> Result<EntityContext>;
}

pub struct RelatedEntity {
    pub entity: Entity,
    pub relationship: Relationship,
    pub distance: usize,
}

pub struct ConnectionPath {
    pub source: Entity,
    pub target: Entity,
    pub path: Vec<PathStep>,
    pub length: usize,
}

pub struct PathStep {
    pub from_entity: Entity,
    pub relationship: Relationship,
    pub to_entity: Entity,
}

pub struct EntityContext {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
    pub summary: String,  // Natural language summary
}
```

---

## Phase 3: Integration with Memory Recall

### 3.1 Hybrid Memory + Graph Recall

**File:** `gateway/gateway-execution/src/recall.rs`

```rust
/// Enhanced recall that includes graph context
pub async fn recall_with_graph_context(
    agent_id: &str,
    query: &str,
    options: RecallOptions,
) -> Result<RecallResult> {
    // 1. Standard hybrid search (facts)
    let facts = search_memory_facts_hybrid(query, agent_id, options.limit)?;

    // 2. Extract entity names from query
    let entity_names = extract_entity_names(query);

    // 3. Get graph context for mentioned entities
    let graph_context = graph_service.get_entity_context(
        agent_id,
        &entity_names,
        options.graph_hops.unwrap_or(1),
    ).await?;

    // 4. Merge and format
    Ok(RecallResult {
        facts,
        graph_context,
        formatted: format_recall_with_graph(facts, graph_context),
    })
}
```

### 3.2 Recall Output Format

```markdown
## Recalled Memory

### Relevant Facts
- User prefers Rust for backend development (confidence: 0.95)
- Project uses SQLite with WAL mode (confidence: 0.90)
- Agent Zero architecture has 13 gateway crates (confidence: 0.85)

### Related Entities
- **AgentZero** (Project)
  - uses → SQLite (storage)
  - uses → Rust (language)
  - created_by → User
- **SQLite** (Tool)
  - configured_with → WAL mode

### Connections Found
- User → prefers → Rust → used_by → AgentZero
```

---

## Phase 4: HTTP API Endpoints

### 4.1 New Endpoints

**File:** `gateway/src/http/knowledge_graph.rs`

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/graph/:agent_id/entities/:id/neighbors` | Get entity neighbors |
| `GET` | `/api/graph/:agent_id/path` | Find path between entities |
| `GET` | `/api/graph/:agent_id/entities/:id/context` | Get entity context |
| `GET` | `/api/graph/:agent_id/stats` | Graph statistics |

### 4.2 Request/Response Examples

**GET /api/graph/root/entities/AgentZero/neighbors**
```json
{
  "entity": {
    "id": "entity_root_AgentZero",
    "name": "AgentZero",
    "type": "project"
  },
  "neighbors": [
    {
      "entity": {"name": "Rust", "type": "tool"},
      "relationship": {"type": "uses", "mention_count": 5},
      "direction": "outgoing"
    },
    {
      "entity": {"name": "User", "type": "person"},
      "relationship": {"type": "created_by", "mention_count": 2},
      "direction": "outgoing"
    }
  ]
}
```

**GET /api/graph/root/path?from=User&to=SQLite&max_depth=3**
```json
{
  "found": true,
  "paths": [
    {
      "length": 2,
      "steps": [
        {"from": "User", "relation": "prefers", "to": "Rust"},
        {"from": "Rust", "relation": "uses", "to": "SQLite"}
      ]
    }
  ]
}
```

---

## Phase 5: Memory Tool Enhancement

### 5.1 New Tool Actions

**File:** `runtime/agent-tools/src/tools/memory.rs`

```rust
// New actions for graph queries
MemoryAction::GraphQuery(GraphQueryAction) => {
    match action {
        GraphQueryAction::Neighbors { entity_name, direction, limit } => {
            // Query neighbors
        }
        GraphQueryAction::Path { from, to, max_depth } => {
            // Find path
        }
        GraphQueryAction::Context { entity_names, hops } => {
            // Get context
        }
    }
}
```

### 5.2 Tool Usage Examples

```typescript
// Get entities related to "AgentZero"
memory(action="graph_neighbors", entity_name="AgentZero", direction="both", limit=10)

// Find how User is connected to SQLite
memory(action="graph_path", from="User", to="SQLite", max_depth=3)

// Get full context around mentioned entities
memory(action="graph_context", entity_names=["AgentZero", "Rust"], hops=2)
```

---

## Files Summary

### New Files
| File | Description |
|------|-------------|
| `services/knowledge-graph/src/graph.rs` | Graph data structures |
| `services/knowledge-graph/src/traversal.rs` | BFS/DFS algorithms |
| `services/knowledge-graph/src/pathfinding.rs` | Path algorithms |
| `gateway/src/http/knowledge_graph.rs` | HTTP endpoints |

### Modified Files
| File | Changes |
|------|---------|
| `services/knowledge-graph/src/repository.rs` | Add neighbor/path queries |
| `services/knowledge-graph/src/service.rs` | Add graph service methods |
| `gateway/gateway-execution/src/recall.rs` | Add graph context to recall |
| `runtime/agent-tools/src/tools/memory.rs` | Add graph query actions |
| `gateway/src/http/mod.rs` | Add graph routes |

---

## Effort Estimate

| Phase | Effort | Complexity |
|-------|--------|------------|
| Phase 1: Repository | 1 day | Medium |
| Phase 2: Service | 0.5 day | Low |
| Phase 3: Recall Integration | 1 day | Medium |
| Phase 4: HTTP API | 0.5 day | Low |
| Phase 5: Memory Tool | 0.5 day | Low |
| **Total** | **3.5 days** | |

---

## Success Criteria

- [ ] Can query neighbors of any entity
- [ ] Can find paths between entities (up to 4 hops)
- [ ] Graph context included in memory recall
- [ ] HTTP API returns valid JSON responses
- [ ] Memory tool supports graph queries
- [ ] Performance: <50ms for 2-hop neighbor queries
