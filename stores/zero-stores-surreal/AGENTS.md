# zero-stores-surreal — AGENTS.md

Locked design decisions for this crate. Spec lives at
`memory-bank/future-state/2026-04-27-surrealdb-backend-design.md`.

## Topology

- **Mode A (today):** embedded RocksDB. URL `rocksdb://$VAULT/...`. Same process as gateway daemon.
- **Mode B (future):** subprocess sidecar over WebSocket. URL `ws://127.0.0.1:PORT`. Same `Surreal<Any>` SDK type, only the URL changes. Mode B requires zero changes to this crate — the supervisor lives in `gateway/gateway-surreal-supervisor/`.

## Engine-erased type

Use `Surreal<Any>` everywhere. Construction is via
`surrealdb::engine::any::connect(&url)`. **Never** use the typed engines
(`Surreal<Db>`, `Surreal<Client>`) — that breaks Mode B migration.

## Namespaces

- `memory_kg` — the only namespace today. Holds entities, relationships, memory facts, wiki docs.
- `conversations` — **reserved**. Do not create. Conversations stay on SQLite for this release.

## Schema is declarative

All `DEFINE NAMESPACE / DATABASE / TABLE / FIELD / INDEX` statements use
`IF NOT EXISTS` and run on every startup. Idempotent. No numbered migration files.
For breaking changes, use `_meta:version` + upgrade closures in `schema/bootstrap.rs`.

## HNSW is lazy + idempotent

- HNSW index is **not** defined at bootstrap when no embeddings exist.
- On the first embedding write: detect dim, write `_meta:embedding_config { dim }`, define index.
- On restart with embeddings already present: bootstrap reads `_meta:embedding_config` and issues `DEFINE INDEX ... IF NOT EXISTS DIMENSION $dim` — **no rebuild**.
- On `reindex_embeddings(N)` when current dim == N: return `rebuilt: false` immediately. No-op.

## Refuse to start on corruption

If `connect()` or the bootstrap health probe fails, the daemon **must not** silently fall back to SQLite or empty state. Surface a clear error and exit non-zero. Recovery is the CLI subcommand backed by `zero-stores-surreal-recovery`.

## Type bridging stays in `types.rs`

`Thing` (Surreal record id) **never** leaks past this crate. Convert to/from `EntityId(String)` at the boundary.

## Transactions = `BEGIN/COMMIT` blocks

SurrealDB has no `tx.commit()` in the SDK. Atomicity is via `BEGIN; ...; COMMIT;` blocks in a single `db.query()` call. Every multi-statement write must be wrapped this way (matches the SQLite-side fixes for delete_entity, store_knowledge, archiver).

## File responsibility

- `connection.rs`: only place that interprets URL strings.
- `schema/`: only place that issues `DEFINE` statements.
- `kg/*.rs` and `memory/*.rs`: only places that issue runtime queries. Each file owns one logical cluster of trait methods. Keep each file under ~300 LoC.

## Testing

- Unit tests use `connect("mem://")` (no file I/O, parallel-safe).
- Conformance suite (`stores/zero-stores-conformance`) runs against both SQLite and Surreal.
- ≥ 90% line coverage on this crate per `cargo llvm-cov` is the merge bar.
