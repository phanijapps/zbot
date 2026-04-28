# zero-stores-surreal-recovery — AGENTS.md

Placeholder crate for recovering corrupt SurrealDB `knowledge.surreal`
directories. Invoked **only** by the `agentzero recover-knowledge` CLI
subcommand. Never auto-invoked by the daemon's startup path.

## Why placeholder

Per the design spec (§7), the daemon refuses to start on corruption. The
recovery flow is a manual user action backed by this crate. The first
implementation does the simplest useful thing (read-only open → JSON
sidecar → rename aside). Smarter strategies (live RocksDB compact-and-repair,
WAL replay) come later if needed.

## Non-goals

- Not auto-recovery. The daemon never invokes this crate.
- Not migration. SQLite → SurrealDB data migration is a separate workstream.
- Not WAL repair. RocksDB's own repair tool is the next escalation.
