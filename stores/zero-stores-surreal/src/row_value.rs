//! Helpers for handling SurrealDB rows that come back as `serde_json::Value`.
//!
//! We deliberately read SCHEMALESS rows as `Value` (not via `SurrealValue`)
//! because the SDK's typed deserializer rejects literal `null` even on
//! `Option<T>`, and SCHEMALESS payloads that we round-trip from the runtime
//! routinely carry nulls on optional fields.

use serde_json::Value;

/// SurrealDB returns the row id as either a string ("`table:key`") or an
/// object (`{ "tb": "...", "id": ... }`) depending on the engine path.
/// HTTP handlers expect a plain string id with the table prefix stripped,
/// matching what SQLite emits. This helper normalises that.
pub fn flatten_record_id(mut row: Value) -> Value {
    if let Some(obj) = row.as_object_mut() {
        if let Some(id_val) = obj.remove("id") {
            obj.insert("id".to_string(), Value::String(extract_id_key(&id_val)));
        }
    }
    row
}

fn extract_id_key(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        // "table:key" → "key"; bare strings pass through.
        let raw = if let Some((_, key)) = s.split_once(':') {
            key
        } else {
            s
        };
        // Surreal serialises non-trivial keys as `` `key` ``.
        return raw.trim_matches('`').to_string();
    }
    if let Some(obj) = v.as_object() {
        // Surreal `Thing` shape: { "tb": "table", "id": <key> }
        if let Some(inner) = obj.get("id") {
            return extract_id_key(inner);
        }
        if let Some(s) = obj.get("String").and_then(|x| x.as_str()) {
            return s.to_string();
        }
    }
    v.to_string().trim_matches('"').trim_matches('`').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flatten_string_id_with_table_prefix() {
        let row = json!({ "id": "goal:g1", "name": "x" });
        let out = flatten_record_id(row);
        assert_eq!(out["id"], "g1");
    }

    #[test]
    fn flatten_thing_object_id() {
        let row = json!({
            "id": { "tb": "goal", "id": { "String": "g1" } },
            "name": "x"
        });
        let out = flatten_record_id(row);
        assert_eq!(out["id"], "g1");
    }

    #[test]
    fn flatten_bare_string_id() {
        let row = json!({ "id": "g1", "name": "x" });
        let out = flatten_record_id(row);
        assert_eq!(out["id"], "g1");
    }

    #[test]
    fn flatten_backtick_wrapped_key() {
        let row = json!({ "id": "episode:`ep-first`", "name": "x" });
        let out = flatten_record_id(row);
        assert_eq!(out["id"], "ep-first");
    }
}
