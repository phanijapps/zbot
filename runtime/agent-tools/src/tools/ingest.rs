// ============================================================================
// INGEST TOOL
// Enqueue text for background extraction into the knowledge graph.
// ============================================================================

// Public API types — consumed by downstream (gateway) that wires a concrete
// IngestionAccess into the tool. No in-crate caller yet.
#![allow(dead_code)]

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use zero_core::{Result, Tool, ToolContext, ZeroError};

/// Abstraction over the gateway's ingestion pipeline. The gateway implements
/// this trait backing the `IngestionQueue` + episode-repository flow.
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
}

/// Tool that enqueues a document or text snippet for background extraction
/// into the knowledge graph.
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
        "Enqueue a document or text for background extraction into the knowledge graph. \
         Returns immediately with the episode count; work happens asynchronously."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "source_id": {
                    "type": "string",
                    "description": "Stable identifier for the source (e.g., 'book-rise-2024')."
                },
                "text": {
                    "type": "string",
                    "description": "Full text to ingest. Will be chunked automatically."
                },
                "source_type": {
                    "type": "string",
                    "description": "Optional source type tag. Defaults to 'document'.",
                    "default": "document"
                }
            },
            "required": ["source_id", "text"]
        }))
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let source_id = args
            .get("source_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'source_id'".to_string()))?;
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'text'".to_string()))?;
        let source_type = args
            .get("source_type")
            .and_then(|v| v.as_str())
            .unwrap_or("document");

        let agent_id = ctx.agent_name().to_string();
        let session_id = ctx.session_id().to_string();
        let session_id_opt = if session_id.is_empty() {
            None
        } else {
            Some(session_id.as_str())
        };

        let (resolved_id, count) = self
            .access
            .enqueue(source_id, source_type, text, session_id_opt, &agent_id)
            .await
            .map_err(ZeroError::Tool)?;

        Ok(json!({
            "source_id": resolved_id,
            "episode_count": count,
            "status": "enqueued",
        }))
    }
}
