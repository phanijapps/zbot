# Defect — Cron-fired sessions appear in the Research list

**Severity:** Low (cosmetic, no data loss)
**Discovered:** 2026-05-03 after default cleanup schedule shipped (PR #96)
**Status:** Open, deferred

## Symptom

Sessions triggered by cron jobs (e.g. the bundled `default-cleanup`
schedule) show up in the `/research` drawer alongside genuine research
sessions, polluting the user's research history with system-maintenance
runs that fire every 4 hours.

## Reproduction

1. Daemon running with PR #96 merged — `default-cleanup` cron fires
   every 4h via `gateway/src/cron/mod.rs::schedule_job`.
2. Wait for at least one cron tick, or manually trigger via
   `POST /api/cron/default-cleanup/trigger`.
3. Open `/research` — the cron-spawned session appears as a row.

## Root cause

`apps/ui/src/services/session-kind.ts:48` (`isChatSession`) classifies
sessions into two buckets only: chat-mode (`mode === 'fast' | 'chat'` or
`conversation_id` starts with `sess-chat-`) and "everything else, treat
as research." The drawer at
`apps/ui/src/features/research-v2/useSessionsList.ts:120-123` filters
`!isChild && !isChatSession(row)`, so any non-chat root session becomes
a research session by default.

Cron sessions carry `TriggerSource::Cron` on the backend
(`gateway/src/cron/mod.rs:159` — `.with_source(TriggerSource::Cron)`)
but the wire shape `LogSession` at
`apps/ui/src/services/transport/types.ts:573-597` does not surface
`trigger_source` at all. The UI literally cannot distinguish a cron run
from a research session.

## Suggested fix (~50 lines)

### Backend (~15 lines)

Add `trigger_source: Option<String>` to `LogSession` in
`gateway/api-logs/src/repository.rs` (or wherever `LogSession` is
serialized). The value already exists on the underlying execution
record; just thread it through to the JSON.

### Frontend (~35 lines)

1. Add `trigger_source` to the `LogSession` TS interface in
   `apps/ui/src/services/transport/types.ts:573-597`.
2. Add `isSystemSession(row)` to
   `apps/ui/src/services/session-kind.ts` returning
   `row.trigger_source === "cron"` (extend later for other system
   sources).
3. Update the filter in
   `apps/ui/src/features/research-v2/useSessionsList.ts:120-123`:
   ```ts
   return !isChild && !isChatSession(row) && !isSystemSession(row);
   ```
4. Optional: surface a "System" tab in the drawer for users who want to
   see cron-triggered runs.

## Acceptance criteria

- Cron-fired sessions no longer appear in `/research` drawer rows.
- Chat sessions still excluded; non-system research sessions still
  visible.
- A system-level filter / tab can show the cron runs if the user wants.

## Out of scope

- Categorizing other `TriggerSource` values (Webhook, Connector, etc.)
  beyond Cron — those have their own surfaces and may not need the same
  filter.
