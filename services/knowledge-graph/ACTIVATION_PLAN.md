# Plan: Activate Knowledge Graph

**Status**: Ready for Implementation
**Priority**: High
**Goal**: Transform the knowledge graph from write-only storage to an active, queryable system

---

## Current Problem

```
┌─────────────────────────────────────────────────────────────┐
│                    CURRENT STATE                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Distillation ──► Store Entities ──► [SITTING UNUSED]     │
│                  Store Relations                            │
│                                                             │
│   Memory Recall ──► Facts Only (no graph context)          │
│                                                             │
│   UI Memory Panel ──► Facts List (no graph view)           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

The knowledge graph is a **write-only system** - entities and relationships are extracted during distillation but never queried or displayed.

---

## Target State

```
┌─────────────────────────────────────────────────────────────┐
│                    TARGET STATE                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Distillation ──► Store Entities/Relations                 │
│                         │                                   │
│                         ▼                                   │
│   Memory Recall ◄──── Graph Context (related entities)     │
│                         │                                   │
│                         ▼                                   │
│   UI Panel ◄────── Graph View (visualization)              │
│                                                             │
│   HTTP API ◄────── Graph Queries (neighbors, paths)        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Graph Repository Layer (Foundation)

**Goal**: Add read capabilities to the existing repository

**File:** `services/knowledge-graph/src/repository.rs`

```rust
impl KnowledgeGraphRepository {
    // ===== EXISTING (Write) =====
    // store_knowledge() - already exists
    // delete_agent_data() - already exists

    // ===== NEW (Read) =====

    /// Get all entities for an agent (with optional type filter)
    pub fn list_entities(
        &self,
        agent_id: &str,
        entity_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Entity>>;

    /// Get all relationships for an agent (with optional type filter)
    pub fn list_relationships(
        &self,
        agent_id: &str,
        relationship_type: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Relationship>>;

    /// Get entity by name (case-insensitive)
    pub fn get_entity_by_name(
        &self,
        agent_id: &str,
        name: &str,
    ) -> Result<Option<Entity>>;

    /// Get neighbors of an entity (1-hop)
    pub fn get_neighbors(
        &self,
        agent_id: &str,
        entity_id: &str,
        direction: Direction,
        limit: usize,
    ) -> Result<Vec<(Entity, Relationship)>>;

    /// Count entities/relationships for an agent
    pub fn count_entities(&self, agent_id: &str) -> Result<usize>;
    pub fn count_relationships(&self, agent_id: &str) -> Result<usize>;
}

pub enum Direction {
    Outgoing,  // Entity → Other
    Incoming,  // Other → Entity
    Both,      // Either direction
}
```

**SQL Implementation:**

```sql
-- Get outgoing neighbors
SELECT e.*, r.*
FROM kg_entities e
INNER JOIN kg_relationships r ON r.target_entity_id = e.id
WHERE r.agent_id = ? AND r.source_entity_id = ?
ORDER BY r.mention_count DESC
LIMIT ?;

-- Get incoming neighbors
SELECT e.*, r.*
FROM kg_entities e
INNER JOIN kg_relationships r ON r.source_entity_id = e.id
WHERE r.agent_id = ? AND r.target_entity_id = ?
ORDER BY r.mention_count DESC
LIMIT ?;

-- Search entities by name
SELECT * FROM kg_entities
WHERE agent_id = ? AND name LIKE ? COLLATE NOCASE
ORDER BY mention_count DESC
LIMIT ?;
```

**Effort:** 0.5 day

---

### Phase 2: Graph Service Layer

**Goal**: Business logic layer with higher-level operations

**File:** `services/knowledge-graph/src/service.rs`

```rust
pub struct GraphService {
    repo: Arc<KnowledgeGraphRepository>,
}

impl GraphService {
    pub fn new(repo: Arc<KnowledgeGraphRepository>) -> Self;

    /// Get graph statistics for an agent
    pub fn get_stats(&self, agent_id: &str) -> Result<GraphStats>;

    /// Get entity with its connections
    pub fn get_entity_with_connections(
        &self,
        agent_id: &str,
        entity_name: &str,
    ) -> Result<Option<EntityWithConnections>>;

    /// Search entities by name (fuzzy)
    pub fn search_entities(
        &self,
        agent_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Entity>>;

    /// Get subgraph (entities within N hops of a center entity)
    pub fn get_subgraph(
        &self,
        agent_id: &str,
        center_entity_id: &str,
        max_hops: usize,
    ) -> Result<Subgraph>;
}

pub struct GraphStats {
    pub entity_count: usize,
    pub relationship_count: usize,
    pub entity_types: HashMap<String, usize>,
    pub relationship_types: HashMap<String, usize>,
    pub most_connected_entities: Vec<(String, usize)>,
}

pub struct EntityWithConnections {
    pub entity: Entity,
    pub outgoing: Vec<(Relationship, Entity)>,
    pub incoming: Vec<(Relationship, Entity)>,
}

pub struct Subgraph {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
    pub center: String,
}
```

**Effort:** 0.5 day

---

### Phase 3: HTTP API Endpoints

**Goal:** Expose graph data via REST API

**File:** `gateway/src/http/graph.rs` (new file)

```rust
/// GET /api/graph/:agent_id/stats
/// Get graph statistics
pub async fn get_graph_stats(
    Path(agent_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<GraphStatsResponse>, (StatusCode, Json<ErrorResponse>)>;

/// GET /api/graph/:agent_id/entities
/// List entities with optional filters
pub async fn list_entities(
    Path(agent_id): Path<String>,
    Query(params): Query<EntityListQuery>,
    State(state): State<AppState>,
) -> Result<Json<EntityListResponse>, (StatusCode, Json<ErrorResponse>)>;

/// GET /api/graph/:agent_id/relationships
/// List relationships with optional filters
pub async fn list_relationships(
    Path(agent_id): Path<String>,
    Query(params): Query<RelationshipListQuery>,
    State(state): State<AppState>,
) -> Result<Json<RelationshipListResponse>, (StatusCode, Json<ErrorResponse>)>;

/// GET /api/graph/:agent_id/entities/:entity_id/neighbors
/// Get neighbors of an entity
pub async fn get_entity_neighbors(
    Path((agent_id, entity_id)): Path<(String, String)>,
    Query(params): Query<NeighborQuery>,
    State(state): State<AppState>,
) -> Result<Json<NeighborResponse>, (StatusCode, Json<ErrorResponse>)>;
```

**Routes to add in `gateway/src/http/mod.rs`:**

```rust
// Graph API endpoints
.route("/api/graph/:agent_id/stats", get(graph::get_graph_stats))
.route("/api/graph/:agent_id/entities", get(graph::list_entities))
.route("/api/graph/:agent_id/relationships", get(graph::list_relationships))
.route("/api/graph/:agent_id/entities/:entity_id/neighbors", get(graph::get_entity_neighbors))
```

**Effort:** 0.5 day

---

### Phase 4: Memory UI - Graph View

**Goal:** Visualize the knowledge graph in the memory panel

**File:** `apps/ui/src/features/memory/GraphView.tsx` (new file)

```tsx
interface GraphViewProps {
  agentId?: string;  // Optional filter
}

// Simple force-directed graph visualization
// Uses react-force-graph-2d or d3-force

export function GraphView({ agentId }: GraphViewProps) {
  const [graphData, setGraphData] = useState<GraphData | null>(null);

  // Fetch graph data
  useEffect(() => {
    const fetchGraph = async () => {
      const transport = await getTransport();
      // New transport method needed
      const result = await transport.getGraphData(agentId);
      if (result.success && result.data) {
        setGraphData(result.data);
      }
    };
    fetchGraph();
  }, [agentId]);

  return (
    <div className="graph-view">
      {/* Force-directed graph visualization */}
      {/* Nodes = entities, colored by type */}
      {/* Edges = relationships, labeled by type */}
      {/* Click node → show details panel */}
    </div>
  );
}
```

**Update `WebMemoryPanel.tsx`:**

```tsx
// Add tabs: Facts | Graph
const [activeView, setActiveView] = useState<'facts' | 'graph'>('facts');

return (
  <div className="page">
    {/* Tab switcher */}
    <div className="flex gap-2 mb-4">
      <button
        className={activeView === 'facts' ? 'btn btn--primary' : 'btn btn--secondary'}
        onClick={() => setActiveView('facts')}
      >
        Facts
      </button>
      <button
        className={activeView === 'graph' ? 'btn btn--primary' : 'btn btn--secondary'}
        onClick={() => setActiveView('graph')}
      >
        Knowledge Graph
      </button>
    </div>

    {/* Content */}
    {activeView === 'facts' ? <FactsList /> : <GraphView agentId={selectedAgentId} />}
  </div>
);
```

**Effort:** 1 day

---

### Phase 5: Integrate Graph with Memory Recall

**Goal:** Include related entities when recalling facts

**File:** `gateway/gateway-execution/src/recall.rs`

```rust
pub async fn recall_with_graph(
    agent_id: &str,
    query: &str,
    options: RecallOptions,
) -> Result<RecallResult> {
    // 1. Standard fact search
    let facts = search_memory_facts_hybrid(query, agent_id, options.limit)?;

    // 2. Extract entity names from top facts
    let entity_names = extract_entity_names_from_facts(&facts);

    // 3. Get graph context for those entities
    let graph_context = if !entity_names.is_empty() {
        Some(graph_service.get_entity_context(agent_id, &entity_names).await?)
    } else {
        None
    };

    // 4. Format combined result
    Ok(RecallResult {
        facts,
        graph_context,
        formatted: format_combined_recall(facts, graph_context),
    })
}

fn format_combined_recall(facts: Vec<MemoryFact>, graph: Option<GraphContext>) -> String {
    let mut output = String::new();

    // Facts section
    output.push_str("## Relevant Facts\n");
    for fact in facts {
        output.push_str(&format!("- {} (confidence: {:.2})\n", fact.content, fact.confidence));
    }

    // Graph context section
    if let Some(ctx) = graph {
        if !ctx.relationships.is_empty() {
            output.push_str("\n## Related Entities\n");
            for rel in ctx.relationships {
                output.push_str(&format!(
                    "- {} {} {} (mentioned {} times)\n",
                    rel.source_name, rel.relationship_type, rel.target_name, rel.mention_count
                ));
            }
        }
    }

    output
}
```

**Effort:** 0.5 day

---

### Phase 6: Transport Layer Updates

**Goal:** Add graph API methods to transport

**File:** `apps/ui/src/services/transport/interface.ts`

```typescript
// Graph operations
getGraphStats(agentId?: string): Promise<TransportResult<GraphStatsResponse>>;
getGraphEntities(agentId?: string, filter?: EntityFilter): Promise<TransportResult<EntityListResponse>>;
getGraphRelationships(agentId?: string, filter?: RelationshipFilter): Promise<TransportResult<RelationshipListResponse>>;
getEntityNeighbors(entityId: string, direction?: 'incoming' | 'outgoing' | 'both'): Promise<TransportResult<NeighborResponse>>;
```

**File:** `apps/ui/src/services/transport/types.ts`

```typescript
export interface GraphStatsResponse {
  entity_count: number;
  relationship_count: number;
  entity_types: Record<string, number>;
  relationship_types: Record<string, number>;
}

export interface EntityListResponse {
  entities: GraphEntity[];
  total: number;
}

export interface GraphEntity {
  id: string;
  agent_id: string;
  entity_type: string;
  name: string;
  properties: Record<string, unknown>;
  mention_count: number;
  first_seen_at: string;
  last_seen_at: string;
}

export interface RelationshipListResponse {
  relationships: GraphRelationship[];
  total: number;
}

export interface GraphRelationship {
  id: string;
  agent_id: string;
  source_entity_id: string;
  source_entity_name: string;
  target_entity_id: string;
  target_entity_name: string;
  relationship_type: string;
  mention_count: number;
}

export interface NeighborResponse {
  entity: GraphEntity;
  neighbors: Array<{
    entity: GraphEntity;
    relationship: GraphRelationship;
    direction: 'incoming' | 'outgoing';
  }>;
}
```

**Effort:** 0.5 day

---

## Files Summary

### New Files
| File | Description |
|------|-------------|
| `gateway/src/http/graph.rs` | HTTP API handlers |
| `apps/ui/src/features/memory/GraphView.tsx` | Graph visualization component |
| `apps/ui/src/features/memory/FactsList.tsx` | Extract facts list (refactor) |

### Modified Files
| File | Changes |
|------|---------|
| `services/knowledge-graph/src/repository.rs` | Add read methods |
| `services/knowledge-graph/src/service.rs` | Add service methods |
| `services/knowledge-graph/src/lib.rs` | Export new modules |
| `gateway/src/http/mod.rs` | Add graph routes |
| `gateway/src/state.rs` | Add GraphService to AppState |
| `gateway/gateway-execution/src/recall.rs` | Add graph context |
| `apps/ui/src/features/memory/WebMemoryPanel.tsx` | Add graph tab |
| `apps/ui/src/services/transport/interface.ts` | Add graph methods |
| `apps/ui/src/services/transport/http.ts` | Implement graph methods |
| `apps/ui/src/services/transport/types.ts` | Add graph types |

---

## Effort Estimate

| Phase | Effort |
|-------|--------|
| Phase 1: Repository | 0.5 day |
| Phase 2: Service | 0.5 day |
| Phase 3: HTTP API | 0.5 day |
| Phase 4: UI Graph View | 1 day |
| Phase 5: Recall Integration | 0.5 day |
| Phase 6: Transport | 0.5 day |
| **Total** | **3.5 days** |

---

## Success Criteria

- [ ] HTTP API returns entities and relationships
- [ ] Memory panel has Facts/Graph tab switcher
- [ ] Graph view shows entities as nodes, relationships as edges
- [ ] Clicking entity shows its connections
- [ ] Memory recall includes related entities
- [ ] Can filter graph by agent
- [ ] Performance: <100ms for loading graph data

---

## Order of Implementation

```
Phase 1 (Repository) ──► Phase 2 (Service) ──► Phase 3 (HTTP API)
                                                        │
                                                        ▼
Phase 6 (Transport) ◄──────────────────────────────────┘
         │
         ▼
Phase 4 (UI) ──► Phase 5 (Recall Integration)
```

**Recommended start:** Phase 1 (Repository) - foundation for everything else
