use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEventName {
    SessionStart,
    UserPrompt,
    RunStart,
    RunEnd,
    BeforeModel,
    AfterModel,
    BeforeTool,
    AfterTool,
    InvalidToolCall,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookActivityStatus {
    Running,
    Completed,
    Blocked,
    Failed,
    Timeout,
    Cancelled,
    Skipped,
}
impl HookActivityStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Hook running",
            Self::Completed => "Hook completed",
            Self::Blocked => "Hook blocked",
            Self::Failed => "Hook failed",
            Self::Timeout => "Hook timed out",
            Self::Cancelled => "Hook cancelled",
            Self::Skipped => "Hook skipped",
        }
    }
}
/// Only fields admitted by the public HookActivity contract can reach storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookActivityMetadata {
    pub hook_id: String,
    #[serde(deserialize_with = "strict_enum_string")]
    pub event: HookEventName,
    pub event_id: String,
    pub invocation_id: String,
    pub agent_id: String,
    pub run_id: Option<String>,
    #[serde(deserialize_with = "strict_enum_string")]
    pub status: HookActivityStatus,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i32>,
}
fn strict_enum_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = String::deserialize(deserializer)?;
    serde_json::from_value(serde_json::Value::String(value)).map_err(serde::de::Error::custom)
}
pub fn hook_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}
impl HookActivityMetadata {
    pub fn validate(&self) -> bool {
        !self.hook_id.is_empty()
            && self.hook_id.len() <= 64
            && self.hook_id.as_bytes()[0].is_ascii_lowercase()
            && self
                .hook_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && [&self.event_id, &self.invocation_id, &self.agent_id]
                .into_iter()
                .all(|value| hook_safe_id(value))
            && self.run_id.as_deref().is_none_or(hook_safe_id)
            && self
                .duration_ms
                .is_none_or(|value| value <= i64::MAX as u64)
    }
    pub fn from_log(log: &crate::ExecutionLog) -> Option<Self> {
        if log.category != crate::LogCategory::System {
            return None;
        }
        let metadata = log.metadata.as_ref()?;
        if metadata.as_object()?.len() != 1 {
            return None;
        }
        if metadata.get("hook")?.as_object()?.len() != 9 {
            return None;
        }
        let hook: Self = serde_json::from_value(metadata.get("hook")?.clone()).ok()?;
        hook.validate().then_some(hook)
    }
}
pub struct HookLogRecord {
    pub id: String,
    pub execution_id: String,
    /// Canonical root session, including for delegated child observations.
    pub session_id: String,
    pub occurred_at: String,
    pub hook: HookActivityMetadata,
}
impl HookLogRecord {
    pub(crate) fn validated_log(&self) -> Result<crate::ExecutionLog, String> {
        if ![&self.id, &self.execution_id, &self.session_id]
            .into_iter()
            .all(|value| hook_safe_id(value))
            || !self.hook.validate()
            || self.occurred_at.len() > 64
            || chrono::DateTime::parse_from_rfc3339(&self.occurred_at).is_err()
        {
            return Err("Invalid hook activity metadata".into());
        }
        let mut log = crate::ExecutionLog::new(
            &self.execution_id,
            &self.session_id,
            &self.hook.agent_id,
            crate::LogLevel::Info,
            crate::LogCategory::System,
            self.hook.status.label(),
        );
        log.id.clone_from(&self.id);
        log.timestamp.clone_from(&self.occurred_at);
        log.metadata = Some(serde_json::json!({"hook": self.hook}));
        log.duration_ms = self.hook.duration_ms.map(|value| value as i64);
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbProvider, LogService, SCHEMA_SQL};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};
    struct Db(Mutex<Connection>);
    impl DbProvider for Db {
        fn with_connection<F, R>(&self, f: F) -> Result<R, String>
        where
            F: FnOnce(&Connection) -> Result<R, rusqlite::Error>,
        {
            f(&self.0.lock().unwrap()).map_err(|_| "storage unavailable".into())
        }
    }
    fn record(id: &str, status: HookActivityStatus) -> HookLogRecord {
        HookLogRecord {
            id: id.into(),
            execution_id: "exec-child".into(),
            session_id: "sess-root".into(),
            occurred_at: "2026-10-04T12:00:00.000001Z".into(),
            hook: HookActivityMetadata {
                hook_id: "observe".into(),
                event: HookEventName::RunEnd,
                event_id: "event-1".into(),
                invocation_id: "invocation-1".into(),
                agent_id: "child".into(),
                run_id: Some("run-1".into()),
                status,
                duration_ms: Some(12),
                exit_code: Some(0),
            },
        }
    }
    fn setup() -> (Arc<Db>, LogService<Db>) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();
        let db = Arc::new(Db(Mutex::new(conn)));
        let service = LogService::new(db.clone());
        (db, service)
    }
    #[test]
    fn hooks_upsert_preserves_identity_start_time_and_recovers_without_replay() {
        let (db, service) = setup();
        let running = record("activity-1", HookActivityStatus::Running);
        service.log_hook_activity(&running).unwrap();
        let mut settled = record("activity-1", HookActivityStatus::Completed);
        settled.occurred_at = "2026-10-04T13:00:00Z".into();
        service.log_hook_activity(&settled).unwrap();
        service.log_hook_activity(&running).unwrap();
        service
            .log_hook_activity(&record("activity-2", HookActivityStatus::Running))
            .unwrap();
        assert_eq!(service.recover_interrupted_hook_activity().unwrap(), 1);
        assert_eq!(service.recover_interrupted_hook_activity().unwrap(), 0);
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT timestamp, conversation_id, metadata FROM execution_logs ORDER BY id",
            )?;
            let rows: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<Result<_, _>>()?;
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].0, running.occurred_at);
            assert_eq!(rows[0].1, "sess-root");
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&rows[0].2).unwrap()["hook"]["status"],
                "completed"
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&rows[1].2).unwrap()["hook"]["status"],
                "cancelled"
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn invalid_metadata_is_rejected_before_persistence() {
        let (db, service) = setup();
        for bad in ["/private/script.py", "bad id", &"x".repeat(129)] {
            let mut row = record("activity-1", HookActivityStatus::Completed);
            row.hook.agent_id = bad.into();
            assert!(service.log_hook_activity(&row).is_err());
        }
        let mut row = record("activity-1", HookActivityStatus::Completed);
        row.hook.hook_id = "bad_id".into();
        assert!(service.log_hook_activity(&row).is_err());
        let mut value =
            serde_json::to_value(record("activity-1", HookActivityStatus::Completed).hook).unwrap();
        value["reason"] = "secret /private/script.py".into();
        assert!(serde_json::from_value::<HookActivityMetadata>(value).is_err());
        for field in ["event", "status"] {
            let mut value =
                serde_json::to_value(record("activity-1", HookActivityStatus::Completed).hook)
                    .unwrap();
            value[field] = serde_json::json!({"completed":null});
            assert!(serde_json::from_value::<HookActivityMetadata>(value).is_err());
        }
        db.with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT count(*) FROM execution_logs", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    }
}
