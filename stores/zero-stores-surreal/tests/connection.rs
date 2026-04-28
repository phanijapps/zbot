use zero_stores_surreal::{SurrealConfig, connect};

#[tokio::test]
async fn connect_in_memory_succeeds() {
    let cfg = SurrealConfig {
        url: "mem://".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    };
    let db = connect(&cfg, None).await.expect("connect");
    let mut resp = db.query("RETURN 42").await.expect("query");
    let n: Option<i64> = resp.take(0).expect("take");
    assert_eq!(n, Some(42));
}

#[tokio::test]
async fn connect_invalid_url_errors() {
    let cfg = SurrealConfig {
        url: "definitely-not-a-scheme://nope".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    };
    let result = connect(&cfg, None).await;
    assert!(result.is_err(), "should reject unknown scheme");
}

#[tokio::test]
async fn vault_placeholder_expanded() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = SurrealConfig {
        url: "rocksdb://$VAULT/data/knowledge.surreal".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    };
    let db = connect(&cfg, Some(tmp.path())).await.expect("connect");
    drop(db);
    let expected = tmp.path().join("data").join("knowledge.surreal");
    assert!(expected.exists(), "rocksdb dir should be created at {expected:?}");
}
