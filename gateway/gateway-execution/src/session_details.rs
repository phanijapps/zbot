//! Redacted, durable session details for the conversation shell.

use std::collections::HashSet;

use api_logs::{ExecutionLog, LogCategory};
use serde::Serialize;

const ACTIVITY_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    Chat,
    Research,
    Unknown,
}

impl SessionMode {
    fn from_stored(value: Option<&str>) -> Self {
        match value {
            Some("fast" | "chat") => Self::Chat,
            Some("deep" | "research") => Self::Research,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Plan,
    Tool,
    Delegation,
    Error,
    MemoryRecall,
    MemoryWrite,
    Hook,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityRecord {
    pub id: String,
    pub sequence: usize,
    pub kind: ActivityKind,
    pub label: &'static str,
    pub occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<api_logs::HookActivityMetadata>,
}

#[derive(Debug, Serialize)]
pub struct SourceRecord {
    pub id: String,
    pub title: String,
    pub url: String,
    pub evidence: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetails {
    pub session_id: String,
    pub mode: SessionMode,
    pub activity: Vec<ActivityRecord>,
    pub activity_truncated: bool,
    pub sources: Vec<SourceRecord>,
    pub sources_truncated: bool,
}

impl SessionDetails {
    /// Project only attributable, allowlisted fields from persisted logs.
    /// No durable structured citation record is available yet, so sources
    /// intentionally remains empty rather than mining URLs from prose/results.
    pub fn project(session_id: &str, mode: Option<&str>, logs: &[ExecutionLog]) -> Self {
        let mut ordered: Vec<_> = logs
            .iter()
            .filter(|log| belongs_to_session(&log.conversation_id, session_id))
            .filter(|log| safe_id(&log.id))
            .filter(|log| chrono::DateTime::parse_from_rfc3339(&log.timestamp).is_ok())
            .filter_map(|log| activity_class(log).map(|(kind, label)| (log, kind, label)))
            .collect();
        ordered.sort_by(|(left, _, _), (right, _, _)| {
            chrono::DateTime::parse_from_rfc3339(&left.timestamp)
                .ok()
                .cmp(&chrono::DateTime::parse_from_rfc3339(&right.timestamp).ok())
                .then_with(|| left.id.cmp(&right.id))
        });

        let mut seen = HashSet::new();
        let mut activity: Vec<_> = ordered
            .into_iter()
            .filter(|(log, _, _)| seen.insert(log.id.as_str()))
            .enumerate()
            .map(|(sequence, (log, kind, label))| ActivityRecord {
                id: log.id.clone(),
                sequence,
                kind,
                label,
                occurred_at: log.timestamp.clone(),
                hook: api_logs::HookActivityMetadata::from_log(log),
            })
            .collect();
        let activity_truncated = activity.len() > ACTIVITY_LIMIT;
        if activity_truncated {
            activity.drain(..activity.len() - ACTIVITY_LIMIT);
        }

        Self {
            session_id: session_id.to_owned(),
            mode: SessionMode::from_stored(mode),
            activity,
            activity_truncated,
            sources: Vec::new(),
            sources_truncated: false,
        }
    }
}

fn belongs_to_session(conversation_id: &str, session_id: &str) -> bool {
    if conversation_id == session_id {
        return true;
    }
    conversation_id
        .strip_prefix(session_id)
        .and_then(|tail| tail.strip_prefix("-cont-"))
        .is_some_and(|suffix| {
            suffix.len() == 8 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// The public contract's 1–128-byte ASCII identifier grammar.
pub fn safe_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(byte))
}

fn activity_class(log: &ExecutionLog) -> Option<(ActivityKind, &'static str)> {
    if let Some(hook) = api_logs::HookActivityMetadata::from_log(log) {
        return Some((ActivityKind::Hook, hook.status.label()));
    }
    match log.category {
        LogCategory::ToolCall => match log
            .metadata
            .as_ref()
            .and_then(|value| value.get("tool_name"))
            .and_then(serde_json::Value::as_str)
        {
            Some("update_plan") => Some((ActivityKind::Plan, "Plan updated")),
            Some("recall" | "memory_recall") => {
                Some((ActivityKind::MemoryRecall, "Memory recalled"))
            }
            Some("memory_write") => Some((ActivityKind::MemoryWrite, "Memory written")),
            _ => Some((ActivityKind::Tool, "Tool called")),
        },
        LogCategory::Delegation => Some((ActivityKind::Delegation, "Delegation event")),
        LogCategory::Error => Some((ActivityKind::Error, "Error recorded")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_logs::{ExecutionLog, LogCategory, LogLevel};

    fn log(id: &str, category: LogCategory, message: &str) -> ExecutionLog {
        let mut row = ExecutionLog::new(
            "exec-1",
            "sess-1",
            "root",
            LogLevel::Info,
            category,
            message,
        );
        row.id = id.to_owned();
        row.timestamp = "2026-09-27T12:00:00Z".to_owned();
        row
    }

    // STUB: AC2 — persisted mode is normalized, never inferred.
    #[test]
    fn mode_spellings_and_unknown() {
        for (stored, expected) in [
            (Some("fast"), "chat"),
            (Some("chat"), "chat"),
            (Some("deep"), "research"),
            (Some("research"), "research"),
            (None, "unknown"),
            (Some("other"), "unknown"),
        ] {
            let value =
                serde_json::to_value(SessionDetails::project("sess-1", stored, &[])).unwrap();
            assert_eq!(value["mode"], expected);
        }
    }

    // STUB: AC4 AC6 AC12 — only durable, safe, redacted activity survives.
    #[test]
    fn attributable_activity_is_ordered_deduplicated_and_redacted() {
        let logs = [
            log("event-2", LogCategory::Error, "secret /home/user/private"),
            log("event-1", LogCategory::ToolCall, "raw memory payload"),
            log("event-1", LogCategory::ToolCall, "duplicate"),
            log("bad id!", LogCategory::Delegation, "unsafe"),
        ];
        let value =
            serde_json::to_value(SessionDetails::project("sess-1", Some("deep"), &logs)).unwrap();
        let activity = value["activity"].as_array().unwrap();
        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0]["id"], "event-1");
        assert_eq!(activity[1]["id"], "event-2");
        let text = value.to_string();
        assert!(!text.contains("secret"));
        assert!(!text.contains("raw memory payload"));
        assert!(!text.contains("bad id!"));
    }

    // STUB: AC4 AC5 — bounded windows advertise truncation; no source is inferred.
    #[test]
    fn activity_window_is_bounded_and_sources_remain_evidence_only() {
        let logs: Vec<_> = (0..501)
            .map(|n| {
                log(
                    &format!("event-{n}"),
                    LogCategory::ToolCall,
                    "https://example.com",
                )
            })
            .collect();
        let value =
            serde_json::to_value(SessionDetails::project("sess-1", Some("fast"), &logs)).unwrap();
        assert_eq!(value["activity"].as_array().unwrap().len(), 500);
        assert_eq!(value["activityTruncated"], true);
        assert_eq!(value["sources"].as_array().unwrap().len(), 0);
        assert_eq!(value["sourcesTruncated"], false);
    }

    // STUB: AC4 — a continuation turn belongs to its canonical session.
    #[test]
    fn continuation_activity_survives_projection_without_cross_session_rows() {
        let mut continued = log("event-cont", LogCategory::ToolCall, "turn two");
        continued.conversation_id = "sess-1-cont-a1b2c3d4".to_owned();
        let mut unrelated = log("event-other", LogCategory::ToolCall, "other");
        unrelated.conversation_id = "sess-1-other".to_owned();
        let value = serde_json::to_value(SessionDetails::project(
            "sess-1",
            Some("deep"),
            &[continued, unrelated],
        ))
        .unwrap();
        assert_eq!(value["activity"].as_array().unwrap().len(), 1);
        assert_eq!(value["activity"][0]["id"], "event-cont");
    }

    #[test]
    fn malformed_or_private_hook_metadata_never_projects() {
        let metadata = serde_json::json!({"hookId":"observe","event":"run_end","eventId":"event-1",
            "invocationId":"invocation-1","agentId":"child","runId":null,"status":"completed","durationMs":12,"exitCode":0});
        let mut valid = log(
            "hook-valid",
            LogCategory::System,
            "/private/script.py secret",
        );
        valid.metadata = Some(serde_json::json!({"hook":metadata}));
        let mut unknown = valid.clone();
        unknown.id = "hook-unknown".into();
        unknown.metadata.as_mut().unwrap()["hook"]["reason"] = "private sentinel".into();
        let mut unsafe_id = valid.clone();
        unsafe_id.id = "hook-unsafe".into();
        unsafe_id.metadata.as_mut().unwrap()["hook"]["agentId"] = "/private/script.py".into();
        let mut missing = valid.clone();
        missing.id = "hook-missing".into();
        missing.metadata.as_mut().unwrap()["hook"]
            .as_object_mut()
            .unwrap()
            .remove("runId");
        let projected = serde_json::to_value(SessionDetails::project(
            "sess-1",
            Some("fast"),
            &[valid, unknown, unsafe_id, missing],
        ))
        .unwrap();
        assert_eq!(projected["activity"].as_array().unwrap().len(), 1);
        assert_eq!(projected["activity"][0]["kind"], "hook");
        assert_eq!(projected["activity"][0]["label"], "Hook completed");
        assert!(!projected.to_string().contains("private"));
        assert!(!projected.to_string().contains("secret"));
    }
}
