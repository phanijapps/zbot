//! Vector similarity index backed by sqlite-vec (`vec0`) virtual tables.
//!
//! Replaces hand-rolled cosine scans across the codebase. Every similarity
//! query in the memory layer routes through this trait.
//!
//! Embeddings MUST be L2-normalized before `upsert`. Distance returned by
//! `query_nearest` is L2 squared; for normalized vectors cosine similarity
//! equals `1.0 - dist / 2.0`.

use std::sync::Arc;

use crate::KnowledgeDatabase;

pub trait VectorIndex: Send + Sync {
    /// Insert or replace the embedding for `id`. Embeddings must be L2-normalized.
    fn upsert(&self, id: &str, embedding: &[f32]) -> Result<(), String>;

    /// Remove the embedding for `id`. No-op if the id is absent.
    fn delete(&self, id: &str) -> Result<(), String>;

    /// Return up to `limit` nearest neighbors as `(id, L2_squared_distance)`.
    /// Callers convert to cosine similarity when needed.
    fn query_nearest(&self, embedding: &[f32], limit: usize) -> Result<Vec<(String, f32)>, String>;
}

/// sqlite-vec-backed VectorIndex over a single `vec0` virtual table.
pub struct SqliteVecIndex {
    db: Arc<KnowledgeDatabase>,
    table: &'static str,
    id_column: &'static str,
    dim: usize,
}

impl SqliteVecIndex {
    pub fn new(
        db: Arc<KnowledgeDatabase>,
        table: &'static str,
        id_column: &'static str,
        dim: usize,
    ) -> Self {
        Self {
            db,
            table,
            id_column,
            dim,
        }
    }
}

impl VectorIndex for SqliteVecIndex {
    fn upsert(&self, id: &str, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dim {
            return Err(format!(
                "embedding dim mismatch: got {}, expected {}",
                embedding.len(),
                self.dim
            ));
        }
        let embedding_json =
            serde_json::to_string(embedding).map_err(|e| format!("serialize embedding: {e}"))?;
        let embedding_col = embedding_column_name(self.table);
        let sql_delete = format!("DELETE FROM {} WHERE {} = ?1", self.table, self.id_column);
        let sql_insert = format!(
            "INSERT INTO {} ({}, {}) VALUES (?1, ?2)",
            self.table, self.id_column, embedding_col
        );
        self.db.with_connection(|conn| {
            conn.execute(&sql_delete, rusqlite::params![id])?;
            conn.execute(&sql_insert, rusqlite::params![id, &embedding_json])?;
            Ok(())
        })
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let sql = format!("DELETE FROM {} WHERE {} = ?1", self.table, self.id_column);
        self.db.with_connection(|conn| {
            conn.execute(&sql, rusqlite::params![id])?;
            Ok(())
        })
    }

    fn query_nearest(&self, embedding: &[f32], limit: usize) -> Result<Vec<(String, f32)>, String> {
        if embedding.len() != self.dim {
            return Err(format!(
                "embedding dim mismatch: got {}, expected {}",
                embedding.len(),
                self.dim
            ));
        }
        let embedding_json =
            serde_json::to_string(embedding).map_err(|e| format!("serialize embedding: {e}"))?;
        let embedding_col = embedding_column_name(self.table);
        let sql = format!(
            "SELECT {}, distance FROM {} WHERE {} MATCH ?1 ORDER BY distance LIMIT ?2",
            self.id_column, self.table, embedding_col
        );
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params![embedding_json, limit as i64], |r| {
                let id: String = r.get(0)?;
                let dist: f32 = r.get(1)?;
                Ok((id, dist))
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }
}

/// Map a vec0 table name to its embedding column name (stable per v22 schema).
fn embedding_column_name(table: &str) -> &'static str {
    match table {
        "kg_name_index" => "name_embedding",
        "memory_facts_index"
        | "wiki_articles_index"
        | "procedures_index"
        | "session_episodes_index" => "embedding",
        _ => "embedding",
    }
}
