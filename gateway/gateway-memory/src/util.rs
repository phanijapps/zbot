//! Shared helpers for parsing JSON-in-content responses from LLM calls.
//!
//! Three patterns recur across the codebase:
//!  1. Strip optional ```json``` code fences
//!  2. serde_json::from_str into a typed struct
//!  3. Truncate the input for error previews so log lines stay readable
//!
//! `parse_llm_json::<T>(content)` does all three.

use serde::de::DeserializeOwned;

/// Strip optional Markdown code fences from an LLM response.
/// Handles ```json … ```, ``` … ```, or no fences at all.
pub fn strip_code_fence(s: &str) -> &str {
    let t = s.trim();
    let t = t
        .strip_prefix("```json")
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t)
        .trim();
    t.strip_suffix("```").unwrap_or(t).trim()
}

/// Parse JSON-in-content from an LLM response into a typed struct.
/// Strips code fences first; on parse failure includes a 200-char
/// preview of the original content for debugging.
pub fn parse_llm_json<T: DeserializeOwned>(content: &str) -> Result<T, String> {
    let stripped = strip_code_fence(content);
    serde_json::from_str(stripped).map_err(|e| {
        let preview: String = content.chars().take(200).collect();
        format!("parse LLM JSON: {e} (preview: {preview})")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Sample {
        name: String,
        count: i64,
    }

    #[test]
    fn parses_clean_json() {
        let s = r#"{"name": "alpha", "count": 3}"#;
        let r: Sample = parse_llm_json(s).unwrap();
        assert_eq!(
            r,
            Sample {
                name: "alpha".into(),
                count: 3
            }
        );
    }

    #[test]
    fn strips_json_code_fence() {
        let s = "```json\n{\"name\": \"beta\", \"count\": 7}\n```";
        let r: Sample = parse_llm_json(s).unwrap();
        assert_eq!(r.name, "beta");
    }

    #[test]
    fn strips_plain_code_fence() {
        let s = "```\n{\"name\": \"gamma\", \"count\": 1}\n```";
        let r: Sample = parse_llm_json(s).unwrap();
        assert_eq!(r.name, "gamma");
    }

    #[test]
    fn errors_with_preview_on_malformed() {
        let s = "not valid json at all here";
        let r: Result<Sample, String> = parse_llm_json(s);
        let err = r.unwrap_err();
        assert!(
            err.contains("preview"),
            "error should include preview, got: {err}"
        );
        assert!(
            err.contains("not valid json"),
            "preview should contain head of input"
        );
    }

    #[test]
    fn handles_empty_string() {
        let r: Result<Sample, String> = parse_llm_json("");
        assert!(r.is_err());
    }
}
