//! Cross-impl conformance scenarios for `zero-stores` traits.
//!
//! Each function takes a generic `&S` where `S: KnowledgeGraphStore` (or
//! `MemoryFactStore`, etc.) and runs an end-to-end behavioural check.
//! Impl crates call these from their integration tests; behavioural drift
//! between impls produces failing assertions.

use knowledge_graph::types::{Entity, EntityType};
use zero_stores::KnowledgeGraphStore;

pub async fn entity_round_trip<S: KnowledgeGraphStore>(store: &S) {
    let e = Entity::new(
        "conformance-agent".to_string(),
        EntityType::Person,
        "Conformance Subject".to_string(),
    );
    let original_id = e.id.clone();

    let id = store
        .upsert_entity("conformance-agent", e)
        .await
        .expect("upsert");
    assert_eq!(id.as_ref(), original_id);

    let fetched = store.get_entity(&id).await.expect("get");
    assert!(fetched.is_some(), "entity should exist after upsert");
    assert_eq!(fetched.unwrap().name, "Conformance Subject");

    store.delete_entity(&id).await.expect("delete");
    let after_delete = store.get_entity(&id).await.expect("get");
    assert!(after_delete.is_none(), "entity should be gone after delete");
}
