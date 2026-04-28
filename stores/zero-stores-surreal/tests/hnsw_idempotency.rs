use zero_stores_surreal::{
    SurrealConfig, connect,
    schema::{apply_schema, hnsw},
};

fn cfg() -> SurrealConfig {
    SurrealConfig {
        url: "mem://".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    }
}

#[tokio::test]
async fn hnsw_define_idempotent_with_matching_dim() {
    let db = connect(&cfg(), None).await.unwrap();
    apply_schema(&db).await.unwrap();

    hnsw::ensure_index(&db, 1024).await.expect("first");
    hnsw::ensure_index(&db, 1024).await.expect("second");
    hnsw::ensure_index(&db, 1024).await.expect("third");
    assert_eq!(hnsw::read_dim(&db).await.unwrap(), Some(1024));
}

#[tokio::test]
async fn hnsw_dim_mismatch_returns_error() {
    let db = connect(&cfg(), None).await.unwrap();
    apply_schema(&db).await.unwrap();

    hnsw::ensure_index(&db, 1024).await.expect("first");
    let err = hnsw::ensure_index(&db, 1536).await.unwrap_err();
    assert!(format!("{err}").contains("dim mismatch"));
}

#[tokio::test]
async fn hnsw_first_write_persists_dim() {
    let db = connect(&cfg(), None).await.unwrap();
    apply_schema(&db).await.unwrap();
    assert_eq!(hnsw::read_dim(&db).await.unwrap(), None, "fresh DB");
    hnsw::ensure_index(&db, 768).await.expect("first write");
    assert_eq!(hnsw::read_dim(&db).await.unwrap(), Some(768));
}
