// ============================================================================
// INGEST TOOL
// Bulk-structured graph writes + text ingest in a single polymorphic tool.
// ============================================================================

// Public API types — consumed by downstream (gateway) that wires a concrete
// IngestionAccess into the tool. No in-crate caller yet.
#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use zero_core::{Result, Tool, ToolContext, ZeroError};

// ---------------------------------------------------------------------------
// Public shapes
// ---------------------------------------------------------------------------

/// Generic entity shape accepted by the structured path. `type` and the free-
/// form `properties` blob let wards encode their own vocabulary without
/// needing to register schemas centrally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredEntity {
    /// Stable slug — the dedup key. Reusing the same id across sources MERGES
    /// properties into one node (evidence arrays concatenate).
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

/// Generic relationship shape. `from` and `to` reference entity ids — either
/// ids listed in the same payload's `entities` array or existing graph ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredRelationship {
    #[serde(rename = "type")]
    pub rel_type: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

/// Counts returned from a structured ingest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredCounts {
    pub entities_upserted: usize,
    pub relationships_upserted: usize,
}

// ---------------------------------------------------------------------------
// Backend abstraction
// ---------------------------------------------------------------------------

/// Abstraction over the gateway's ingestion pipeline. Implementations bridge
/// to the episode repository (for text) and the graph storage (for structured).
#[async_trait]
pub trait IngestionAccess: Send + Sync + 'static {
    /// Chunk `text`, create one episode per chunk, and notify workers.
    /// Returns `(source_id, episode_count)`.
    async fn enqueue(
        &self,
        source_id: &str,
        source_type: &str,
        text: &str,
        session_id: Option<&str>,
        agent_id: &str,
    ) -> std::result::Result<(String, usize), String>;

    /// Bulk-upsert structured entities and relationships. No LLM extraction.
    /// The agent is responsible for the shape; backend merges into existing
    /// rows by id (entities) and by (source, target, type) (relationships).
    async fn ingest_structured(
        &self,
        agent_id: &str,
        entities: Vec<StructuredEntity>,
        relationships: Vec<StructuredRelationship>,
    ) -> std::result::Result<StructuredCounts, String>;
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

/// Polymorphic ingest tool: text (LLM extracts asynchronously), structured
/// (direct bulk write), or both in the same call.
pub struct IngestTool {
    access: Arc<dyn IngestionAccess>,
}

impl IngestTool {
    pub fn new(access: Arc<dyn IngestionAccess>) -> Self {
        Self { access }
    }
}

#[async_trait]
impl Tool for IngestTool {
    fn name(&self) -> &str {
        "ingest"
    }

    fn description(&self) -> &str {
        "Write knowledge to the graph. Two modes, usable together in a single call: \
         (a) TEXT — pass `text` (and a `source_id` for provenance); a background \
         LLM extractor will chunk, extract entities and relationships, and upsert \
         asynchronously. \
         (b) STRUCTURED — pass `entities[]` and/or `relationships[]` directly and \
         they are bulk-upserted into the graph synchronously (no LLM). \
         \
         Entity shape: `{id, name, type, properties?}`. Use stable slug ids like \
         'person:steve-jobs' so the same entity across sources merges into one \
         node. Evidence arrays in properties append across ingests. \
         Relationship shape: `{type, from, to, properties?}` where from/to are \
         entity ids from the same payload or existing graph ids. \
         All fields are arrays — send one item or many."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "source_id": {
                    "type": "string",
                    "description": "Optional provenance tag for this batch (e.g., 'book-jobs-bio')."
                },
                "source_type": {
                    "type": "string",
                    "description": "Optional tag — 'book', 'paper', 'earnings_call', etc. Defaults to 'document'.",
                    "default": "document"
                },
                "text": {
                    "type": "string",
                    "description": "Prose to enqueue for background LLM extraction. Optional."
                },
                "entities": {
                    "type": "array",
                    "description": "Structured entities to bulk-upsert. Stable ids merge into existing nodes.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id":   { "type": "string" },
                            "name": { "type": "string" },
                            "type": { "type": "string" },
                            "properties": { "type": "object" }
                        },
                        "required": ["id", "name", "type"]
                    }
                },
                "relationships": {
                    "type": "array",
                    "description": "Typed edges. `from`/`to` reference entity ids.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string" },
                            "from": { "type": "string" },
                            "to":   { "type": "string" },
                            "properties": { "type": "object" }
                        },
                        "required": ["type", "from", "to"]
                    }
                }
            }
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let agent_id = ctx.agent_name().to_string();

        let entities: Vec<StructuredEntity> = args
            .get("entities")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let relationships: Vec<StructuredRelationship> = args
            .get("relationships")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");

        if entities.is_empty() && relationships.is_empty() && text.is_empty() {
            return Err(ZeroError::Tool(
                "ingest requires at least one of: entities, relationships, or text".into(),
            ));
        }

        // Structured path — synchronous bulk write, runs first so relationships
        // whose `from`/`to` reference entities in the same call see them.
        let counts = if !entities.is_empty() || !relationships.is_empty() {
            self.access
                .ingest_structured(&agent_id, entities, relationships)
                .await
                .map_err(ZeroError::Tool)?
        } else {
            StructuredCounts {
                entities_upserted: 0,
                relationships_upserted: 0,
            }
        };

        // Text path — async background extraction (unchanged behavior).
        let (resolved_source, chunk_count) = if !text.is_empty() {
            let source_id = args
                .get("source_id")
                .and_then(|v| v.as_str())
                .unwrap_or("agent-ingest");
            let source_type = args
                .get("source_type")
                .and_then(|v| v.as_str())
                .unwrap_or("document");
            let session_id = ctx.session_id().to_string();
            let session_id_opt = if session_id.is_empty() {
                None
            } else {
                Some(session_id.as_str())
            };
            self.access
                .enqueue(source_id, source_type, text, session_id_opt, &agent_id)
                .await
                .map_err(ZeroError::Tool)?
        } else {
            (String::new(), 0)
        };

        Ok(json!({
            "entities_upserted": counts.entities_upserted,
            "relationships_upserted": counts.relationships_upserted,
            "text_chunks_enqueued": chunk_count,
            "source_id": resolved_source,
            "status": "ok",
        }))
    }
}
