// ============================================================================
// GRAPH QUERY TOOL
// Search, explore neighbors, and get contextual subgraphs from the knowledge graph.
// ============================================================================

// Public API types — consumed by downstream crates (e.g., pi-mono) that wire
// a concrete GraphStorageAccess into the tool. No internal caller yet.
#![allow(dead_code)]

use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use zero_core::{Result, Tool, ToolContext, ZeroError};

// ============================================================================
// DATA TYPES
// ============================================================================

/// Minimal entity information returned by graph queries.
#[derive(Debug, Clone)]
pub struct EntityInfo {
    pub name: String,
    pub entity_type: String,
    pub mention_count: i64,
}

/// A neighbor entity with the relationship connecting it.
#[derive(Debug, Clone)]
pub struct NeighborInfo {
    pub entity: EntityInfo,
    pub relationship_type: String,
    /// "outgoing" or "incoming"
    pub direction: String,
}

// ============================================================================
// GRAPH STORAGE TRAIT
// ============================================================================

/// Abstraction over the knowledge-graph storage layer.
///
/// Implementors provide the actual DB queries; the tool formats the results.
#[async_trait]
pub trait GraphStorageAccess: Send + Sync + 'static {
    /// Find entities whose name matches `query` (LIKE / substring search).
    async fn search_entities_by_name(
        &self,
        query: &str,
        entity_type: Option<&str>,
        limit: usize,
    ) -> std::result::Result<Vec<EntityInfo>, String>;

    /// Find entities whose name matches `query`, ranked by the requested view.
    ///
    /// `view` is one of `"semantic"`, `"temporal"`, `"entity"`, `"hybrid"`.
    /// Unknown values fall back to `"semantic"`.
    ///
    /// Default implementation delegates to [`search_entities_by_name`], so
    /// implementors only need to override for MAGMA-style views.
    async fn search_entities_with_view(
        &self,
        query: &str,
        entity_type: Option<&str>,
        _view: &str,
        limit: usize,
    ) -> std::result::Result<Vec<EntityInfo>, String> {
        self.search_entities_by_name(query, entity_type, limit)
            .await
    }

    /// Get entities connected to `entity_name`.
    /// `direction` is one of "outgoing", "incoming", or "both".
    async fn get_entity_neighbors(
        &self,
        entity_name: &str,
        direction: &str,
        limit: usize,
    ) -> std::result::Result<Vec<NeighborInfo>, String>;

    /// Look up a single entity by exact name.
    async fn get_entity_by_name(
        &self,
        name: &str,
    ) -> std::result::Result<Option<EntityInfo>, String>;
}

// ============================================================================
// TOOL
// ============================================================================

/// Tool that lets agents query the knowledge graph.
///
/// Supports three actions:
/// - `search` — find entities by name
/// - `neighbors` — explore connections from a given entity
/// - `context` — get entities and relationships relevant to a topic
pub struct GraphQueryTool {
    storage: Arc<dyn GraphStorageAccess>,
}

impl GraphQueryTool {
    pub fn new(storage: Arc<dyn GraphStorageAccess>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl Tool for GraphQueryTool {
    fn name(&self) -> &str {
        "graph_query"
    }

    fn description(&self) -> &str {
        "Query the knowledge graph: search entities, explore neighbors, or get contextual subgraphs."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "neighbors", "context"],
                    "description": "The query action to perform"
                },
                "query": {
                    "type": "string",
                    "description": "Search query string (for 'search' and 'context' actions)"
                },
                "entity_name": {
                    "type": "string",
                    "description": "Entity name (for 'neighbors' action)"
                },
                "entity_type": {
                    "type": "string",
                    "description": "Filter by entity type (for 'search' action)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["outgoing", "incoming", "both"],
                    "default": "both",
                    "description": "Relationship direction (for 'neighbors' action)"
                },
                "depth": {
                    "type": "integer",
                    "default": 1,
                    "description": "Traversal depth 1-2 (for 'neighbors' action)"
                },
                "limit": {
                    "type": "integer",
                    "default": 20,
                    "description": "Maximum number of results"
                },
                "view": {
                    "type": "string",
                    "enum": ["semantic", "temporal", "entity", "hybrid"],
                    "description": "Query view: semantic (by similarity/mentions, default), temporal (most recent first), entity (most connected first), hybrid (reranked combination)"
                }
            },
            "required": ["action"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'action' parameter".to_string()))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        match action {
            "search" => self.handle_search(&args, limit).await,
            "neighbors" => self.handle_neighbors(&args, limit).await,
            "context" => self.handle_context(&args, limit).await,
            other => Err(ZeroError::Tool(format!(
                "Unknown action '{}'. Use 'search', 'neighbors', or 'context'.",
                other
            ))),
        }
    }
}

// ============================================================================
// ACTION HANDLERS
// ============================================================================

impl GraphQueryTool {
    async fn handle_search(&self, args: &Value, limit: usize) -> Result<Value> {
        let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
            ZeroError::Tool("Missing 'query' parameter for search action".to_string())
        })?;

        let entity_type = args.get("entity_type").and_then(|v| v.as_str());
        let view = args
            .get("view")
            .and_then(|v| v.as_str())
            .unwrap_or("semantic");

        let entities = self
            .storage
            .search_entities_with_view(query, entity_type, view, limit)
            .await
            .map_err(|e| ZeroError::Tool(format!("Graph search failed: {e}")))?;

        if entities.is_empty() {
            return Ok(json!({
                "result": format!("No entities found matching \"{}\".", query)
            }));
        }

        let mut md = format!("## Entities matching \"{}\"\n", query);
        for e in &entities {
            let _ = writeln!(
                md,
                "- **{}** ({}): mentions={}",
                e.name, e.entity_type, e.mention_count
            );
        }

        Ok(json!({ "result": md, "count": entities.len() }))
    }

    async fn handle_neighbors(&self, args: &Value, limit: usize) -> Result<Value> {
        let entity_name = args
            .get("entity_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ZeroError::Tool("Missing 'entity_name' parameter for neighbors action".to_string())
            })?;

        let direction = args
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("both");

        let depth = args
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .min(2) as usize;

        // Verify the entity exists first.
        let entity = self
            .storage
            .get_entity_by_name(entity_name)
            .await
            .map_err(|e| ZeroError::Tool(format!("Graph lookup failed: {e}")))?;

        let Some(entity) = entity else {
            return Ok(json!({
                "result": format!("Entity \"{}\" not found in the knowledge graph.", entity_name)
            }));
        };

        let mut md = format!(
            "## Neighbors of \"{}\" ({})\n",
            entity.name, entity.entity_type
        );

        // Depth-1 neighbors
        let neighbors = self
            .storage
            .get_entity_neighbors(entity_name, direction, limit)
            .await
            .map_err(|e| ZeroError::Tool(format!("Graph neighbors failed: {e}")))?;

        if neighbors.is_empty() {
            let _ = writeln!(md, "No relationships found.");
            return Ok(json!({ "result": md, "count": 0 }));
        }

        Self::append_neighbors(&mut md, &neighbors, 1);

        // Depth-2: expand each depth-1 neighbor
        if depth >= 2 {
            self.expand_depth2(&mut md, &neighbors, direction, limit)
                .await?;
        }

        let count = neighbors.len();
        Ok(json!({ "result": md, "count": count }))
    }

    fn append_neighbors(md: &mut String, neighbors: &[NeighborInfo], depth: usize) {
        let prefix = if depth == 1 { "" } else { "  " };
        for n in neighbors {
            let arrow = if n.direction == "outgoing" {
                "-->"
            } else {
                "<--"
            };
            let _ = writeln!(
                md,
                "{}- {} {} **{}** ({}) via `{}`",
                prefix,
                n.entity.name,
                arrow,
                n.entity.name,
                n.entity.entity_type,
                n.relationship_type
            );
        }
    }

    async fn expand_depth2(
        &self,
        md: &mut String,
        neighbors: &[NeighborInfo],
        direction: &str,
        limit: usize,
    ) -> Result<()> {
        let _ = writeln!(md, "\n### Depth 2");
        for n in neighbors.iter().take(5) {
            let depth2 = self
                .storage
                .get_entity_neighbors(&n.entity.name, direction, limit.min(10))
                .await
                .map_err(|e| ZeroError::Tool(format!("Depth-2 expansion failed: {e}")))?;
            if !depth2.is_empty() {
                let _ = writeln!(md, "#### From \"{}\"", n.entity.name);
                Self::append_neighbors(md, &depth2, 2);
            }
        }
        Ok(())
    }

    async fn handle_context(&self, args: &Value, limit: usize) -> Result<Value> {
        let topic = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
            ZeroError::Tool("Missing 'query' parameter for context action".to_string())
        })?;

        let view = args
            .get("view")
            .and_then(|v| v.as_str())
            .unwrap_or("semantic");

        // Search for entities matching the topic
        let entities = self
            .storage
            .search_entities_with_view(topic, None, view, limit)
            .await
            .map_err(|e| ZeroError::Tool(format!("Context search failed: {e}")))?;

        if entities.is_empty() {
            return Ok(json!({
                "result": format!("No context found for topic \"{}\".", topic)
            }));
        }

        let mut md = format!("## Context for \"{}\"\n\n### Entities\n", topic);
        for e in &entities {
            let _ = writeln!(
                md,
                "- **{}** ({}): mentions={}",
                e.name, e.entity_type, e.mention_count
            );
        }

        // Gather relationships for found entities (limit expansion)
        let _ = writeln!(md, "\n### Relationships");
        let mut has_relationships = false;
        for e in entities.iter().take(5) {
            let neighbors = self
                .storage
                .get_entity_neighbors(&e.name, "both", 10)
                .await
                .map_err(|err| ZeroError::Tool(format!("Context neighbors failed: {err}")))?;
            for n in &neighbors {
                has_relationships = true;
                let arrow = if n.direction == "outgoing" {
                    "-->"
                } else {
                    "<--"
                };
                let _ = writeln!(
                    md,
                    "- {} {} {} via `{}`",
                    e.name, arrow, n.entity.name, n.relationship_type
                );
            }
        }

        if !has_relationships {
            let _ = writeln!(md, "No relationships found.");
        }

        Ok(json!({ "result": md, "count": entities.len() }))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use zero_core::{CallbackContext, Content, EventActions, ReadonlyContext, ToolContext};

    // --- Mock graph storage ---

    struct MockGraphStorage {
        entities: Vec<EntityInfo>,
        neighbors: Vec<NeighborInfo>,
    }

    impl MockGraphStorage {
        fn sample() -> Self {
            let entities = vec![
                EntityInfo {
                    name: "yfinance".to_string(),
                    entity_type: "module".to_string(),
                    mention_count: 5,
                },
                EntityInfo {
                    name: "rate_limiting".to_string(),
                    entity_type: "concept".to_string(),
                    mention_count: 3,
                },
                EntityInfo {
                    name: "pandas".to_string(),
                    entity_type: "module".to_string(),
                    mention_count: 8,
                },
            ];
            let neighbors = vec![
                NeighborInfo {
                    entity: EntityInfo {
                        name: "rate_limiting".to_string(),
                        entity_type: "concept".to_string(),
                        mention_count: 3,
                    },
                    relationship_type: "requires".to_string(),
                    direction: "outgoing".to_string(),
                },
                NeighborInfo {
                    entity: EntityInfo {
                        name: "pandas".to_string(),
                        entity_type: "module".to_string(),
                        mention_count: 8,
                    },
                    relationship_type: "depends_on".to_string(),
                    direction: "outgoing".to_string(),
                },
            ];
            Self {
                entities,
                neighbors,
            }
        }

        fn empty() -> Self {
            Self {
                entities: vec![],
                neighbors: vec![],
            }
        }
    }

    #[async_trait]
    impl GraphStorageAccess for MockGraphStorage {
        async fn search_entities_by_name(
            &self,
            query: &str,
            entity_type: Option<&str>,
            limit: usize,
        ) -> std::result::Result<Vec<EntityInfo>, String> {
            let query_lower = query.to_lowercase();
            Ok(self
                .entities
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&query_lower))
                .filter(|e| entity_type.map(|t| e.entity_type == t).unwrap_or(true))
                .take(limit)
                .cloned()
                .collect())
        }

        async fn get_entity_neighbors(
            &self,
            _entity_name: &str,
            direction: &str,
            limit: usize,
        ) -> std::result::Result<Vec<NeighborInfo>, String> {
            Ok(self
                .neighbors
                .iter()
                .filter(|n| direction == "both" || n.direction == direction)
                .take(limit)
                .cloned()
                .collect())
        }

        async fn get_entity_by_name(
            &self,
            name: &str,
        ) -> std::result::Result<Option<EntityInfo>, String> {
            Ok(self.entities.iter().find(|e| e.name == name).cloned())
        }
    }

    // --- Mock tool context ---

    struct MockToolContext;

    impl ReadonlyContext for MockToolContext {
        fn invocation_id(&self) -> &str {
            "test"
        }
        fn agent_name(&self) -> &str {
            "test-agent"
        }
        fn user_id(&self) -> &str {
            "test"
        }
        fn app_name(&self) -> &str {
            "test"
        }
        fn session_id(&self) -> &str {
            "test-session"
        }
        fn branch(&self) -> &str {
            "test"
        }
        fn user_content(&self) -> &Content {
            use std::sync::LazyLock;
            static CONTENT: LazyLock<Content> = LazyLock::new(|| Content {
                role: "user".to_string(),
                parts: vec![],
            });
            &CONTENT
        }
    }

    impl CallbackContext for MockToolContext {
        fn get_state(&self, _key: &str) -> Option<Value> {
            None
        }
        fn set_state(&self, _key: String, _value: Value) {}
    }

    impl ToolContext for MockToolContext {
        fn function_call_id(&self) -> String {
            "test-call".to_string()
        }
        fn actions(&self) -> EventActions {
            EventActions::default()
        }
        fn set_actions(&self, _actions: EventActions) {}
    }

    fn make_ctx() -> Arc<dyn ToolContext> {
        Arc::new(MockToolContext)
    }

    // --- Tests ---

    #[tokio::test]
    async fn search_finds_entities() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::sample()));
        let result = tool
            .execute(make_ctx(), json!({"action": "search", "query": "yfinance"}))
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("yfinance"));
        assert!(text.contains("module"));
        assert_eq!(result["count"], 1);
    }

    #[tokio::test]
    async fn search_no_matches() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::empty()));
        let result = tool
            .execute(
                make_ctx(),
                json!({"action": "search", "query": "nonexistent"}),
            )
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("No entities found"));
    }

    #[tokio::test]
    async fn search_filters_by_type() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::sample()));
        let result = tool
            .execute(
                make_ctx(),
                json!({"action": "search", "query": "a", "entity_type": "concept"}),
            )
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("rate_limiting"));
        assert!(!text.contains("yfinance"));
    }

    #[tokio::test]
    async fn neighbors_shows_connections() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::sample()));
        let result = tool
            .execute(
                make_ctx(),
                json!({"action": "neighbors", "entity_name": "yfinance"}),
            )
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("rate_limiting"));
        assert!(text.contains("requires"));
        assert!(text.contains("-->"));
    }

    #[tokio::test]
    async fn neighbors_entity_not_found() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::sample()));
        let result = tool
            .execute(
                make_ctx(),
                json!({"action": "neighbors", "entity_name": "nonexistent"}),
            )
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("not found"));
    }

    #[tokio::test]
    async fn context_returns_entities_and_relationships() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::sample()));
        let result = tool
            .execute(make_ctx(), json!({"action": "context", "query": "finance"}))
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("Context for"));
        assert!(text.contains("yfinance"));
        assert!(text.contains("Relationships"));
    }

    #[tokio::test]
    async fn context_no_matches() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::empty()));
        let result = tool
            .execute(make_ctx(), json!({"action": "context", "query": "nothing"}))
            .await
            .unwrap();
        let text = result["result"].as_str().unwrap();
        assert!(text.contains("No context found"));
    }

    #[tokio::test]
    async fn unknown_action_returns_error() {
        let tool = GraphQueryTool::new(Arc::new(MockGraphStorage::sample()));
        let result = tool
            .execute(make_ctx(), json!({"action": "bad_action"}))
            .await;
        assert!(result.is_err());
    }
}
