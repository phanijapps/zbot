mod fixtures;

#[tokio::test]
async fn sqlite_entity_round_trip_conformance() {
    let (_tmp, store) = fixtures::sqlite_store().await;
    zero_stores_conformance::entity_round_trip(&store).await;
}
