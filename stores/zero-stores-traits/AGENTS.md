# zero-stores-traits

Dependency-light home for all storage trait surfaces. Only depends on `serde_json` and `zero-stores-domain`.

## Purpose

Breaks a potential dep cycle: `agent-tools` needs `MemoryFactStore` but sits below `gateway` in the dep graph. Pulling in `zero-stores` (which depends on `knowledge-graph`) would create a cycle. This crate exposes the trait without the cycle.

The full `zero-stores` crate re-exports everything here, so the canonical import path `zero_stores::MemoryFactStore` still works for callers that already depend on `zero-stores`.

## Trait Surfaces

| Trait | Purpose |
|-------|---------|
| `ConversationStore` | Read/write conversation messages |
| `EpisodeStore` | Session episodes for recall |
| `KgEpisodeStore` | Knowledge-graph episode tracking |
| `MemoryFactStore` | Agent memory facts (semantic + structured) |
| `CompactionStore` | DB compaction run tracking |
| `OutboxStore` | Bridge worker outbox |
| `GoalStore` | Agent goal persistence |
| `DistillationStore` | Session distillation records |
| `RecallLogStore` | Recall query logging |
| `WikiStore` | Ward wiki article storage |
| `ProcedureStore` | Learned procedure patterns |

## Intra-Repo Dependencies

- `zero-stores-domain` — domain value types for method signatures

## Notes

- All traits use `async-trait`.
- Implementations live in `zero-stores-sqlite`.
- Never add DB driver dependencies here.
