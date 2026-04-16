-- gateway/gateway-database/migrations/v24_global_scope_backfill.sql
--
-- One-time backfill: promote facts in "global-type" categories from the
-- default scope='agent' to scope='global' so they are visible to every
-- agent via the scope-aware search filter.
--
-- Idempotent: only rows still at the default 'agent' scope are touched;
-- explicit agent-scoped writes (corrections/strategies/instructions/
-- patterns) are left alone, and re-running is a no-op once applied.
UPDATE memory_facts
   SET scope = 'global'
 WHERE scope = 'agent'
   AND category IN ('domain', 'reference', 'book', 'research', 'user');
