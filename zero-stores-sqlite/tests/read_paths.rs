use knowledge_graph::types::{Entity, EntityType, Relationship, RelationshipType};
use zero_stores::types::Direction;
use zero_stores::KnowledgeGraphStore;

mod fixtures;

#[tokio::test]
async fn neighbors_returns_outgoing_edge() {
    let (_tmp, store) = fixtures::sqlite_store().await;

    let a = Entity::new(
        "test-agent".to_string(),
        EntityType::Person,
        "Dan".to_string(),
    );
    let b = Entity::new(
        "test-agent".to_string(),
        EntityType::Project,
        "Halite".to_string(),
    );
    let a_id = store.upsert_entity("test-agent", a.clone()).await.unwrap();
    let b_id = store.upsert_entity("test-agent", b.clone()).await.unwrap();

    let rel = Relationship::new(
        "test-agent".to_string(),
        a.id.clone(),
        b.id.clone(),
        RelationshipType::Created,
    );
    store.upsert_relationship("test-agent", rel).await.unwrap();

    let neighbors = store
        .get_neighbors(&a_id, Direction::Outgoing, 10)
        .await
        .expect("neighbors");
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].entity_id, b_id);
}

#[tokio::test]
async fn search_by_name_finds_entity() {
    let (_tmp, store) = fixtures::sqlite_store().await;

    let e = Entity::new(
        "test-agent".to_string(),
        EntityType::Person,
        "Erika Mustermann".to_string(),
    );
    store.upsert_entity("test-agent", e).await.unwrap();

    let hits = store
        .search_entities_by_name("test-agent", "Erika", 10)
        .await
        .expect("search");
    assert_eq!(hits.len(), 1);
}
