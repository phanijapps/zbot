# zero-stores

Backend-agnostic persistence interfaces for AgentZero. Defines store traits and types — no database drivers.

## Purpose

Public design surface for the persistence layer. Consumers depend on `zero-stores` to get traits and types without coupling to a specific database backend. The concrete implementation is in `zero-stores-sqlite`.

## Modules

| Module | Purpose |
|--------|---------|
| `knowledge_graph` | `KnowledgeGraphStore` trait + query types |
| `memory_facts` | `MemoryFactStore` trait + metric types |
| `types` | Shared types: `EntityId`, `Direction`, `ResolveOutcome` |
| `extracted` | `ExtractedKnowledge` — batch extraction result |
| `error` | `StoreError`, `StoreResult` |

## Key Exports

```rust
pub use knowledge_graph::{KnowledgeGraphStore, GraphView, DecayCandidate, ...};
pub use memory_facts::{MemoryFactStore, MemoryAggregateStats, MemoryHealthMetrics, ...};
pub use types::*;
pub use extracted::ExtractedKnowledge;
```

All trait surfaces from `zero-stores-traits` are also re-exported here for the canonical import path.

## Intra-Repo Dependencies

- `zero-stores-domain` — domain value types
- `zero-stores-traits` — trait surfaces (re-exported)
- `knowledge-graph` — `Entity`, `Relationship`, `EntityType` etc. from the types module

## Notes

- Never import `zero-stores-sqlite` directly from business logic — always go through `zero-stores` traits.
- The gateway wires the concrete `SqliteKgStore` / `GatewayMemoryFactStore` via `persistence_factory.rs`.
