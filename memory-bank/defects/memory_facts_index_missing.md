# Defect — `memory.recall` tool errors with missing `memory_facts_index`

## Symptom

Every `memory` tool call with `action: "recall"` fails on the first run of
a session, emitting this tool error into the WebSocket feed:

```
Error: Tool execution failed: Tool("Knowledge DB operation failed: no such table: memory_facts_index")
```

Visible in the `/research-v2` StatusPill as a sticky red "Tool error"
(see R14e), and persisted in the session's message log as a `role: "tool"`
row immediately after the recall tool_call.

## Reproduction

Load any recent research session and query its messages — the error is
captured in the DB. Two examples from live sessions:

- `sess-d22f0b91-150b-4ae1-91f7-1acc04e94c11` (TSLA vs Ford) — fires
  twice, once at the start of root and once later.
- `sess-c962f36a-f4d8-41c6-8892-f037bd060f39` (NVDA vs AMD) — fires at
  least twice in root's execution.

Direct fetch to confirm:

```bash
curl -s "http://localhost:18791/api/sessions/sess-d22f0b91-150b-4ae1-91f7-1acc04e94c11/messages?scope=all" \
  | jq '.data[] | select(.content | test("Knowledge DB operation failed")) | .content'
```

## Scope

`memory_facts_index` is the `vec0` virtual table that backs embedding
lookups for `memory_facts` (see
`memory-bank/components/memory-layer/data-model.md:569` —
`CREATE VIRTUAL TABLE memory_facts_index USING vec0(fact_id TEXT
PRIMARY KEY, embedding FLOAT[384])`).

When the vec0 table is absent, every recall attempt fails with
`no such table: memory_facts_index`.

## Likely root cause — hypotheses

1. **Migration not run for existing DBs.** The vec0 table was added in a
   later schema version (v13 / v14-ish per the version history in
   `data-model.md`); DBs created before that and not migrated still
   lack the table.
2. **sqlite-vec extension failed to load at startup.** `vec0` is an
   extension-provided virtual table; if the extension isn't loaded the
   `CREATE VIRTUAL TABLE` statement in the migration silently no-ops or
   errors. Check gateway startup logs for `SqliteVecIndex::new` or
   `sqlite_vec` warnings.
3. **Per-ward conditional creation.** If `memory_facts_index` is created
   only for specific wards, new-ward first-call races the creation.

The test fixture in
`/home/videogamer/projects/agentzero/gateway/gateway-database/tests/vector_index.rs:50`
proves the table name and type are correct (`vec0`, PK `fact_id`,
embedding `FLOAT[384]`). The issue is that the runtime DB wasn't migrated
to include it.

## Impact on the user

- First recall always misses — the agent loses pre-existing `memory_facts`
  context and starts cold.
- Tool error clutters the research-v2 news ticker even when the session
  is otherwise succeeding.
- Agent may still complete the task (it did in both NVDA and TSLA
  sessions), but quality is silently degraded — the "memory layer" isn't
  actually being read.

## Fix approach (proposed, not yet applied)

1. **Check gateway DB migrations at startup**: verify `memory_facts_index`
   exists in the current DB; if not, run the missing vec0 migration.
2. **Guard tool call**: in the `memory.recall` handler, detect the
   "no such table" error and return an empty recall result with a
   `degraded_mode: true` flag instead of propagating a fatal tool error.
   Log once per session so the error doesn't spam the ticker.
3. **Gate vec0 presence**: if the extension fails to load at startup,
   disable the recall feature globally and emit a single startup warning,
   rather than firing per-call errors.

## Files to audit

- `gateway/gateway-database/src/**` — migration scripts, `SqliteVecIndex`
  setup.
- `gateway/agent-tools/src/tools/memory.rs` (or similar) — the recall
  tool handler that emits the error.
- `runtime/agent-runtime/src/tools/memory.rs` — possibly the source of
  the error wrapping.

## Related

- R14e (LLM + tool errors in StatusPill) — the ticker surfacing this is
  already working; fixing the underlying tool is independent.
- `memory-bank/components/memory-layer/data-model.md:565-580` — vec0
  table definition.
