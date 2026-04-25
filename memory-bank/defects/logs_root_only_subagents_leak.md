# Defect — `/logs` sidebar leaks subagents under `root_only=true`

## Symptom

The Observability dashboard sidebar at `/logs` requests
`GET /api/logs/sessions?limit=100&root_only=true` and is supposed to show
only root sessions. In practice, subagent executions (`builder-agent`,
`planner-agent`, `research-agent`, `writing-agent`, …) leak through and
appear as their own list rows alongside the roots.

Confirmed live in dev (`http://localhost:3000/logs`) on 2026-04-25 — out
of 19 returned rows, 3 were subagents whose `parent_session_id` field
pointed at a sibling root row.

## Reproduction

1. Run the gateway with a real conversations DB that contains historical
   delegation runs (typical dev workstation).
2. Open `/logs` in the browser.
3. Inspect the network response for `/api/logs/sessions?root_only=true`.
   Some rows will have `agent_id` ≠ `"root"` and `parent_session_id` set
   to the parent root's exec id.

## Root cause

The repository SQL at `services/api-logs/src/repository.rs::list_sessions`
*used to* implement `root_only` like this:

```sql
LEFT JOIN agent_executions ae ON ae.id = e.session_id
WHERE 1=1
  AND ae.parent_execution_id IS NULL   -- when root_only
GROUP BY e.session_id
```

That filter relies on the `agent_executions` row of every execution being
present and authoritative. Probing real production data
(`~/Documents/zbot/data/conversations.db`):

- 5 of 52 distinct `execution_logs.session_id` values had **no matching
  `agent_executions` row at all** (older runtime paths, crash recovery,
  pre-migration data). For those rows the LEFT JOIN resolved
  `ae.parent_execution_id` to NULL, the WHERE filter accepted them, and
  the subagent leaked into the list.
- A second, independent issue: 36 sessions had a **mixed** distribution
  of `parent_session_id` across their log rows — one row with NULL
  (often the first init log, before parent context is wired) and the
  rest set. A pre-aggregation `WHERE parent_session_id IS NULL` filter
  would still let the NULL row pass and `GROUP BY` would emit the
  subagent.

A pre-existing comment on `LogsRepository::get_session` claimed
`execution_logs.parent_session_id` was unreliable for subagents — that
comment was **stale and inverted from reality**. The probe showed
`parent_session_id` is reliably populated for at least one row of every
real subagent (and for ALL rows of most). That column is the trustworthy
signal; `agent_executions.parent_execution_id` is not.

## Fix

Two changes, both required:

1. **Gateway (authoritative)** — `services/api-logs/src/repository.rs`:
   replace `WHERE ae.parent_execution_id IS NULL` with a HAVING clause
   over the aggregate. SQLite's `MAX` ignores NULLs, so
   `MAX(parent_session_id) IS NULL` is true iff *every* log row of the
   group has parent NULL — i.e. a real root.

   ```sql
   GROUP BY e.session_id
   HAVING MAX(e.parent_session_id) IS NULL    -- when root_only
   ORDER BY started_at DESC
   ```

2. **UI defense-in-depth** — `apps/ui/src/features/logs/log-hooks.ts`
   (`useLogSessions`): when `root_only` is requested, drop any row
   whose `parent_session_id` is non-empty before storing the list.
   Mirrors `apps/ui/src/features/research-v2/useSessionsList.ts:120-123`.
   If the gateway query ever regresses again, the sidebar still stays
   clean.

The stale comment on `LogsRepository::get_session` was rewritten in the
same commit so future readers don't get a wrong steer.

## Tests locking the regression

- `services/api-logs/src/repository.rs` (4 tests):
  - `root_only_excludes_subagent_with_no_agent_executions_row`
  - `root_only_excludes_subagent_with_mixed_null_parent_session_id_rows`
  - `root_only_includes_pure_root_session`
  - `root_only_off_returns_subagents_too`
- `apps/ui/src/features/logs/log-hooks.test.ts` (3 tests):
  - filter drops `parent_session_id`-set rows when `root_only=true`
  - filter is a no-op when `root_only=false`
  - treats empty-string `parent_session_id` as a root

The Rust tests previously did not seed `agent_executions` rows at all,
which is why the filter passed in CI even though it was broken in
production. The new tests seed both tables and explicitly reproduce the
no-`agent_executions`-row and mixed-parent-NULL scenarios observed in
real data.

## Related

- `apps/ui/src/features/research-v2/useSessionsList.ts:120-123` — the
  research drawer's filter; the new UI filter mirrors it.
- `defect_session_status_inheritance.md` — another root/subagent
  ambiguity in the same area; different symptom, related schema lesson.
