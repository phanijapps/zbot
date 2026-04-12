//! VectorIndex integration tests against a real knowledge.db + sqlite-vec.

use std::sync::Arc;
use tempfile::tempdir;

use gateway_database::vector_index::{SqliteVecIndex, VectorIndex};
use gateway_database::KnowledgeDatabase;
use gateway_services::VaultPaths;

fn setup() -> (tempfile::TempDir, Arc<KnowledgeDatabase>) {
    let tmp = tempdir().expect("tempdir");
    let paths = Arc::new(VaultPaths::new(tmp.path().to_path_buf()));
    std::fs::create_dir_all(paths.conversations_db().parent().expect("parent")).expect("mkdir");
    let db = Arc::new(KnowledgeDatabase::new(paths.clone()).expect("init knowledge db"));
    (tmp, db)
}

fn normalized(v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-9 {
        v
    } else {
        v.into_iter().map(|x| x / norm).collect()
    }
}

#[test]
fn upsert_and_query_nearest_returns_self() {
    let (_tmp, db) = setup();
    let idx = SqliteVecIndex::new(db.clone(), "kg_name_index", "entity_id", 384);

    let v = normalized((0..384).map(|i| i as f32).collect());
    idx.upsert("e1", &v).expect("upsert");

    let results = idx.query_nearest(&v, 1).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "e1");
    assert!(
        results[0].1 < 1e-3,
        "nearest self distance should be ~0, got {}",
        results[0].1
    );
}

#[test]
fn delete_removes_entry() {
    let (_tmp, db) = setup();
    let idx = SqliteVecIndex::new(db.clone(), "memory_facts_index", "fact_id", 384);

    let v = normalized((0..384).map(|i| i as f32).collect());
    idx.upsert("f1", &v).expect("upsert");
    idx.delete("f1").expect("delete");

    let results = idx.query_nearest(&v, 5).expect("query");
    assert!(results.iter().all(|(id, _)| id != "f1"));
}

#[test]
fn upsert_same_id_replaces_not_duplicates() {
    let (_tmp, db) = setup();
    let idx = SqliteVecIndex::new(db.clone(), "procedures_index", "procedure_id", 384);

    let v1 = normalized(vec![1.0; 384]);
    let v2 = normalized((0..384).map(|i| i as f32).collect());
    idx.upsert("p1", &v1).expect("first");
    idx.upsert("p1", &v2).expect("replace");

    let count_total = db
        .with_connection(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM procedures_index WHERE procedure_id = ?1",
                rusqlite::params!["p1"],
                |r| r.get(0),
            )?;
            Ok(n)
        })
        .expect("count");
    assert_eq!(count_total, 1, "upsert must replace, not duplicate");
}

#[test]
fn dim_mismatch_errors() {
    let (_tmp, db) = setup();
    let idx = SqliteVecIndex::new(db.clone(), "wiki_articles_index", "article_id", 384);

    let wrong = vec![0.5_f32; 100];
    assert!(idx.upsert("w1", &wrong).is_err());
    assert!(idx.query_nearest(&wrong, 5).is_err());
}
