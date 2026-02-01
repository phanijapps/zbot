//! # Schema Sanitization
//!
//! Sanitizes MCP tool schemas for LLM compatibility.

use serde_json::Value;
use tracing::debug;

/// Sanitizes a tool schema for LLM compatibility.
///
/// Removes fields that LLM providers may not understand:
/// - `$schema` - JSON Schema version identifier
/// - `definitions` - Schema definitions/references
/// - `$ref` - Schema references
/// - `default` - Default values (not needed for function calling)
/// - `examples` - Example values (LLMs don't use these)
/// - `additionalProperties` when true (redundant)
///
/// # Arguments
///
/// * `schema` - The original schema from MCP server
/// * `server_id` - Server ID for error reporting
/// * `tool_name` - Tool name for error reporting
///
/// # Returns
///
/// A sanitized schema compatible with LLM function calling.
pub fn sanitize_tool_schema(
    schema: &Value,
    server_id: &str,
    tool_name: &str,
) -> Value {
    let mut sanitized = schema.clone();
    sanitize_value(&mut sanitized);
    debug!(
        "Sanitized schema for {}.{}: removed incompatible fields",
        server_id, tool_name
    );
    sanitized
}

/// Recursively sanitize a JSON value.
fn sanitize_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // Remove incompatible keys
            map.remove("$schema");
            map.remove("definitions");
            map.remove("$ref");
            map.remove("examples");

            // Remove default if it's complex (simple defaults are ok)
            if let Some(default) = map.get("default") {
                if is_complex_value(default) {
                    map.remove("default");
                }
            }

            // Remove additionalProperties if explicitly true (redundant)
            if let Some(Value::Bool(true)) = map.get("additionalProperties") {
                map.remove("additionalProperties");
            }

            // Recursively sanitize nested values
            for (_, v) in map.iter_mut() {
                sanitize_value(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                sanitize_value(v);
            }
        }
        _ => {}
    }
}

/// Check if a value is "complex" (not a primitive).
fn is_complex_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Object(_) | Value::Array(_) | Value::Null
    )
}

/// Extracts the relevant schema content for a tool.
///
/// Some MCP servers wrap schemas in additional metadata.
/// This extracts just the input schema.
pub fn extract_input_schema(tool_schema: &Value) -> Option<Value> {
    // Check for common wrapper patterns
    if let Some(input) = tool_schema.get("inputSchema") {
        return Some(input.clone());
    }

    // If the root is an object with type "object", it's probably the schema
    if let Some(obj) = tool_schema.as_object() {
        if obj.get("type").and_then(|t| t.as_str()) == Some("object") {
            return Some(tool_schema.clone());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitize_removes_dollar_schema() {
        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert!(sanitized.get("$schema").is_none());
        assert_eq!(sanitized["type"], "object");
    }

    #[test]
    fn test_sanitize_removes_definitions() {
        let schema = json!({
            "type": "object",
            "definitions": {
                "address": {
                    "type": "object",
                    "properties": {
                        "street": {"type": "string"}
                    }
                }
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert!(sanitized.get("definitions").is_none());
    }

    #[test]
    fn test_sanitize_removes_ref() {
        let schema = json!({
            "type": "object",
            "properties": {
                "ref_field": {"$ref": "#/definitions/address"}
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert!(sanitized["properties"]["ref_field"].get("$ref").is_none());
    }

    #[test]
    fn test_sanitize_removes_examples() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "examples": ["John", "Jane"]
                }
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert!(sanitized["properties"]["name"].get("examples").is_none());
    }

    #[test]
    fn test_sanitize_removes_complex_default() {
        let schema = json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "default": {"key": "value"}
                }
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert!(sanitized["properties"]["config"].get("default").is_none());
    }

    #[test]
    fn test_sanitize_keeps_simple_default() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "default": 0
                }
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert_eq!(sanitized["properties"]["count"]["default"], 0);
    }

    #[test]
    fn test_sanitize_removes_additional_properties_true() {
        let schema = json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "name": {"type": "string"}
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert!(sanitized.get("additionalProperties").is_none());
    }

    #[test]
    fn test_sanitize_keeps_additional_properties_false() {
        let schema = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "name": {"type": "string"}
            }
        });

        let sanitized = sanitize_tool_schema(&schema, "test", "test_tool");
        assert_eq!(sanitized["additionalProperties"], false);
    }

    #[test]
    fn test_extract_input_schema_wrapped() {
        let tool_schema = json!({
            "name": "test_tool",
            "description": "A test tool",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "arg": {"type": "string"}
                }
            }
        });

        let extracted = extract_input_schema(&tool_schema);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap()["type"], "object");
    }

    #[test]
    fn test_extract_input_schema_direct() {
        let tool_schema = json!({
            "type": "object",
            "properties": {
                "arg": {"type": "string"}
            }
        });

        let extracted = extract_input_schema(&tool_schema);
        assert!(extracted.is_some());
        assert_eq!(extracted.unwrap()["type"], "object");
    }

    #[test]
    fn test_extract_input_schema_none() {
        let tool_schema = json!({
            "name": "test_tool",
            "description": "A test tool"
        });

        let extracted = extract_input_schema(&tool_schema);
        assert!(extracted.is_none());
    }
}
