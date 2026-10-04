//! Narrow session metadata reads for runtime consumers.
//!
//! This provides metadata-only reads without growing a broad conversation facade.

use anyhow::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;

/// Non-secret acceptance identity. Claimed before any external side effect.
#[derive(Debug, Clone)]
pub struct HookInvocationClaim {
    pub invocation_id: String,
    pub session_start: bool,
    pub ingress_required: bool,
    pub consumed_context_bytes: usize,
}

pub trait SessionMetaStore: Send + Sync {
    fn checkpoint_hook_context_bytes(
        &self,
        _session_id: &str,
        _invocation_id: &str,
        _used: usize,
    ) -> Result<()> {
        anyhow::bail!("hook_acceptance_store_unsupported")
    }

    fn record_hook_invocation_identity(
        &self,
        _session_id: &str,
        identity: Option<(&str, &str)>,
    ) -> Result<()> {
        if identity.is_some() {
            anyhow::bail!("hook_acceptance_store_unsupported");
        }
        Ok(())
    }

    fn hook_invocation_identity(&self, _session_id: &str) -> Result<Option<(String, String)>> {
        Ok(None)
    }
    fn claim_hook_invocation(
        &self,
        _session_id: &str,
        _message_id: &str,
        _revision: &str,
        _proposed_id: &str,
        _resume: bool,
    ) -> Result<HookInvocationClaim> {
        anyhow::bail!("hook_acceptance_store_unsupported")
    }

    /// Return the session's active ward id, if one is recorded.
    fn session_ward_id(&self, session_id: &str) -> Result<Option<String>>;

    /// Return the root agent id recorded on the session.
    fn session_agent_id(&self, session_id: &str) -> Result<Option<String>>;
}

pub struct SqliteSessionMetaStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSessionMetaStore {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }
}

impl SessionMetaStore for SqliteSessionMetaStore {
    fn checkpoint_hook_context_bytes(
        &self,
        session_id: &str,
        invocation_id: &str,
        used: usize,
    ) -> Result<()> {
        if used > 8192 {
            anyhow::bail!("hook_context_budget_invalid");
        }
        let mut conn = self.pool.get()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: Option<String> = tx.query_row(
            "SELECT metadata FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )?;
        let mut metadata: serde_json::Value = serde_json::from_str(raw.as_deref().unwrap_or("{}"))?;
        if let Some(marker) = metadata.get_mut("external_hooks") {
            if marker.get("invocation_id").and_then(|v| v.as_str()) == Some(invocation_id) {
                marker
                    .as_object_mut()
                    .ok_or_else(|| anyhow::anyhow!("hook_acceptance_metadata_invalid"))?
                    .insert("consumed_context_bytes".into(), serde_json::json!(used));
                tx.execute(
                    "UPDATE sessions SET metadata=?1 WHERE id=?2",
                    (metadata.to_string(), session_id),
                )?;
            }
        }
        // Superseded live owners remain governed by their in-memory budget;
        // they cannot be restored through the current root acceptance marker.
        tx.commit()?;
        Ok(())
    }

    fn record_hook_invocation_identity(
        &self,
        session_id: &str,
        identity: Option<(&str, &str)>,
    ) -> Result<()> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let raw: Option<String> = tx.query_row(
            "SELECT metadata FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )?;
        let mut metadata = raw
            .map(|raw| serde_json::from_str::<serde_json::Value>(&raw))
            .transpose()?
            .unwrap_or_else(|| serde_json::json!({}));
        let object = metadata
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("hook_acceptance_metadata_invalid"))?;
        match identity {
            Some((id, revision)) => {
                object.insert(
                    "external_hooks".into(),
                    serde_json::json!({"invocation_id":id,"revision":revision,"consumed_context_bytes":0}),
                );
            }
            None => {
                object.remove("external_hooks");
            }
        }
        tx.execute(
            "UPDATE sessions SET metadata=?1 WHERE id=?2",
            (metadata.to_string(), session_id),
        )?;
        tx.commit()?;
        Ok(())
    }

    fn hook_invocation_identity(&self, session_id: &str) -> Result<Option<(String, String)>> {
        let raw: Option<String> = self
            .pool
            .get()?
            .query_row(
                "SELECT metadata FROM sessions WHERE id=?1",
                [session_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        let Some(raw) = raw else { return Ok(None) };
        let metadata: serde_json::Value = serde_json::from_str(&raw)?;
        let Some(marker) = metadata.get("external_hooks") else {
            return Ok(None);
        };
        let id = marker
            .get("invocation_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("hook_acceptance_metadata_invalid"))?;
        let revision = marker
            .get("revision")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("hook_acceptance_metadata_invalid"))?;
        Ok(Some((id.to_owned(), revision.to_owned())))
    }

    fn claim_hook_invocation(
        &self,
        session_id: &str,
        message_id: &str,
        revision: &str,
        proposed_id: &str,
        resume: bool,
    ) -> Result<HookInvocationClaim> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let first: String = tx.query_row(
            "SELECT id FROM messages WHERE session_id=?1 AND role='user' ORDER BY seq,id LIMIT 1",
            [session_id],
            |row| row.get(0),
        )?;
        let raw: Option<String> = tx.query_row(
            "SELECT metadata FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )?;
        let mut metadata = match raw {
            Some(raw) => serde_json::from_str::<serde_json::Value>(&raw)?,
            None => serde_json::json!({}),
        };
        let object = metadata
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("hook_acceptance_metadata_invalid"))?;
        if let Some(marker) = object.get("external_hooks") {
            if marker.get("message_id").and_then(|v| v.as_str()) == Some(message_id) {
                if marker.get("revision").and_then(|v| v.as_str()) != Some(revision) {
                    anyhow::bail!("hook_resume_revision_changed");
                }
                let invocation_id = marker
                    .get("invocation_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("hook_acceptance_metadata_invalid"))?
                    .to_owned();
                return Ok(HookInvocationClaim {
                    invocation_id,
                    session_start: false,
                    ingress_required: false,
                    consumed_context_bytes: marker
                        .get("consumed_context_bytes")
                        .map(|v| {
                            v.as_u64()
                                .filter(|n| *n <= 8192)
                                .map(|n| n as usize)
                                .ok_or_else(|| anyhow::anyhow!("hook_context_budget_invalid"))
                        })
                        .transpose()?
                        .unwrap_or(0),
                });
            }
            if resume {
                anyhow::bail!("hook_resume_identity_expired");
            }
        }
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE id=?1 AND session_id=?2 AND role='user')",
            (message_id, session_id),
            |row| row.get(0),
        )?;
        if !exists {
            anyhow::bail!("hook_acceptance_message_missing");
        }
        // A resume without an earlier claim establishes the first explicit accepted invocation.
        object.insert("external_hooks".into(), serde_json::json!({"message_id":message_id,"invocation_id":proposed_id,"revision":revision,"consumed_context_bytes":0}));
        tx.execute(
            "UPDATE sessions SET metadata=?1 WHERE id=?2",
            (metadata.to_string(), session_id),
        )?;
        tx.commit()?;
        Ok(HookInvocationClaim {
            invocation_id: proposed_id.to_owned(),
            session_start: first == message_id,
            ingress_required: true,
            consumed_context_bytes: 0,
        })
    }

    fn session_ward_id(&self, session_id: &str) -> Result<Option<String>> {
        let conn = self.pool.get()?;
        let ward_id = conn
            .query_row(
                "SELECT ward_id FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(ward_id)
    }

    fn session_agent_id(&self, session_id: &str) -> Result<Option<String>> {
        let conn = self.pool.get()?;
        let agent_id = conn
            .query_row(
                "SELECT root_agent_id FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(agent_id)
    }
}
