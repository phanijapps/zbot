//! HandoffWriter — writes session handoff to `session_summaries.json` on completion,
//! and provides `read_handoff_block` for injection at session start.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_runtime::ChatMessage;
use agent_tools::{MemoryEntry, MemoryStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zero_stores_sqlite::ConversationRepository;

pub const HANDOFF_MAX_AGE_DAYS: i64 = 7;

/// Stored JSON shape inside each `handoff.*` entry in `session_summaries.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEntry {
    pub summary: String,
    pub session_id: String,
    pub completed_at: String,
    pub ward_id: String,
    pub intent_key: String,
    pub goal_count: u32,
    pub open_task_count: u32,
    pub correction_count: u32,
    pub turns: u32,
}

/// Input passed to the LLM for summarisation.
#[derive(Debug, Clone)]
pub struct HandoffInput {
    pub messages: Vec<ChatMessage>,
    pub ward_id: String,
}

/// Mockable LLM interface for generating 3-5 sentence handoff summaries.
#[async_trait]
pub trait HandoffLlm: Send + Sync {
    async fn summarize(&self, input: &HandoffInput) -> Result<String, String>;
}

/// Writes session handoff to the shared `session_summaries.json` KV file.
pub struct HandoffWriter {
    llm: Arc<dyn HandoffLlm>,
    /// Path to `agents_data/shared/` directory.
    shared_kv_dir: PathBuf,
    conversation_repo: Arc<ConversationRepository>,
}

impl HandoffWriter {
    pub fn new(
        llm: Arc<dyn HandoffLlm>,
        shared_kv_dir: PathBuf,
        conversation_repo: Arc<ConversationRepository>,
    ) -> Self {
        Self { llm, shared_kv_dir, conversation_repo }
    }

    /// Fire-and-forget entry point: loads last 50 messages then calls
    /// `write_with_messages`. All errors are logged at warn and swallowed.
    pub async fn write(&self, session_id: &str, agent_id: &str, ward_id: &str) {
        let messages_raw = match self.conversation_repo
            .get_session_conversation(session_id, 50)
        {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(session_id, "handoff: failed to load messages: {e}");
                return;
            }
        };
        let messages = self.conversation_repo
            .session_messages_to_chat_format(&messages_raw);
        if let Err(e) = self.write_with_messages(session_id, ward_id, messages).await {
            tracing::warn!(session_id, "handoff: write failed: {e}");
        }
    }

    /// Testable core: accepts pre-loaded messages; returns Err on LLM or I/O failure.
    pub async fn write_with_messages(
        &self,
        session_id: &str,
        ward_id: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<(), String> {
        let turns = messages.iter().filter(|m| m.role == "user").count() as u32;
        let input = HandoffInput {
            messages,
            ward_id: ward_id.to_string(),
        };
        let summary = self.llm.summarize(&input).await?;
        let entry = HandoffEntry {
            summary,
            session_id: session_id.to_string(),
            completed_at: Utc::now().to_rfc3339(),
            ward_id: ward_id.to_string(),
            intent_key: format!("ctx.{session_id}.intent"),
            goal_count: 0,
            open_task_count: 0,
            correction_count: 0,
            turns,
        };
        self.persist(session_id, &entry)
    }

    fn persist(&self, session_id: &str, entry: &HandoffEntry) -> Result<(), String> {
        let path = self.shared_kv_dir.join("session_summaries.json");
        let mut store = load_kv_store(&path)?;
        let value = serde_json::to_string(entry)
            .map_err(|e| format!("serialize entry: {e}"))?;
        let now = Utc::now().to_rfc3339();
        let make_mem_entry = |v: String| MemoryEntry {
            value: v,
            tags: vec![],
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        store.entries.insert("handoff.latest".to_string(), make_mem_entry(value.clone()));
        store
            .entries
            .insert(format!("handoff.{session_id}"), make_mem_entry(value));
        save_kv_store(&path, &store)
    }
}

/// Returns false if the entry is older than `HANDOFF_MAX_AGE_DAYS` or unparseable.
pub fn should_inject(entry: &HandoffEntry) -> bool {
    let Ok(completed_at) = entry.completed_at.parse::<DateTime<Utc>>() else {
        return false;
    };
    let age_days = (Utc::now() - completed_at).num_days();
    age_days <= HANDOFF_MAX_AGE_DAYS
}

/// Reads `handoff.latest` from `<vault_dir>/agents_data/shared/session_summaries.json`.
/// Returns `None` if absent, unparseable, or older than `HANDOFF_MAX_AGE_DAYS`.
/// Returns `Some(block)` where `block` is the `## Last Session` formatted string.
pub fn read_handoff_block(vault_dir: &Path) -> Option<String> {
    todo!("implement in Task 3")
}

// ============================================================================
// Internal KV helpers (no file locking needed — single writer per session)
// ============================================================================

fn load_kv_store(path: &Path) -> Result<MemoryStore, String> {
    if !path.exists() {
        return Ok(MemoryStore::default());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("parse session_summaries: {e}"))
}

fn save_kv_store(path: &Path, store: &MemoryStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(store)
        .map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(path, content.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}

// ============================================================================
// Production LLM impl
// ============================================================================

/// Production `HandoffLlm` wired to the default configured provider.
/// Propagates errors so `write` can log + swallow.
pub struct LlmHandoffWriter {
    provider_service: Arc<gateway_services::ProviderService>,
}

impl LlmHandoffWriter {
    pub fn new(provider_service: Arc<gateway_services::ProviderService>) -> Self {
        Self { provider_service }
    }

    fn build_client(
        &self,
    ) -> Result<Arc<dyn agent_runtime::llm::LlmClient>, String> {
        use agent_runtime::llm::{LlmConfig, openai::OpenAiClient};
        let providers = self
            .provider_service
            .list()
            .map_err(|e| format!("list providers: {e}"))?;
        let provider = providers
            .iter()
            .find(|p| p.is_default)
            .or_else(|| providers.first())
            .ok_or_else(|| "no providers configured".to_string())?;
        let model = provider.default_model().to_string();
        let provider_id = provider.id.clone().unwrap_or_else(|| "default".to_string());
        let config = LlmConfig::new(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model,
            provider_id,
        )
        .with_temperature(0.2)
        .with_max_tokens(256);
        let client = OpenAiClient::new(config)
            .map_err(|e| format!("build client: {e}"))?;
        Ok(Arc::new(client) as Arc<dyn agent_runtime::llm::LlmClient>)
    }
}

#[async_trait]
impl HandoffLlm for LlmHandoffWriter {
    async fn summarize(&self, input: &HandoffInput) -> Result<String, String> {
        let client = self.build_client()?;
        let conversation = input
            .messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .take(40)
            .map(|m| format!("{}: {}", m.role, m.text_content()))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "Summarize this conversation in 3-5 sentences. Cover:\n\
             - What was accomplished\n\
             - What was left incomplete or in progress\n\
             - What the user seemed most focused on or interested in next\n\n\
             Be specific. Do not use filler phrases like 'the user and assistant discussed'.\n\
             Use past tense. Write for an agent reading this at the start of the NEXT session.\n\n\
             Ward: {ward}\n\n\
             Conversation:\n{conversation}",
            ward = input.ward_id,
            conversation = conversation,
        );
        let messages = vec![
            ChatMessage::system(
                "You are a concise session summarizer. Return only the summary text, no JSON.".to_string(),
            ),
            ChatMessage::user(prompt),
        ];
        let response = client
            .chat(messages, None)
            .await
            .map_err(|e| format!("LLM call: {e}"))?;
        Ok(response.content)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use zero_stores_sqlite::{ConversationRepository, DatabaseManager};

    // ---- Mock LLM ----

    struct MockLlm {
        response: Mutex<Result<String, String>>,
        calls: Mutex<u32>,
    }

    impl MockLlm {
        fn ok(summary: &str) -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Ok(summary.to_string())),
                calls: Mutex::new(0),
            })
        }
        fn err() -> Arc<Self> {
            Arc::new(Self {
                response: Mutex::new(Err("mock LLM error".to_string())),
                calls: Mutex::new(0),
            })
        }
        fn call_count(&self) -> u32 {
            *self.calls.lock().unwrap()
        }
    }

    #[async_trait]
    impl HandoffLlm for MockLlm {
        async fn summarize(&self, _input: &HandoffInput) -> Result<String, String> {
            *self.calls.lock().unwrap() += 1;
            self.response.lock().unwrap().clone()
        }
    }

    // ---- Harness ----

    fn make_conversation_repo(tmp: &TempDir) -> Arc<ConversationRepository> {
        use gateway_services::VaultPaths;
        let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
        paths.ensure_dirs_exist().expect("ensure vault dirs");
        let db = Arc::new(DatabaseManager::new(paths).expect("db"));
        Arc::new(ConversationRepository::new(db))
    }

    fn make_writer(
        tmp: &TempDir,
        llm: Arc<dyn HandoffLlm>,
    ) -> HandoffWriter {
        let shared_kv_dir = tmp.path().join("agents_data").join("shared");
        HandoffWriter::new(llm, shared_kv_dir, make_conversation_repo(tmp))
    }

    fn sample_messages(n: usize) -> Vec<ChatMessage> {
        (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    ChatMessage::user(format!("user message {i}"))
                } else {
                    ChatMessage::assistant(format!("assistant reply {i}"))
                }
            })
            .collect()
    }

    // ---- Test 1: generates_summary_from_messages ----

    #[tokio::test]
    async fn generates_summary_from_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = MockLlm::ok("User explored memory limits. Wrote reflective spec. Left impl incomplete.");
        let writer = make_writer(&tmp, llm.clone());

        let result = writer
            .write_with_messages(
                "sess-abc",
                "test-ward",
                sample_messages(6),
            )
            .await;

        assert!(result.is_ok(), "write_with_messages failed: {result:?}");
        assert_eq!(llm.call_count(), 1, "LLM should be called exactly once");

        // Verify the stored entry has non-empty summary
        let path = tmp
            .path()
            .join("agents_data")
            .join("shared")
            .join("session_summaries.json");
        let store = load_kv_store(&path).unwrap();
        let latest = store.entries.get("handoff.latest").expect("handoff.latest missing");
        let entry: HandoffEntry = serde_json::from_str(&latest.value).unwrap();
        assert!(!entry.summary.is_empty(), "summary should be non-empty");
        assert_eq!(entry.session_id, "sess-abc");
        assert_eq!(entry.ward_id, "test-ward");
        assert_eq!(entry.turns, 3, "3 user messages in sample_messages(6)");
    }

    // ---- Test 2: writes_latest_and_archived_keys ----

    #[tokio::test]
    async fn writes_latest_and_archived_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = MockLlm::ok("Session summary here.");
        let writer = make_writer(&tmp, llm);

        writer
            .write_with_messages("sess-xyz", "my-ward", sample_messages(4))
            .await
            .unwrap();

        let path = tmp
            .path()
            .join("agents_data")
            .join("shared")
            .join("session_summaries.json");
        let store = load_kv_store(&path).unwrap();

        assert!(
            store.entries.contains_key("handoff.latest"),
            "handoff.latest missing"
        );
        assert!(
            store.entries.contains_key("handoff.sess-xyz"),
            "handoff.sess-xyz missing"
        );

        // Both entries should have the same summary
        let latest: HandoffEntry =
            serde_json::from_str(&store.entries["handoff.latest"].value).unwrap();
        let archived: HandoffEntry =
            serde_json::from_str(&store.entries["handoff.sess-xyz"].value).unwrap();
        assert_eq!(latest.summary, archived.summary);
    }

    // ---- Test 3: failure_is_silent ----

    #[tokio::test]
    async fn failure_is_silent() {
        let tmp = tempfile::tempdir().unwrap();
        let llm = MockLlm::err();
        let writer = make_writer(&tmp, llm);

        // Should NOT panic, should NOT write anything
        let result = writer
            .write_with_messages("sess-fail", "ward", sample_messages(2))
            .await;

        // write_with_messages returns Err (callers swallow it via `write`)
        assert!(result.is_err(), "expected Err when LLM fails");

        let path = tmp
            .path()
            .join("agents_data")
            .join("shared")
            .join("session_summaries.json");
        assert!(!path.exists(), "no file should be written on LLM failure");
    }

    // ---- Test 4: stale_handoff_excluded ----

    #[test]
    fn stale_handoff_excluded() {
        // 8 days ago — should NOT inject
        let stale = HandoffEntry {
            summary: "old summary".to_string(),
            session_id: "sess-old".to_string(),
            completed_at: (Utc::now() - chrono::Duration::days(8)).to_rfc3339(),
            ward_id: "ward".to_string(),
            intent_key: "ctx.sess-old.intent".to_string(),
            goal_count: 0,
            open_task_count: 0,
            correction_count: 0,
            turns: 5,
        };
        assert!(!should_inject(&stale), "8-day-old handoff should be excluded");

        // 6 days ago — should inject
        let fresh = HandoffEntry {
            completed_at: (Utc::now() - chrono::Duration::days(6)).to_rfc3339(),
            ..stale.clone()
        };
        assert!(should_inject(&fresh), "6-day-old handoff should be injected");
    }
}
