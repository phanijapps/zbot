//! Exploration pagination construction tests (spec
//! observatory-graph-completeness AC1/AC2/AC8). Generated fixture: 1200
//! entities and 900 relationships across two agents, one edge whose endpoint
//! entity is absent, and an empty third scope. Asserts exact scoped totals,
//! deterministic disjoint pages, filter consistency, live-view behavior under
//! mutation, and bounded limits.

use knowledge_graph::kg_trait::KnowledgeGraphStore;
use knowledge_graph::types::{Entity, EntityType, Relationship, RelationshipType};
use std::collections::HashSet;
use zbot_engram_adapter::stores::knowledge_graph::EngramKnowledgeGraphStore;
use zbot_engram_adapter::AdapterConfig;

fn engram_config(root: &tempfile::TempDir) -> AdapterConfig {
    AdapterConfig::engram_for_data_root(root.path(), "engram.db")
}

const ENTITIES_PER_AGENT: usize = 600;
const RELATIONSHIPS_PER_AGENT: usize = 450;
const PAGE: usize = 50;

async fn seed_fixture(store: &EngraphStoreForTest) {
    for agent in ["agent-a", "agent-b"] {
        for index in 0..ENTITIES_PER_AGENT {
            store
                .upsert_entity(
                    agent,
                    Entity {
                        id: format!("entity-{agent}-{index:04}"),
                        mention_count: ((index % 7) + 1) as i64,
                        ..Entity::new(
                            agent.to_string(),
                            if index % 3 == 0 { EntityType::Concept } else { EntityType::Tool },
                            format!("{agent} concept {index}"),
                        )
                    },
                )
                .await
                .expect("seed entity");
        }
        for index in 0..RELATIONSHIPS_PER_AGENT {
            store
                .upsert_relationship(
                    agent,
                    Relationship::new(
                        agent.to_string(),
                        format!("entity-{agent}-{index:04}"),
                        format!("entity-{agent}-{:04}", (index + 1) % ENTITIES_PER_AGENT),
                        RelationshipType::RelatedTo,
                    ),
                )
                .await
                .expect("seed relationship");
        }
    }
    // NOTE: the store rejects relationships whose endpoint entities do not
    // exist (see missing_endpoint_edges_are_rejected_at_write_time), so
    // stored data cannot dangle — unresolved endpoints are a pagination-
    // ordering phenomenon handled at the HTTP/UI layers (T2/T3).
}

type EngraphStoreForTest = EngramKnowledgeGraphStore;

async fn open_store() -> (tempfile::TempDir, EngraphStoreForTest) {
    let root = tempfile::tempdir().expect("root");
    let store = EngraphStoreForTest::open(engram_config(&root)).expect("store");
    (root, store)
}

#[tokio::test]
async fn cross_agent_entity_pages_enumerate_exactly_once_with_exact_total() {
    let _root = tempfile::tempdir().expect("root");
    let (root, store) = open_store().await;
    drop(_root);
    seed_fixture(&store).await;

    let mut seen: HashSet<String> = HashSet::new();
    let mut offset = 0usize;
    let mut total_from_first_page = None;
    loop {
        let page = store
            .list_all_entities_paged(None, None, PAGE, offset)
            .await
            .expect("page");
        if total_from_first_page.is_none() {
            total_from_first_page = Some(page.total);
        }
        if page.entities.is_empty() {
            break;
        }
        for entity in &page.entities {
            assert!(seen.insert(entity.id.clone()), "duplicate row: {}", entity.id);
        }
        offset += page.entities.len();
        if offset >= page.total {
            break;
        }
    }
    assert_eq!(seen.len(), 2 * ENTITIES_PER_AGENT);
    assert_eq!(total_from_first_page, Some(2 * ENTITIES_PER_AGENT), "total must be the exact scope count");
    assert_eq!(offset, 2 * ENTITIES_PER_AGENT);
    let _ = root;
}

// AC2/AC8 missing-endpoint case at the store boundary: referential
// integrity is enforced at write time, so traversal can never encounter a
// dangling stored edge.
#[tokio::test]
async fn missing_endpoint_edges_are_rejected_at_write_time() {
    let (_root, store) = open_store().await;
    seed_fixture(&store).await;
    let rejected = store
        .upsert_relationship(
            "agent-a",
            Relationship::new(
                "agent-a".to_string(),
                "entity-agent-a-0000".to_string(),
                "entity-agent-a-absent".to_string(),
                RelationshipType::RelatedTo,
            ),
        )
        .await;
    assert!(rejected.is_err(), "dangling edges must be rejected at write time");
    let page = store
        .list_all_relationships_paged(None, PAGE, 0)
        .await
        .expect("page");
    assert_eq!(page.total, 2 * RELATIONSHIPS_PER_AGENT);
}

#[tokio::test]
async fn cross_agent_relationship_pages_have_exact_deduplicated_total() {
    let (_root, store) = open_store().await;
    seed_fixture(&store).await;

    let mut seen: HashSet<String> = HashSet::new();
    let mut offset = 0usize;
    let mut total = None;
    loop {
        let page = store
            .list_all_relationships_paged(None, PAGE, offset)
            .await
            .expect("page");
        if total.is_none() {
            total = Some(page.total);
        }
        if page.relationships.is_empty() {
            break;
        }
        for relationship in &page.relationships {
            assert!(
                seen.insert(relationship.id.clone()),
                "duplicate relationship: {}",
                relationship.id
            );
        }
        offset += page.relationships.len();
        if offset >= page.total {
            break;
        }
    }
    assert_eq!(total, Some(2 * RELATIONSHIPS_PER_AGENT));
    assert_eq!(seen.len(), 2 * RELATIONSHIPS_PER_AGENT);
}

#[tokio::test]
async fn per_agent_scope_and_type_filter_have_consistent_totals() {
    let (_root, store) = open_store().await;
    seed_fixture(&store).await;

    let per_agent = store
        .list_entities_paged("agent-a", None, PAGE, 0)
        .await
        .expect("per-agent page");
    // Per-agent scope includes the agent's entities only (no __global__ rows
    // exist in this fixture).
    assert_eq!(per_agent.total, ENTITIES_PER_AGENT);

    let concepts = store
        .list_entities_paged("agent-a", Some("concept"), PAGE, 0)
        .await
        .expect("filtered page");
    let expected_concepts = (0..ENTITIES_PER_AGENT).filter(|i| i % 3 == 0).count();
    assert_eq!(concepts.total, expected_concepts, "filter total must match the same WHERE");

    let all_concepts = store
        .list_all_entities_paged(None, Some("concept"), PAGE, 0)
        .await
        .expect("all-agent filtered page");
    assert_eq!(all_concepts.total, 2 * expected_concepts);

    let per_agent_rels = store
        .list_relationships_paged("agent-b", None, PAGE, 0)
        .await
        .expect("per-agent relationships");
    assert_eq!(per_agent_rels.total, RELATIONSHIPS_PER_AGENT);
}

#[tokio::test]
async fn deterministic_order_is_mention_first_with_unique_tiebreaker() {
    let (_root, store) = open_store().await;
    seed_fixture(&store).await;

    let first = store
        .list_all_entities_paged(None, None, 5, 0)
        .await
        .expect("first page")
        .entities;
    let mentions: Vec<i64> = first.iter().map(|e| e.mention_count).collect();
    let max_mention = 7i64; // (index % 7) + 1 peaks at 7 for indices ≡ 6 (mod 7), which exist below 600
    assert!(
        mentions.iter().all(|m| *m == max_mention),
        "first page must be most-mentioned-first: {mentions:?} (expected {max_mention})"
    );
    // Same query again → identical page (deterministic).
    let again = store
        .list_all_entities_paged(None, None, 5, 0)
        .await
        .expect("repeat page")
        .entities;
    assert_eq!(
        first.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
        again.iter().map(|e| e.id.clone()).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn mutation_between_pages_is_a_live_view_with_changed_totals() {
    let (_root, store) = open_store().await;
    seed_fixture(&store).await;

    let first = store
        .list_all_entities_paged(None, None, PAGE, 0)
        .await
        .expect("first page");
    assert_eq!(first.total, 2 * ENTITIES_PER_AGENT);

    // Mutate between pages: add one entity, prune-check by changed total.
    store
        .upsert_entity(
            "agent-a",
            Entity {
                id: "entity-agent-a-late".to_string(),
                mention_count: 99,
                ..Entity::new(
                    "agent-a".to_string(),
                    EntityType::Concept,
                    "Late arrival".to_string(),
                )
            },
        )
        .await
        .expect("late entity");

    let second = store
        .list_all_entities_paged(None, None, PAGE, PAGE)
        .await
        .expect("second page");
    assert_eq!(
        second.total,
        2 * ENTITIES_PER_AGENT + 1,
        "totals are a live view; a changed total signals refresh, not a frozen snapshot"
    );
    // The high-mention late arrival is on page one, not page two.
    assert!(!second.entities.iter().any(|e| e.id == "entity-agent-a-late"));
}

#[tokio::test]
async fn empty_scope_and_bounded_limits_behave() {
    let (_root, store) = open_store().await;
    seed_fixture(&store).await;

    let empty = store
        .list_all_entities_paged(Some("ward-none"), None, PAGE, 0)
        .await
        .expect("empty scope");
    assert_eq!(empty.total, 0);
    assert!(empty.entities.is_empty());

    let beyond = store
        .list_all_entities_paged(None, None, PAGE, 2 * ENTITIES_PER_AGENT + 10)
        .await
        .expect("over-offset page");
    assert_eq!(beyond.total, 2 * ENTITIES_PER_AGENT, "over-offset still reports the exact total");
    assert!(beyond.entities.is_empty());

    // The ward filter matches entities carrying a ward_id property.
    let mut warded = Entity::new("agent-a".to_string(), EntityType::Concept, "Warded".to_string());
    warded
        .properties
        .insert("ward_id".to_string(), serde_json::json!("ward-x"));
    store.upsert_entity("agent-a", warded).await.expect("warded");
    let warded_page = store
        .list_all_entities_paged(Some("ward-x"), None, PAGE, 0)
        .await
        .expect("ward page");
    assert_eq!(warded_page.total, 1);
    assert_eq!(warded_page.entities.len(), 1);
}
