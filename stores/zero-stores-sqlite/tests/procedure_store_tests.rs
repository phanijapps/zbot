//! Integration tests for `GatewayProcedureStore`.

use std::sync::Arc;

use gateway_services::paths::VaultPaths;
use tempfile::TempDir;
use zero_stores_sqlite::{
    GatewayProcedureStore, KnowledgeDatabase, ProcedureRepository, SqliteVecIndex, VectorIndex,
};
use zero_stores_traits::{PatternProcedureInsert, ProcedureStore};

/// Build a `GatewayProcedureStore` backed by an on-disk temp SQLite database.
/// The TempDir must outlive the store, so it is returned alongside.
fn test_procedure_store() -> (TempDir, GatewayProcedureStore) {
    let tmp = TempDir::new().expect("tempdir");
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    let db = Arc::new(KnowledgeDatabase::new(paths).expect("knowledge db"));
    let vec_index: Arc<dyn VectorIndex> = Arc::new(
        SqliteVecIndex::new(db.clone(), "procedures_index", "procedure_id")
            .expect("vec index init"),
    );
    let repo = Arc::new(ProcedureRepository::new(db, vec_index));
    let store = GatewayProcedureStore::new(repo);
    (tmp, store)
}

/// L2-normalize a vector. The `procedures_index` vec0 table expects
/// normalized 384-dim vectors when callers want meaningful similarity.
fn normalized(v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-9 {
        v
    } else {
        v.into_iter().map(|x| x / norm).collect()
    }
}

#[tokio::test]
async fn insert_pattern_procedure_persists_embedding() {
    let (_tmp, store) = test_procedure_store();

    // procedures_index is a 384-dim vec0 table; use a normalized 384-dim
    // vector so the upsert succeeds and similarity search returns 1.0.
    let mut emb = vec![0.0_f32; 384];
    emb[0] = 0.1;
    emb[1] = 0.2;
    emb[2] = 0.3;
    let emb = normalized(emb);

    let req = PatternProcedureInsert {
        agent_id: "root".into(),
        ward_id: Some("__global__".into()),
        name: "test_proc".into(),
        description: "test".into(),
        trigger_pattern: None,
        steps_json: "[]".into(),
        parameters_json: None,
        embedding: Some(emb.clone()),
    };
    let id = store.insert_pattern_procedure(req).await.unwrap();

    let results = store
        .search_procedures_by_similarity(&emb, "root", None, 5)
        .await
        .unwrap();
    assert!(
        results.iter().any(|r| r["procedure"]["id"] == id),
        "expected inserted procedure {id} to be returned by similarity search; got {results:?}",
    );
}

#[tokio::test]
async fn get_procedure_by_name_returns_full_row() {
    let (_tmp, store) = test_procedure_store();
    let req = PatternProcedureInsert {
        agent_id: "root".into(),
        ward_id: Some("__global__".into()),
        name: "find_me".into(),
        description: "a procedure".into(),
        trigger_pattern: None,
        steps_json: r#"[{"action":"shell","args":{"cmd":"ls"},"binds":[]}]"#.into(),
        parameters_json: Some(r#"["dir"]"#.into()),
        embedding: None,
    };
    store.insert_pattern_procedure(req).await.unwrap();
    let found = store
        .get_procedure_by_name("root", "find_me")
        .await
        .unwrap();
    let proc = found.expect("not found");
    assert_eq!(proc.name, "find_me");
    assert_eq!(proc.description, "a procedure");
    assert_eq!(proc.ward_id.as_deref(), Some("__global__"));
    assert!(proc.steps.contains("shell"));
    assert_eq!(proc.parameters.as_deref(), Some(r#"["dir"]"#));
}

#[tokio::test]
async fn get_procedure_by_name_returns_none_when_missing() {
    let (_tmp, store) = test_procedure_store();
    let found = store
        .get_procedure_by_name("root", "nonexistent")
        .await
        .unwrap();
    assert!(found.is_none());
}
