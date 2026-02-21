# Plan: Graph Algorithms for Knowledge Graph

**Status**: Planned
**Priority**: Medium
**Depends On**: Graph Query API (Plan 1)

---

## Problem

The knowledge graph lacks **analytical capabilities**:
- Cannot identify important entities (centrality)
- Cannot discover entity clusters (community detection)
- Cannot generate recommendations
- Cannot detect isolated/disconnected components

**Current state**: Flat storage with no insights

---

## Solution

Add **Graph Algorithms** that enable:
1. **Centrality measures** - Find important entities
2. **Community detection** - Discover entity clusters
3. **Recommendations** - Suggest related entities
4. **Graph analytics** - Health and statistics

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Graph Algorithms Layer                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ Centrality  │  │ Community   │  │ Recommendations         │ │
│  │ PageRank    │  │ Detection   │  │ (Collaborative Filter)  │ │
│  │ Degree      │  │ Louvain     │  │ Content-Based           │ │
│  │ Betweenness │  │ Label Prop  │  │ Graph-Based             │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                      Graph Analytics                            │
│  - Statistics (nodes, edges, density)                          │
│  - Health checks (isolated nodes, orphans)                     │
│  - Trends (growth over time)                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Centrality Algorithms

### 1.1 Centrality Types

```rust
/// Entity importance metrics
pub struct CentralityScores {
    pub entity_id: String,
    pub degree_centrality: f64,      // Number of connections
    pub weighted_degree: f64,        // Weighted by mention_count
    pub pagerank: f64,               // Iterative importance
    pub betweenness: f64,            // Bridge entities (optional)
}

pub struct CentralityConfig {
    pub algorithm: CentralityAlgorithm,
    pub entity_type_filter: Option<Vec<EntityType>>,
    pub relationship_type_filter: Option<Vec<RelationshipType>>,
    pub top_k: usize,
    pub min_connections: usize,
}

pub enum CentralityAlgorithm {
    Degree,          // Simple connection count
    WeightedDegree,  // Weighted by mention_count
    PageRank,        // Iterative importance (damping=0.85)
    Eigenvector,     // Connected to important entities
}
```

### 1.2 Implementation

**File:** `services/knowledge-graph/src/algorithms/centrality.rs`

```rust
impl GraphAlgorithms {
    /// Calculate degree centrality for all entities
    pub fn degree_centrality(&self, agent_id: &str) -> Result<Vec<CentralityScores>> {
        // SQL: COUNT relationships per entity
        // Normalize by max possible connections
    }

    /// Calculate weighted degree (sum of mention_counts)
    pub fn weighted_degree(&self, agent_id: &str) -> Result<Vec<CentralityScores>> {
        // SQL: SUM(mention_count) for all relationships
    }

    /// PageRank algorithm (iterative)
    pub fn pagerank(
        &self,
        agent_id: &str,
        damping: f64,
        iterations: usize,
    ) -> Result<Vec<CentralityScores>> {
        // 1. Load adjacency list
        // 2. Initialize scores to 1/N
        // 3. Iterate: score = (1-d)/N + d * sum(incoming_scores / outgoing_degree)
        // 4. Return top-K entities
    }

    /// Get top important entities
    pub fn top_entities(
        &self,
        agent_id: &str,
        config: CentralityConfig,
    ) -> Result<Vec<RankedEntity>> {
        match config.algorithm {
            Degree => self.degree_centrality(agent_id),
            WeightedDegree => self.weighted_degree(agent_id),
            PageRank => self.pagerank(agent_id, 0.85, 20),
        }
        .map(|scores| self.rank_and_filter(scores, config))
    }
}

pub struct RankedEntity {
    pub entity: Entity,
    pub rank: usize,
    pub score: f64,
    pub score_type: String,
}
```

### 1.3 SQL for Degree Centrality

```sql
-- Combined incoming + outgoing degree
SELECT
    e.id,
    e.name,
    e.entity_type,
    COUNT(DISTINCT r.id) as degree,
    SUM(r.mention_count) as weighted_degree
FROM kg_entities e
LEFT JOIN kg_relationships r
    ON r.source_entity_id = e.id OR r.target_entity_id = e.id
WHERE e.agent_id = ?
GROUP BY e.id
ORDER BY degree DESC
LIMIT ?;
```

---

## Phase 2: Community Detection

### 2.1 Community Structures

```rust
/// Detected community of related entities
pub struct Community {
    pub id: String,
    pub agent_id: String,
    pub entities: Vec<Entity>,
    pub entity_count: usize,
    pub internal_edges: usize,
    pub modularity: f64,
    pub dominant_type: Option<EntityType>,
    pub label: Option<String>,  // Auto-generated label
}

pub struct CommunityDetectionResult {
    pub communities: Vec<Community>,
    pub total_entities: usize,
    pub total_edges: usize,
    pub modularity_score: f64,  // Quality measure
    pub algorithm: String,
}
```

### 2.2 Implementation (Label Propagation)

**File:** `services/knowledge-graph/src/algorithms/community.rs`

```rust
impl GraphAlgorithms {
    /// Detect communities using Label Propagation (fast, simple)
    pub fn detect_communities_label_propagation(
        &self,
        agent_id: &str,
        max_iterations: usize,
    ) -> Result<CommunityDetectionResult> {
        // 1. Load graph into memory (adjacency list)
        // 2. Initialize: each entity has unique label
        // 3. Iterate: each entity adopts majority label of neighbors
        // 4. Converge when labels stabilize
        // 5. Group entities by final label -> communities
    }

    /// Get community for a specific entity
    pub fn get_entity_community(&self, entity_id: &str) -> Result<Option<Community>>;

    /// Find entities similar to target (same community)
    pub fn find_similar_entities(&self, entity_id: &str, limit: usize) -> Result<Vec<Entity>>;
}
```

### 2.3 Algorithm: Label Propagation

```
Input: Graph G = (V, E)
Output: Communities C1, C2, ..., Ck

1. Initialize: Each vertex v has label L(v) = v
2. Repeat until convergence:
   a. Shuffle vertices randomly
   b. For each vertex v:
      L(v) = most_frequent_label(neighbors(v))
3. Group vertices by label -> communities
```

**Properties:**
- O(m) time complexity (m = edges)
- Fast convergence (usually < 10 iterations)
- Non-deterministic (run multiple times, pick best)

---

## Phase 3: Recommendations

### 3.1 Recommendation Types

```rust
pub struct Recommendation {
    pub entity: Entity,
    pub score: f64,
    pub reason: RecommendationReason,
    pub evidence: Vec<String>,  // Why this was recommended
}

pub enum RecommendationReason {
    SimilarEntities,      // Same community
    FrequentlyCoMentioned, // Appear together in sessions
    RelatedByType,        // Same entity type
    PathExists,           // Connected via short path
    Popular,              // High centrality
}

pub struct RecommendationConfig {
    pub entity_id: String,
    pub max_recommendations: usize,
    pub min_score: f64,
    pub include_reasons: bool,
    pub entity_types: Option<Vec<EntityType>>,
}
```

### 3.2 Implementation

**File:** `services/knowledge-graph/src/algorithms/recommendations.rs`

```rust
impl GraphAlgorithms {
    /// Get entity recommendations using multiple signals
    pub fn recommend_entities(
        &self,
        agent_id: &str,
        config: RecommendationConfig,
    ) -> Result<Vec<Recommendation>> {
        let mut scores: HashMap<String, f64> = HashMap::new();

        // Signal 1: Same community (weight: 0.3)
        if let Some(community) = self.get_entity_community(&config.entity_id)? {
            for entity in community.entities {
                *scores.entry(entity.id).or_default() += 0.3;
            }
        }

        // Signal 2: 2-hop neighbors (weight: 0.25)
        let neighbors = self.get_2hop_neighbors(&config.entity_id)?;
        for entity in neighbors {
            *scores.entry(entity.id).or_default() += 0.25;
        }

        // Signal 3: Same type + high centrality (weight: 0.2)
        let entity = self.get_entity(&config.entity_id)?;
        let same_type = self.get_entities_by_type(agent_id, &entity.entity_type)?;
        let centralities = self.degree_centrality(agent_id)?;
        for (e, cent) in same_type.iter().zip(centralities.iter()) {
            *scores.entry(e.id.clone()).or_default() += 0.2 * cent.normalized;
        }

        // Signal 4: Frequently co-mentioned (weight: 0.25)
        let co_mentions = self.get_co_mentioned_entities(&config.entity_id)?;
        for (entity_id, count) in co_mentions {
            *scores.entry(entity_id).or_default() += 0.25 * (count as f64).log2();
        }

        // Sort, filter, return top-K
        self.rank_recommendations(scores, config)
    }
}
```

---

## Phase 4: Graph Analytics

### 4.1 Statistics

```rust
pub struct GraphStatistics {
    // Basic counts
    pub total_entities: usize,
    pub total_relationships: usize,
    pub entity_types: HashMap<EntityType, usize>,
    pub relationship_types: HashMap<RelationshipType, usize>,

    // Connectivity
    pub density: f64,
    pub avg_degree: f64,
    pub max_degree: usize,
    pub isolated_entities: usize,

    // Communities
    pub community_count: usize,
    pub largest_community_size: usize,
    pub modularity: f64,

    // Health
    pub orphan_entities: usize,      // No relationships
    pub singleton_communities: usize, // Communities of size 1
    pub strongly_connected: bool,
}

pub struct GraphTrends {
    pub entities_added_7d: usize,
    pub relationships_added_7d: usize,
    pub most_active_entities: Vec<Entity>,
    pub emerging_types: Vec<EntityType>,
}
```

### 4.2 Implementation

**File:** `services/knowledge-graph/src/analytics.rs`

```rust
impl GraphAnalytics {
    /// Calculate comprehensive graph statistics
    pub fn compute_statistics(&self, agent_id: &str) -> Result<GraphStatistics>;

    /// Compute trends over time
    pub fn compute_trends(&self, agent_id: &str, days: usize) -> Result<GraphTrends>;

    /// Identify graph health issues
    pub fn health_check(&self, agent_id: &str) -> Result<Vec<HealthIssue>>;

    /// Export graph for visualization
    pub fn export_for_visualization(&self, agent_id: &str) -> Result<GraphVisualization>;
}

pub struct HealthIssue {
    pub severity: Severity,
    pub issue_type: String,
    pub description: String,
    pub affected_entities: Vec<String>,
    pub suggestion: String,
}

pub struct GraphVisualization {
    pub nodes: Vec<VisualizationNode>,
    pub edges: Vec<VisualizationEdge>,
    pub layout: LayoutHint,
}
```

---

## Phase 5: HTTP API

### 5.1 Endpoints

**File:** `gateway/src/http/knowledge_graph.rs`

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/graph/:agent_id/centrality` | Top entities by importance |
| `GET` | `/api/graph/:agent_id/communities` | Detected communities |
| `GET` | `/api/graph/:agent_id/recommendations/:entity_id` | Entity recommendations |
| `GET` | `/api/graph/:agent_id/statistics` | Graph statistics |
| `GET` | `/api/graph/:agent_id/health` | Health issues |
| `GET` | `/api/graph/:agent_id/visualization` | Export for viz |

### 5.2 Response Examples

**GET /api/graph/root/centrality?algorithm=pagerank&top=10**
```json
{
  "algorithm": "pagerank",
  "entities": [
    {
      "entity": {"name": "AgentZero", "type": "project"},
      "rank": 1,
      "score": 0.156,
      "connections": 12
    },
    {
      "entity": {"name": "Rust", "type": "tool"},
      "rank": 2,
      "score": 0.124,
      "connections": 8
    }
  ]
}
```

**GET /api/graph/root/communities**
```json
{
  "communities": [
    {
      "id": "comm_1",
      "entity_count": 5,
      "dominant_type": "tool",
      "entities": [
        {"name": "Rust", "type": "tool"},
        {"name": "SQLite", "type": "tool"},
        {"name": "Tokio", "type": "tool"}
      ],
      "modularity": 0.42
    }
  ],
  "total_communities": 3,
  "modularity_score": 0.38
}
```

**GET /api/graph/root/recommendations/AgentZero?limit=5**
```json
{
  "source_entity": "AgentZero",
  "recommendations": [
    {
      "entity": {"name": "SQLite", "type": "tool"},
      "score": 0.82,
      "reason": "frequently_co_mentioned",
      "evidence": ["Appears together in 8 sessions"]
    },
    {
      "entity": {"name": "Gateway", "type": "concept"},
      "score": 0.71,
      "reason": "path_exists",
      "evidence": ["Connected via 2-hop path: AgentZero → uses → Rust → configured_with → Gateway"]
    }
  ]
}
```

---

## Phase 6: Memory Tool Integration

### 6.1 New Actions

```typescript
// Get most important entities
memory(action="graph_important", algorithm="pagerank", limit=10)

// Discover entity communities
memory(action="graph_communities", min_size=3)

// Get recommendations for an entity
memory(action="graph_recommend", entity="AgentZero", limit=5)

// Get graph statistics
memory(action="graph_stats")
```

### 6.2 Agent Prompt Integration

When agent asks "What's important in this project?", the system can:
1. Run PageRank to find central entities
2. Detect communities to understand project structure
3. Provide natural language summary

---

## Files Summary

### New Files
| File | Description |
|------|-------------|
| `services/knowledge-graph/src/algorithms/mod.rs` | Algorithm module |
| `services/knowledge-graph/src/algorithms/centrality.rs` | Centrality algorithms |
| `services/knowledge-graph/src/algorithms/community.rs` | Community detection |
| `services/knowledge-graph/src/algorithms/recommendations.rs` | Recommendation engine |
| `services/knowledge-graph/src/analytics.rs` | Statistics and health |

### Modified Files
| File | Changes |
|------|---------|
| `services/knowledge-graph/src/lib.rs` | Export algorithms |
| `services/knowledge-graph/src/service.rs` | Add algorithm methods |
| `gateway/src/http/knowledge_graph.rs` | Add algorithm endpoints |
| `runtime/agent-tools/src/tools/memory.rs` | Add algorithm actions |

---

## Effort Estimate

| Phase | Effort | Complexity |
|-------|--------|------------|
| Phase 1: Centrality | 1.5 days | Medium |
| Phase 2: Community Detection | 1.5 days | Medium-High |
| Phase 3: Recommendations | 1 day | Medium |
| Phase 4: Analytics | 0.5 day | Low |
| Phase 5: HTTP API | 0.5 day | Low |
| Phase 6: Tool Integration | 0.5 day | Low |
| **Total** | **5.5 days** | |

---

## Success Criteria

- [ ] Can identify top 10 most important entities via PageRank
- [ ] Can detect entity communities with modularity > 0.3
- [ ] Can generate entity recommendations with relevance scores
- [ ] HTTP API returns valid statistics and health checks
- [ ] Memory tool supports algorithm queries
- [ ] Performance: Centrality <100ms for 1000 entities
- [ ] Performance: Community detection <500ms for 500 entities

---

## Dependencies

- **Graph Query API (Plan 1)** must be implemented first
- SQLite with recursive CTE support (available)
- No external graph libraries required (pure Rust implementation)

---

## Future Enhancements

1. **Incremental updates** - Recompute only changed entities
2. **Caching** - Store computed scores, refresh periodically
3. **Visualization** - D3.js/Cytoscape integration for graph view
4. **Machine learning** - Learn entity similarity from user feedback
5. **Temporal analysis** - Track how importance changes over time
