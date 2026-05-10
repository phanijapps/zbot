# Stores

Backend-agnostic persistence layer for AgentZero. Separates domain types, trait surfaces, and concrete implementations so alternate database backends can be added without changing consumers.

## Crates

| Crate | Purpose |
|-------|---------|
| `zero-stores-domain` | Pure-data domain types (serde only, no DB) |
| `zero-stores-traits` | Store trait surfaces — lightweight, no DB drivers |
| `zero-stores` | Re-exports traits + types; also holds `KnowledgeGraphStore` / `MemoryFactStore` traits |
| `zero-stores-sqlite` | SQLite implementation (rusqlite + sqlite-vec + r2d2) — the only concrete backend |
| `zero-stores-conformance` | Cross-impl behavioral test harness |

## Dependency Order

```
zero-stores-domain  (serde only)
    └── zero-stores-traits
          └── zero-stores  (adds KnowledgeGraphStore, MemoryFactStore traits + types)
                └── zero-stores-sqlite  (concrete impl + r2d2 pool + vector index)
                      └── zero-stores-conformance  (test harness, no production usage)
```

## Why the Split

`zero-stores-traits` exists to break a dep cycle: `agent-tools` (deep in the graph, below
`gateway`) needs `MemoryFactStore` but cannot depend on `zero-stores` (which pulls in
`knowledge-graph` which pulls in `agent-runtime` → cycle). The traits crate has only `serde_json`
and `zero-stores-domain` as deps.

## Key Crate Details

### zero-stores-domain
Domain value types that cross the persistence boundary. No methods that touch a DB.
Examples: `Goal`, `MemoryFact`, `Procedure`, `SessionEpisode`, `KgEpisode`, `WikiArticle`,
`UndistilledSession`, `DistillationStats`.

### zero-stores-traits
Trait surfaces for each storage concern:
`ConversationStore`, `EpisodeStore`, `MemoryFactStore`, `KgEpisodeStore`, `CompactionStore`,
`OutboxStore`, `GoalStore`, `DistillationStore`, `RecallLogStore`, `WikiStore`, `ProcedureStore`.

### zero-stores-sqlite
The production backend. Also absorbed `gateway-database` (Slice D8, 2026-04):
- `DatabaseManager` — r2d2 connection pool, WAL mode, 8 connections
- `SqliteKgStore` — implements `KnowledgeGraphStore`
- `SqliteMemoryStore` / `GatewayMemoryFactStore` — implements `MemoryFactStore`
- `ConversationRepository`, `EpisodeRepository`, `MemoryRepository`, etc.
- `SqliteVecIndex` / `VectorIndex` — sqlite-vec based embedding index
- `KnowledgeDatabase` — knowledge-graph + embedding unified DB handle

### zero-stores-conformance
Generic test functions parameterized over store traits. Impl crates call these from
integration tests to catch behavioral drift between backends.
