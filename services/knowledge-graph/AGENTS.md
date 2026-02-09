# knowledge-graph

Entity extraction and relationship storage from conversations. Uses heuristic + LLM-powered extraction with SQLite full-text search.

## Build & Test

```bash
cargo test -p knowledge-graph    # 19 tests
```

## Key Types

| Type | Purpose |
|------|---------|
| `Entity` | Named entity with type, properties, timestamps, mention count |
| `Relationship` | Connection between entities with confidence/context |
| `EntityType` | Person, Organization, Location, Concept, Tool, Project, Custom |
| `RelationshipType` | WorksFor, LocatedIn, RelatedTo, Created, Uses, PartOf, Mentions, Custom |
| `ExtractedKnowledge` | Batch of entities and relationships |

## Public API

| Struct | Method | Purpose |
|--------|--------|---------|
| `EntityExtractor` | `extract_from_message()` | Extract entities from text |
| `GraphStorage` | `store_knowledge()` | Persist extracted knowledge |
| `GraphStorage` | `get_entities()` | Query entities |
| `GraphStorage` | `get_relationships()` | Query relationships |

## File Structure

| File | Purpose |
|------|---------|
| `types.rs` | Entity/Relationship types (~15 tests) |
| `extractor.rs` | Entity extraction logic (~4 tests) |
| `storage.rs` | SQLite operations |
| `error.rs` | GraphError type |
| `lib.rs` | Public exports |
