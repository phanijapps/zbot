use zero_stores_surreal::{SurrealConfig, connect, schema::apply_schema};

fn mem_config() -> SurrealConfig {
    SurrealConfig {
        url: "mem://".into(),
        namespace: "memory_kg".into(),
        database: "main".into(),
        credentials: None,
    }
}

#[tokio::test]
async fn apply_schema_runs_idempotently() {
    let db = connect(&mem_config(), None).await.expect("connect");
    apply_schema(&db).await.expect("first apply");
    apply_schema(&db).await.expect("second apply");
    apply_schema(&db).await.expect("third apply");
}

#[tokio::test]
async fn schema_creates_entity_table() {
    let db = connect(&mem_config(), None).await.expect("connect");
    apply_schema(&db).await.expect("apply");

    db.query(
        "CREATE entity:test_e SET agent_id='a', name='Alice', entity_type='person'",
    )
    .await
    .expect("insert");

    let mut resp = db
        .query("SELECT name FROM entity:test_e")
        .await
        .expect("select");
    let names: Vec<String> = resp.take("name").expect("take");
    assert_eq!(names.first().map(String::as_str), Some("Alice"));
}

#[tokio::test]
async fn schema_version_recorded_on_first_apply() {
    use zero_stores_surreal::schema::{CURRENT_SCHEMA_VERSION, bootstrap::read_version};
    let db = connect(&mem_config(), None).await.expect("connect");
    assert_eq!(read_version(&db).await.unwrap(), 0, "fresh DB at v0");
    apply_schema(&db).await.expect("apply");

    assert_eq!(read_version(&db).await.unwrap(), CURRENT_SCHEMA_VERSION);
}
