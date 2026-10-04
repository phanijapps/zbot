use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use zbot_conversation::{open_conversation_pool, SessionMetaStore, SqliteSessionMetaStore};

fn store() -> (Pool<SqliteConnectionManager>, SqliteSessionMetaStore) {
    let file = tempfile::NamedTempFile::new().unwrap();
    let pool = open_conversation_pool(file.path()).unwrap();
    {
        let conn = pool.get().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                root_agent_id TEXT NOT NULL,
                ward_id TEXT
            );
            "#,
        )
        .unwrap();
    }
    let store = SqliteSessionMetaStore::new(pool.clone());
    (pool, store)
}

#[test]
fn reads_session_ward_and_agent() {
    let (pool, store) = store();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, root_agent_id, ward_id) VALUES (?1, ?2, ?3)",
            ("sess-1", "root-agent", "ward-alpha"),
        )
        .unwrap();
    }

    assert_eq!(
        store.session_ward_id("sess-1").unwrap().as_deref(),
        Some("ward-alpha")
    );
    assert_eq!(
        store.session_agent_id("sess-1").unwrap().as_deref(),
        Some("root-agent")
    );
}

#[test]
fn missing_session_returns_none() {
    let (_pool, store) = store();
    assert!(store.session_ward_id("missing").unwrap().is_none());
    assert!(store.session_agent_id("missing").unwrap().is_none());
}

#[test]
fn hook_acceptance_claim_is_durable_once_and_revision_checked() {
    let (pool, store) = store();
    let conn = pool.get().unwrap();
    conn.execute_batch("ALTER TABLE sessions ADD COLUMN metadata TEXT; INSERT INTO sessions(id,root_agent_id,metadata) VALUES('s','root','{\"other\":true}'); INSERT INTO messages(id,session_id,role,content,created_at,seq) VALUES('m1','s','user','a','now',1),('m2','s','user','b','now',2);").unwrap();
    drop(conn);
    let first = store
        .claim_hook_invocation("s", "m1", "revision-a", "invocation-a", false)
        .unwrap();
    assert!(first.session_start && first.ingress_required);
    store
        .checkpoint_hook_context_bytes("s", "invocation-a", 4096)
        .unwrap();
    let replay = store
        .claim_hook_invocation("s", "m1", "revision-a", "unused", true)
        .unwrap();
    assert_eq!(replay.invocation_id, "invocation-a");
    assert!(!replay.ingress_required);
    assert_eq!(replay.consumed_context_bytes, 4096);
    assert!(store
        .claim_hook_invocation("s", "m1", "revision-b", "unused", true)
        .is_err());
    let second = store
        .claim_hook_invocation("s", "m2", "revision-b", "invocation-b", false)
        .unwrap();
    assert!(!second.session_start);
    assert!(second.ingress_required);
    store
        .checkpoint_hook_context_bytes("s", "invocation-a", 8192)
        .unwrap();
    assert_eq!(
        store
            .claim_hook_invocation("s", "m2", "revision-b", "unused", true)
            .unwrap()
            .consumed_context_bytes,
        0
    );
    assert!(store
        .claim_hook_invocation("s", "m1", "revision-a", "unused", true)
        .is_err());
    let metadata: String = pool
        .get()
        .unwrap()
        .query_row("SELECT metadata FROM sessions WHERE id='s'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&metadata).unwrap()["other"],
        true
    );
}
