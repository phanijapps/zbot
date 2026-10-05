// Empirical check: does list_all_relationships_paged deadlock when
// relationship rows lack ward_id properties (dedup falls back to get_entity)?
use knowledge_graph::kg_trait::KnowledgeGraphStore;
use knowledge_graph::types::{Entity, EntityType, Relationship, RelationshipType};
use zbot_engram_adapter::stores::knowledge_graph::EngramKnowledgeGraphStore;
use zbot_engram_adapter::AdapterConfig;

#[tokio::test]
async fn paged_relationships_do_not_deadlock_on_wardless_rows() {
    let root = tempfile::tempdir().expect("root");
    let store = EngramKnowledgeGraphStore::open(AdapterConfig::engram_for_data_root(root.path(), "engram.db")).expect("store");
    // Entities WITHOUT ward_id, relationships WITHOUT ward_id properties —
    // exactly the legacy/raw shape the dedup fallback exists for.
    for index in 0..3 {
        store.upsert_entity("agent-a", Entity {
            id: format!("e-{index}"),
            ..Entity::new("agent-a".to_string(), EntityType::Concept, format!("E{index}"))
        }).await.expect("entity");
    }
    for index in 0..2 {
        store.upsert_relationship("agent-a", Relationship::new(
            "agent-a".to_string(),
            format!("e-{index}"),
            format!("e-{}", index + 1),
            RelationshipType::RelatedTo,
        )).await.expect("relationship");
    }
    // Strip ward_id properties directly in the sidecar tables to force the
    // get_entity fallback path in relationship_dedup_key.
    // (The write path injects ward_id; raw rows may lack it.)
    let conn_path = root.path().join("engram.db").join("engram_data.db");
    let connection = rusqlite::Connection::open(&conn_path).unwrap();
    connection.execute("UPDATE kg_relationships SET properties_json = '{}', relationship_json = json_set(relationship_json, '$.properties', json('{}'))", []).unwrap();
    drop(connection);

    let page = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        store.list_all_relationships_paged(None, 10, 0),
    ).await.expect("TIMED OUT — deadlock confirmed").expect("page");
    assert_eq!(page.total, 2);
}
