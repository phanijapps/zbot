# Services

Standalone data services with minimal framework dependencies. Each service is independently testable.

## Crates

| Crate | Purpose | Storage |
|-------|---------|---------|
| `api-logs` | Execution logging and tracing | SQLite |
| `knowledge-graph` | Entity extraction and relationships | SQLite |
| `search-index` | Full-text search | Tantivy |
| `session-archive` | Long-term session archival | Parquet |
| `daily-sessions` | Daily session management | SQLite + Cache |

## Design Pattern

Services expose traits that the gateway implements:

```rust
// In services/api-logs
pub trait DbProvider {
    fn get_connection(&self) -> &Connection;
}

// Gateway implements this trait
impl DbProvider for AppState { ... }
```

This inverts dependencies - services don't depend on gateway, gateway depends on services.

## api-logs

Execution tracing with REST API:

```
GET  /sessions         - List execution sessions
GET  /sessions/:id     - Get session with logs
DELETE /sessions/:id   - Delete session
```

## knowledge-graph

Extract and query entities/relationships:

- Entity types: Person, Organization, Place, Concept
- Relationship extraction
- LLM-powered smart extraction

## search-index

Full-text search using Tantivy:

- Index conversation messages
- Fast predicate pushdown queries

## session-archive

Parquet-based archival:

- Columnar compression
- Efficient long-term storage
- Query archived sessions

## daily-sessions

Session lifecycle management:

- Group sessions by day
- Caching layer (moka)
- Database persistence
