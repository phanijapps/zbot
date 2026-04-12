//! Working Memory Middleware — processes tool results to update working memory.
//!
//! Extracts entities from tool output, records discoveries from errors,
//! and tracks delegation status changes.

use super::working_memory::WorkingMemory;
use regex::Regex;
use std::sync::LazyLock;

/// Regex for extracting entity candidates from text.
/// Matches: "quoted strings", PascalCase words, ALLCAPS (3+ chars).
static ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:"([^"]{2,30})"|([A-Z][a-z]+(?:[A-Z][a-z]+)+)|(\b[A-Z]{3,}\b))"#)
        .unwrap_or_else(|_| Regex::new(".^").expect("fallback regex must compile"))
});

/// Process a tool result and update working memory.
pub fn process_tool_result(
    wm: &mut WorkingMemory,
    tool_name: &str,
    result: &str,
    error: Option<&str>,
    iteration: u32,
) {
    // Record errors as discoveries
    if let Some(err) = error {
        let msg = truncate(err, 200);
        wm.add_discovery(&format!("{tool_name} error: {msg}"), iteration, tool_name);
        return;
    }

    // Tool-specific processing
    match tool_name {
        "delegate_to_agent" => handle_delegation_result(wm, result),
        "respond" | "set_session_title" => {} // Final response / metadata — skip
        _ => {
            // Extract entities from tool output (for shell, read, grep, etc.)
            extract_and_add_entities(wm, result, iteration, tool_name);
        }
    }
}

/// Process a delegation start event.
pub fn process_delegation_started(wm: &mut WorkingMemory, agent_id: &str, task: &str) {
    wm.set_delegation_task(agent_id, task);
}

/// Process a delegation completion event.
pub fn process_delegation_completed(wm: &mut WorkingMemory, agent_id: &str, result: &str) {
    let findings = extract_key_lines(result, 3);
    wm.update_delegation(agent_id, "completed", findings);
}

/// Extract key lines from a delegation result (first N non-empty lines).
fn extract_key_lines(text: &str, max_lines: usize) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && l.len() > 10)
        .take(max_lines)
        .map(|l| truncate(l, 100))
        .collect()
}

/// Extract entity candidates from text and add to working memory.
fn extract_and_add_entities(wm: &mut WorkingMemory, text: &str, iteration: u32, source: &str) {
    // Only scan first 2000 chars to keep it fast
    let scan_text = if text.len() > 2000 {
        &text[..2000]
    } else {
        text
    };

    for cap in ENTITY_RE.captures_iter(scan_text) {
        let name = cap
            .get(1)
            .or(cap.get(2))
            .or(cap.get(3))
            .map(|m| m.as_str().to_string());

        if let Some(name) = name {
            // Skip very short or very common words
            if name.len() < 3 || is_common_word(&name) {
                continue;
            }
            // Extract a brief context snippet around the match
            let snippet = extract_context_snippet(scan_text, &name);
            wm.add_entity(&name, None, &snippet, iteration);
        }
    }

    // Check for error-like patterns as discoveries
    if text.contains("Error:") || text.contains("error:") || text.contains("FAILED") {
        if let Some(error_line) = text
            .lines()
            .find(|l| l.contains("Error:") || l.contains("error:") || l.contains("FAILED"))
        {
            wm.add_discovery(&truncate(error_line.trim(), 150), iteration, source);
        }
    }
}

/// Handle delegation tool result — parse agent_id from result JSON.
fn handle_delegation_result(wm: &mut WorkingMemory, result: &str) {
    // delegate_to_agent returns JSON with delegation info
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(result) {
        let agent_id = value
            .get("agent_id")
            .or(value.get("child_agent_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let task = value.get("task").and_then(|v| v.as_str()).unwrap_or("");
        wm.set_delegation_task(agent_id, task);
    }
}

/// Extract a brief context snippet around a name in text.
fn extract_context_snippet(text: &str, name: &str) -> String {
    if let Some(pos) = text.find(name) {
        let start = pos.saturating_sub(20);
        let end = (pos + name.len() + 40).min(text.len());
        let snippet = &text[start..end];
        // Clean up: take the line containing the match
        snippet
            .lines()
            .find(|l| l.contains(name))
            .map(|l| truncate(l.trim(), 80))
            .unwrap_or_else(|| truncate(snippet.trim(), 80))
    } else {
        format!("mentioned in {}", truncate(text, 40))
    }
}

/// Check if a word is too common to be a useful entity.
fn is_common_word(word: &str) -> bool {
    matches!(
        word.to_uppercase().as_str(),
        "THE"
            | "AND"
            | "FOR"
            | "NOT"
            | "THIS"
            | "THAT"
            | "WITH"
            | "FROM"
            | "HAVE"
            | "WILL"
            | "ARE"
            | "BUT"
            | "ALL"
            | "CAN"
            | "HAS"
            | "HER"
            | "WAS"
            | "ONE"
            | "OUR"
            | "OUT"
            | "YOU"
            | "HAD"
            | "HOT"
            | "HIS"
            | "GET"
            | "LET"
            | "SAY"
            | "SHE"
            | "TOO"
            | "USE"
            | "WAY"
            | "WHO"
            | "DID"
            | "ITS"
            | "SET"
            | "TRY"
            | "ASK"
            | "MEN"
            | "RUN"
            | "GOT"
            | "OLD"
            | "END"
            | "NOW"
            | "PUT"
            | "BOX"
            | "ROW"
            | "COL"
            | "KEY"
            | "MAP"
            | "JSON"
            | "HTTP"
            | "URL"
            | "API"
            | "CSS"
            | "HTML"
            | "NONE"
            | "NULL"
            | "TRUE"
            | "FALSE"
            | "SELF"
            | "TODO"
            | "NOTE"
            | "INFO"
            | "WARN"
            | "DEBUG"
    )
}

/// Truncate a string to max_len.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_extraction_quoted() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, r#"Using "West Bengal" data source"#, 1, "shell");
        assert!(!wm.is_empty());
        let output = wm.format_for_prompt();
        assert!(output.contains("West Bengal"));
    }

    #[test]
    fn test_entity_extraction_pascal_case() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, "Use MultiIndex from DataFrame", 1, "shell");
        let output = wm.format_for_prompt();
        assert!(output.contains("MultiIndex"));
    }

    #[test]
    fn test_entity_extraction_allcaps() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, "Analyzing AAPL and TSLA stocks", 1, "shell");
        let output = wm.format_for_prompt();
        assert!(output.contains("AAPL"));
        assert!(output.contains("TSLA"));
    }

    #[test]
    fn test_common_words_filtered() {
        let mut wm = WorkingMemory::new(5000);
        extract_and_add_entities(&mut wm, "THE JSON HTTP API", 1, "shell");
        // All common words — nothing should be added
        assert!(wm.is_empty());
    }

    #[test]
    fn test_error_recorded_as_discovery() {
        let mut wm = WorkingMemory::new(5000);
        process_tool_result(&mut wm, "shell", "", Some("Connection refused"), 3);
        let output = wm.format_for_prompt();
        assert!(output.contains("shell error: Connection refused"));
    }

    #[test]
    fn test_delegation_started() {
        let mut wm = WorkingMemory::new(5000);
        process_delegation_started(&mut wm, "research-agent", "fetch stock data");
        let output = wm.format_for_prompt();
        assert!(output.contains("research-agent"));
        assert!(output.contains("running"));
    }

    #[test]
    fn test_delegation_completed() {
        let mut wm = WorkingMemory::new(5000);
        process_delegation_started(&mut wm, "research-agent", "fetch stock data");
        process_delegation_completed(
            &mut wm,
            "research-agent",
            "Found 8 news sources\nSaved to ward\nAnalysis complete",
        );
        let output = wm.format_for_prompt();
        assert!(output.contains("completed"));
        assert!(output.contains("Found 8 news sources"));
    }

    #[test]
    fn test_extract_key_lines() {
        let text = "Too short\nA longer line that has real content here\nAnother meaningful line with data\nShort";
        let lines = extract_key_lines(text, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("longer line"));
    }

    #[test]
    fn test_respond_tool_skipped() {
        let mut wm = WorkingMemory::new(5000);
        process_tool_result(&mut wm, "respond", "Here is the final answer", None, 10);
        assert!(wm.is_empty());
    }
}
