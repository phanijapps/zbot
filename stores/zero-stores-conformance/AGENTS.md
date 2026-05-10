# zero-stores-conformance

Cross-impl behavioral test harness for `zero-stores` traits. Not used in production.

## Purpose

Provides generic test functions parameterized over store traits (e.g., `KnowledgeGraphStore`, `MemoryFactStore`). Backend crates call these from their integration tests to verify behavioral correctness and catch drift between implementations.

## Usage Pattern

```rust
// In zero-stores-sqlite integration tests:
use zero_stores_conformance::entity_round_trip;

#[tokio::test]
async fn test_entity_round_trip() {
    let store = SqliteKgStore::new_in_memory().await.unwrap();
    entity_round_trip(&store).await;
}
```

## Available Test Functions

- `entity_round_trip` — upsert, fetch, delete
- `upsert_increments_mention_count` — repeated upserts grow mention count
- `bump_mention_increases_count` — `bump_entity_mention` increments counter
- (More functions cover relationship CRUD, vector search, etc.)

## Intra-Repo Dependencies

- `zero-stores` — `KnowledgeGraphStore` and related traits
- `zero-stores-traits` — `MemoryFactStore` and related traits
- `knowledge-graph` — `Entity`, `Relationship` domain types

## Notes

- This crate is a `[dev-dependency]` only; never appear in production binaries.
- When adding a new trait method, add a conformance test here alongside.
