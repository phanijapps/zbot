//! Type bridges between zero-stores-traits domain types and SurrealDB types.
//!
//! `RecordId` (Surreal record id) does not leak past this crate.

use surrealdb::types::{RecordId, RecordIdKey};
use zero_stores::types::EntityId;

pub trait EntityIdExt {
    /// Convert an `EntityId` to a SurrealDB `RecordId` on the `entity` table.
    fn to_thing(&self) -> RecordId;
}

impl EntityIdExt for EntityId {
    fn to_thing(&self) -> RecordId {
        RecordId::new("entity", self.as_ref())
    }
}

pub trait ThingExt {
    fn to_entity_id(&self) -> EntityId;
}

impl ThingExt for RecordId {
    fn to_entity_id(&self) -> EntityId {
        let raw = match &self.key {
            RecordIdKey::String(s) => s.clone(),
            RecordIdKey::Number(n) => n.to_string(),
            RecordIdKey::Uuid(u) => u.to_string(),
            other => format!("{other:?}"),
        };
        EntityId::from(raw)
    }
}

/// Convert a `Vec<f32>` embedding to a serde_json::Value array.
/// SurrealDB's `array<float>` accepts JSON arrays of numbers directly via bind.
pub fn embedding_to_value(emb: &[f32]) -> serde_json::Value {
    serde_json::Value::Array(emb.iter().map(|x| serde_json::json!(x)).collect())
}

/// Convert a serde_json::Value (array of numbers) back to Vec<f32>.
pub fn value_to_embedding(v: &serde_json::Value) -> Option<Vec<f32>> {
    let arr = v.as_array()?;
    arr.iter()
        .map(|x| x.as_f64().map(|f| f as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_to_thing_round_trip() {
        let id = EntityId::from("e_abc123".to_string());
        let thing = id.to_thing();
        assert_eq!(thing.table.as_str(), "entity");
        assert!(matches!(&thing.key, RecordIdKey::String(s) if s == "e_abc123"));

        let back = thing.to_entity_id();
        assert_eq!(back.as_ref(), "e_abc123");
    }
}

#[cfg(test)]
mod embedding_tests {
    use super::*;

    #[test]
    fn embedding_round_trip() {
        let emb = vec![0.1_f32, 0.2, 0.3, 0.4];
        let value = embedding_to_value(&emb);
        let back = value_to_embedding(&value).expect("round trip");
        assert_eq!(emb.len(), back.len());
        for (a, b) in emb.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-6, "{a} vs {b}");
        }
    }

    #[test]
    fn embedding_empty_round_trip() {
        let emb: Vec<f32> = vec![];
        let value = embedding_to_value(&emb);
        let back = value_to_embedding(&value).expect("round trip");
        assert_eq!(back.len(), 0);
    }
}
