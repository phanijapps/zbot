//! Repository for kg_episodes — records every extraction event for
//! provenance tracking. Facts, entities, and relationships reference
//! an episode ID so we can always answer "where did this come from?"

use crate::KnowledgeDatabase;
use rusqlite::{params, OptionalExtension};
use std::sync::Arc;

/// The source system that produced an episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpisodeSource {
    ToolResult,
    WardFile,
    Session,
    Distillation,
    UserInput,
}

impl EpisodeSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToolResult => "tool_result",
            Self::WardFile => "ward_file",
            Self::Session => "session",
            Self::Distillation => "distillation",
            Self::UserInput => "user_input",
        }
    }
}

/// A provenance record: one extraction event from one source.
#[derive(Debug, Clone)]
pub struct KgEpisode {
    pub id: String,
    pub source_type: String,
    pub source_ref: String,
    pub content_hash: String,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub created_at: String,
}

pub struct KgEpisodeRepository {
    db: Arc<KnowledgeDatabase>,
}

impl KgEpisodeRepository {
    pub fn new(db: Arc<KnowledgeDatabase>) -> Self {
        Self { db }
    }

    /// Insert an episode. Returns Ok(true) if inserted, Ok(false) if a duplicate
    /// (same content_hash + source_type) already exists.
    pub fn upsert_episode(&self, ep: &KgEpisode) -> Result<bool, String> {
        self.db.with_connection(|conn| {
            let changed = conn.execute(
                "INSERT OR IGNORE INTO kg_episodes \
                 (id, source_type, source_ref, content_hash, session_id, agent_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    ep.id,
                    ep.source_type,
                    ep.source_ref,
                    ep.content_hash,
                    ep.session_id,
                    ep.agent_id,
                    ep.created_at,
                ],
            )?;
            Ok(changed > 0)
        })
    }

    /// Look up an episode by content_hash + source_type. Used for dedup
    /// before extraction: if content hasn't changed, skip re-extraction.
    pub fn get_by_content_hash(
        &self,
        content_hash: &str,
        source_type: &str,
    ) -> Result<Option<KgEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id, created_at \
                 FROM kg_episodes WHERE content_hash = ?1 AND source_type = ?2",
            )?;
            let result = stmt
                .query_row(params![content_hash, source_type], Self::row_to_episode)
                .optional()?;
            Ok(result)
        })
    }

    /// Get all episodes for a session.
    pub fn list_by_session(&self, session_id: &str) -> Result<Vec<KgEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id, created_at \
                 FROM kg_episodes WHERE session_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt
                .query_map(params![session_id], Self::row_to_episode)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Get a single episode by ID.
    pub fn get(&self, id: &str) -> Result<Option<KgEpisode>, String> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_ref, content_hash, session_id, agent_id, created_at \
                 FROM kg_episodes WHERE id = ?1",
            )?;
            let result = stmt
                .query_row(params![id], Self::row_to_episode)
                .optional()?;
            Ok(result)
        })
    }

    fn row_to_episode(row: &rusqlite::Row) -> rusqlite::Result<KgEpisode> {
        Ok(KgEpisode {
            id: row.get(0)?,
            source_type: row.get(1)?,
            source_ref: row.get(2)?,
            content_hash: row.get(3)?,
            session_id: row.get(4)?,
            agent_id: row.get(5)?,
            created_at: row.get(6)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KnowledgeDatabase;
    use std::sync::Arc;

    fn setup() -> (tempfile::TempDir, KgEpisodeRepository) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Arc::new(gateway_services::VaultPaths::new(tmp.path().to_path_buf()));
        std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
        let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
        let repo = KgEpisodeRepository::new(db);
        (tmp, repo)
    }

    fn sample_episode() -> KgEpisode {
        KgEpisode {
            id: "ep-1".into(),
            source_type: "ward_file".into(),
            source_ref: "timeline.json".into(),
            content_hash: "abc123".into(),
            session_id: Some("sess-1".into()),
            agent_id: "root".into(),
            created_at: "2026-04-12T00:00:00Z".into(),
        }
    }

    #[test]
    fn upsert_and_get_by_id() {
        let (_tmp, repo) = setup();
        let ep = sample_episode();
        let inserted = repo.upsert_episode(&ep).unwrap();
        assert!(inserted);
        let fetched = repo.get("ep-1").unwrap().unwrap();
        assert_eq!(fetched.source_type, "ward_file");
        assert_eq!(fetched.source_ref, "timeline.json");
    }

    #[test]
    fn duplicate_content_hash_returns_false() {
        let (_tmp, repo) = setup();
        let ep = sample_episode();
        assert!(repo.upsert_episode(&ep).unwrap());
        let ep2 = KgEpisode {
            id: "ep-2".into(),
            ..ep
        };
        assert!(!repo.upsert_episode(&ep2).unwrap());
    }

    #[test]
    fn get_by_content_hash() {
        let (_tmp, repo) = setup();
        let ep = sample_episode();
        repo.upsert_episode(&ep).unwrap();
        let found = repo
            .get_by_content_hash("abc123", "ward_file")
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "ep-1");
        assert!(repo
            .get_by_content_hash("abc123", "tool_result")
            .unwrap()
            .is_none());
    }

    #[test]
    fn list_by_session_returns_in_order() {
        let (_tmp, repo) = setup();
        for i in 0..3 {
            let ep = KgEpisode {
                id: format!("ep-{i}"),
                content_hash: format!("hash-{i}"),
                created_at: format!("2026-04-12T00:00:0{i}Z"),
                ..sample_episode()
            };
            repo.upsert_episode(&ep).unwrap();
        }
        let eps = repo.list_by_session("sess-1").unwrap();
        assert_eq!(eps.len(), 3);
        assert_eq!(eps[0].id, "ep-0");
        assert_eq!(eps[2].id, "ep-2");
    }

    #[test]
    fn get_missing_returns_none() {
        let (_tmp, repo) = setup();
        assert!(repo.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn episode_source_as_str_roundtrip() {
        assert_eq!(EpisodeSource::ToolResult.as_str(), "tool_result");
        assert_eq!(EpisodeSource::WardFile.as_str(), "ward_file");
        assert_eq!(EpisodeSource::Session.as_str(), "session");
        assert_eq!(EpisodeSource::Distillation.as_str(), "distillation");
        assert_eq!(EpisodeSource::UserInput.as_str(), "user_input");
    }
}
