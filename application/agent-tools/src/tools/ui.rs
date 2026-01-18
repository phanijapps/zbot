// ============================================================================
// UI TOOLS
// RequestInput and ShowContent tools
// ============================================================================

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use zero_core::{Tool, ToolContext, Result};

// ============================================================================
// REQUEST INPUT TOOL
// ============================================================================

/// Tool for requesting structured user input
pub struct RequestInputTool;

#[async_trait]
impl Tool for RequestInputTool {
    fn name(&self) -> &str {
        "request_input"
    }

    fn description(&self) -> &str {
        "IMPORTANT: Use this tool whenever you need to collect 2+ pieces of related information from the user. Instead of asking multiple separate questions in plain text, use this tool to request all information at once via a form. This provides a better user experience."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "form_id": {
                    "type": "string",
                    "description": "Unique identifier for this form"
                },
                "title": {
                    "type": "string",
                    "description": "Form title"
                },
                "description": {
                    "type": "string",
                    "description": "Form description"
                },
                "schema": {
                    "type": "object",
                    "description": "JSON Schema for the form"
                }
            },
            "required": ["form_id", "title", "schema"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let form_id = args.get("form_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'form_id' parameter".to_string()))?
            .to_string();

        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'title' parameter".to_string()))?
            .to_string();

        let description = args.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());

        let schema = args.get("schema")
            .cloned()
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'schema' parameter".to_string()))?;

        let submit_button = args.get("submit_button").and_then(|v| v.as_str()).map(|s| s.to_string());

        tracing::debug!("Requesting input form: {}", form_id);

        // Return the form request with the special marker
        Ok(json!({
            "__request_input": true,
            "form_id": form_id,
            "form_type": "json_schema",
            "title": title,
            "description": description,
            "schema": schema,
            "submit_button": submit_button
        }))
    }
}

// ============================================================================
// SHOW CONTENT TOOL
// ============================================================================

/// Tool for displaying content in the UI
pub struct ShowContentTool;

#[async_trait]
impl Tool for ShowContentTool {
    fn name(&self) -> &str {
        "show_content"
    }

    fn description(&self) -> &str {
        "IMPORTANT: Use this tool to display saved content (HTML, PDF, images, markdown, etc.) in a specialized viewer. ALWAYS save content first using write, then use this tool to display it. This provides a much better user experience than displaying raw content."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "content_type": {
                    "type": "string",
                    "description": "Type of content (pdf, image, html, text, etc.)"
                },
                "title": {
                    "type": "string",
                    "description": "Title for the content"
                },
                "content": {
                    "type": "string",
                    "description": "The content to display (or file path)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to a previously saved file"
                }
            },
            "required": ["content_type", "title"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        let content_type = args.get("content_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'content_type' parameter".to_string()))?
            .to_string();

        let title = args.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| zero_core::ZeroError::Tool("Missing 'title' parameter".to_string()))?
            .to_string();

        let content = args.get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let file_path = args.get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let metadata = args.get("metadata").cloned();

        let is_attachment = args.get("is_attachment")
            .and_then(|v| v.as_bool());

        let base64 = args.get("base64")
            .and_then(|v| v.as_bool());

        tracing::debug!("Showing content: type={}, title={}", content_type, title);

        // Return the content display request with the special marker
        Ok(json!({
            "__show_content": true,
            "content_type": content_type,
            "title": title,
            "content": content.unwrap_or_default(),
            "file_path": file_path,
            "metadata": metadata,
            "is_attachment": is_attachment,
            "base64": base64
        }))
    }
}
