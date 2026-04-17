# Session Context Bundle — Memory-as-Ctx

**Status:** Draft · awaiting approval
**Author:** session-spec
**Date:** 2026-04-17
**Target branch:** feature/session-ctx-bundle (to be created)

## Motivation

Subagents in AgentZero currently cold-start each delegation. They cannot see what other subagents in the same session produced, which causes:

- **Code duplication.** GOOG session wrote `goog-relative-valuation.py` + `goog_relative_valuation.py` + a third report-generator despite `analysis/relative_valuation.py::get_multiples(ticker)` already existing and being ticker-agnostic.
- **Context rot.** Each subagent's context accumulates tool results over its turns (Step 5: 35 turns, 524K billed tokens, 15.6K of replayed yfinance JSON).
- **Drift from intent.** Subagents receive the step's task description but no canonical view of the original user ask, the intent-analyzer's decision, or prior steps' outputs.

The memory subsystem already holds global facts and recall works. We can promote it to the session's shared-state substrate without adding a filesystem layer.

## Non-goals

- Fixing planner's Skill-First compliance (covered separately by pre-flight checklist + policies).
- Reducing per-turn tool-result bloat (separate optimization — tool-result summarization).
- Cross-session persistence of ctx (session-scoped with TTL; not a long-term store).
- AST-extracted code inventory — explicitly deferred. If duplication persists after this bundle ships, revisit.

## Architecture

```
┌────────────────────────────────────────────────────────────────┐
│  Session lifecycle events  ────►  writer.* hooks  ─────┐       │
│   • session created                                    │       │
│   • intent analyzed                                    │       │
│   • ward entered                                       │       │
│   • planner returned                                   │       │
│   • subagent respond()                                 │       │
│   • session archived (TTL)                             │       │
└────────────────────────────────────────────────────────────────┘
                                                         │
                                   writes (owner-checked)▼
┌────────────────────────────────────────────────────────────────┐
│  memory_facts table                                            │
│    • category='ctx'                                            │
│    • scope='session'                                           │
│    • key='ctx.<sid>.<sub_key>'                                 │
│    • properties.owner ∈ {'root', 'subagent:<id>'}              │
│  ─ firewalled from distillation (cannot be read OR written)    │
│  ─ excluded from fuzzy recall unless query includes session_id │
└────────────────────────────────────────────────────────────────┘
                                                         ▲
                                              reads via  │
┌────────────────────────────────────────────────────────────────┐
│  memory tool, two modes                                        │
│    get(key='ctx.<sid>.<k>')   → exact bytes, no ranking        │
│    recall(query, session_id?) → fuzzy; ctx excluded unless sid │
└────────────────────────────────────────────────────────────────┘
                                                         ▲
┌────────────────────────────────────────────────────────────────┐
│  Static shard (cached, same for every subagent)                │
│    gateway/templates/shards/session_ctx.md                     │
│    — tells agents how to use memory(get) for ctx keys          │
│                                                                │
│  Dynamic preamble (per delegation, ~30 tokens)                 │
│    <session_ctx sid="…" ward="…" step="3/7"                    │
│                 prior_states="exec-abc,exec-def" />            │
│  — injected into task by spawn.rs                              │
└────────────────────────────────────────────────────────────────┘
```

## Data model

No schema migration. Existing `memory_facts` table holds ctx facts via new values in existing columns.

| Column | Value for ctx facts |
|---|---|
| `category` | `'ctx'` (new string, existing column) |
| `scope` | `'session'` (new string, existing column) |
| `session_id` | the session's ID |
| `key` | `ctx.<sid>.<sub_key>` — sid prefix prevents cross-session collision |
| `agent_id` | the writing agent (root for root-owned, subagent id otherwise) |
| `properties` | JSON — includes `owner: 'root' | 'subagent:<id>'` + type-specific metadata |
| `content` | the textual ctx content (markdown, JSON-encoded, or structured block) |
| `pinned` | `true` for root-owned canonical facts (intent, prompt, plan) |

### Key namespace

| Sub-key | Owner | What |
|---|---|---|
| `session.meta` | root | JSON: sid, ward, started_at, root_agent_id |
| `intent` | root | intent-analyzer's output — ward pick, approach, skill matches |
| `prompt` | root | original user message verbatim |
| `plan` | root | contents of `specs/<task>/plan.md` when planner returns |
| `ward_briefing` | root | ward-tree snapshot (optional; cached once per session) |
| `state.<execution_id>` | subagent | handoff summary from one completed subagent execution |

### Write permissions

Enforced at `memory_repository.save_fact` when the call originates from a delegated subagent (`app:is_delegated=true` in tool context):

- **REJECT** if `category='ctx'` AND key sub-part is one of `{session.meta, intent, prompt, plan, ward_briefing}` (root-owned).
- **REJECT** if `category='ctx'` AND key starts with `ctx.<sid>.state.` but the target `execution_id` does not match the caller's own `execution_id`.
- **ALLOW** if `category='ctx'` AND key is `ctx.<sid>.state.<caller_execution_id>`.
- Non-ctx writes pass through unchanged.

Root always passes (not delegated).

### Fuzzy-recall filter

`recall_facts_prioritized(agent_id, query, limit, session_id: Option<String>)`:

- If `session_id` is `None`: append `AND NOT (category = 'ctx')` to the WHERE clause.
- If `session_id` is `Some(sid)`: append `AND (category != 'ctx' OR (category = 'ctx' AND session_id = ?))`.

Effect: ctx facts never contaminate cross-session recall. Within-session recall includes them when the caller is session-aware.

## API — memory tool

### Existing actions (unchanged)
- `recall`, `save_fact`, `save`, `forget`

### New action: `get`

```json
{
  "action": "get",
  "key": "ctx.sess-beb261fd.intent"
}
```

Response shape:
```json
{
  "found": true,
  "key": "ctx.sess-beb261fd.intent",
  "content": "…markdown or JSON-encoded text…",
  "owner": "root",
  "created_at": "2026-04-17T10:12:30Z"
}
```

Or on miss:
```json
{ "found": false, "key": "ctx.sess-beb261fd.nonexistent" }
```

### Permission extension on `save_fact`

Subagents calling `save_fact` with category='ctx' are permission-checked per the rules above. On reject:
```json
{ "error": "Subagent cannot write to root-owned ctx key 'intent'. Writes to ctx.state.<your_exec_id> are allowed." }
```

## Auto-population hooks

**New module:** `gateway-execution/src/session_ctx/writer.rs`

Functions:

```rust
pub fn session_meta(sid: &str, ward: &str, root_agent: &str, started: DateTime) -> Result<()>
pub fn intent_snapshot(sid: &str, intent: &IntentAnalysis, prompt: &str) -> Result<()>
pub fn plan_snapshot(sid: &str, plan_md_path: &Path) -> Result<()>
pub fn ward_briefing(sid: &str, ward: &str) -> Result<()>  // optional, lazy
pub fn state_handoff(
    sid: &str,
    execution_id: &str,
    agent_id: &str,
    step_num: Option<u32>,
    completed_at: DateTime,
    respond_payload: &serde_json::Value,
    artifacts: Vec<String>,
) -> Result<()>
pub fn ttl_cleanup(now: DateTime, ttl_days: i64) -> Result<u64>  // returns rows deleted
```

Each writes a single `memory_facts` row with the conventions above. All are idempotent (upsert on key).

**Hook wiring** — 5 edit points, each ~3 lines:

1. `gateway/src/http/chat.rs` or session creator → `session_meta(...)` after session row inserted.
2. `gateway-execution/src/middleware/intent_analysis.rs` → `intent_snapshot(sid, &analysis, &user_prompt)` at end of intent analysis.
3. `gateway-execution/src/delegation/spawn.rs` → `plan_snapshot(sid, &plan_path)` after planner-agent delegation completes and plan file is detected.
4. `runtime/agent-tools/src/tools/respond.rs` → `state_handoff(...)` inside respond handler, right before returning to the parent.
5. `gateway/src/session_archiver.rs` (or the job that archives) → `ttl_cleanup(now, ttl_days)` daily.

### State handoff format

Content field of `ctx.<sid>.state.<execution_id>`:

```markdown
---
execution_id: exec-5bdc1632
agent_id: code-agent
step: 3
completed_at: 2026-04-17T10:24:38Z
artifacts:
  - models/goog-dcf-model.py
  - models/goog-dcf-output.json
imports_used:
  - core/valuation.py::dcf_valuation
  - core/valuation.py::sensitivity_grid
duration_sec: 337
tokens_in: 356660
tokens_out: 21522
---

## What I did
One-paragraph narrative.

## Handoff for next agents
- Peer fundamentals are at `data/goog-peer-fundamentals.json` (keys: ticker, market_cap, ev, fpe, rev, ni, ebitda).
- `analysis/relative_valuation.py::get_multiples(ticker)` already takes a ticker — extend, don't duplicate.
- DCF primitives in `core/valuation.py`: calc_wacc, dcf_valuation, sensitivity_grid.
```

The hook populates frontmatter from the execution record. The narrative body is extracted from the subagent's `respond()` payload (typically a `summary` field). If no narrative is available, frontmatter-only is acceptable.

Content size cap: 2 KB. Larger summaries are truncated with a "[…truncated, see artifacts for detail]" marker.

## Static shard — `gateway/templates/shards/session_ctx.md`

```markdown
<session_ctx>
Every session carries shared context accessible to all agents. The `<session_ctx ... />` tag in your task prefix tells you the runtime values:
- sid: session id (e.g. sess-beb261fd)
- ward: active ward name
- step: which step of the plan you are executing (e.g. 3/7)
- prior_states: execution ids of completed prior steps

To read shared context, call the memory tool:
  memory(action="get", key="ctx.<sid>.<field>")

Available fields:
- intent — the intent analyzer's interpretation of the user's ask
- prompt — the user's original message verbatim
- plan — the current execution plan
- state.<exec_id> — handoff summary from a specific prior step (use an id from prior_states)

Usage rules:
- Read on-demand, not speculatively. Fetch only what you need.
- You cannot write root-owned ctx keys (intent, prompt, plan). Your respond() output auto-populates state.<your_exec_id> — do not write it manually.
- The ctx namespace is session-scoped: reads never leak from other sessions.
</session_ctx>
```

Loaded into every subagent's system prompt at spawn time. Static — cache-friendly.

## Dynamic preamble

**New module:** `gateway-execution/src/session_ctx/preamble.rs`

```rust
pub fn build(
    sid: &str,
    ward: &str,
    step_current: Option<u32>,
    step_total: Option<u32>,
    prior_execution_ids: &[&str],
) -> String
```

Returns:
```
<session_ctx sid="sess-beb261fd" ward="stock-analysis" step="3/7" prior_states="exec-abc,exec-def" />
```

Prepended to the subagent's user message (task description) in `gateway-execution/src/delegation/spawn.rs`. Dynamic per delegation. ~30 tokens.

## Distillation firewall

**Edits to** `gateway-execution/src/distillation.rs`:

- Write-side: the fact emitter rejects any fact with `category='ctx'`. Logs `warn!` and skips.
- Read-side: the conversation-to-patterns harvester excludes rows where `category='ctx'` from its input query.

~30 lines, two places.

## Agent prompt updates

Each `agents/<name>/AGENTS.md` (template + user install) gets a one-line pointer to the new shard:

```markdown
## Session context

You run inside a session with shared ctx. See the `session_ctx.md` shard for how to query the session's intent, plan, and prior step handoffs via the memory tool.
```

Optional: remove the static "use memory(recall) before discovery" prose from agents that now have `memory(get)` as their primary session-state access path.

## Testing

### Phase 1 unit tests (memory layer)

- `ctx_fact_write_as_root_succeeds` — root can write any ctx key.
- `ctx_fact_write_as_subagent_rejects_root_owned` — subagent writing `ctx.<sid>.intent` returns permission error.
- `ctx_fact_write_as_subagent_allows_own_state` — subagent writing `ctx.<sid>.state.<own_exec_id>` succeeds.
- `ctx_fact_write_as_subagent_rejects_foreign_state` — subagent writing `ctx.<sid>.state.<other_exec_id>` returns error.
- `get_existing_key_returns_content` — exact match returns the row.
- `get_missing_key_returns_not_found` — never returns nearest neighbor.
- `recall_without_session_filter_excludes_ctx` — ctx facts don't appear.
- `recall_with_session_filter_includes_matching_ctx` — session-scoped recall sees them.

### Phase 2 unit tests (writer)

One test per writer function, mock `memory_repository`. Verify the row written has correct category / scope / key / owner / pinned.

### Phase 5 unit tests (distillation firewall)

- `distiller_write_rejects_ctx_category`
- `distiller_read_excludes_ctx_rows`

### Integration test

`tests/session_ctx_e2e.rs` — in-process:
1. Create a session.
2. Run mock intent analysis → verify ctx.intent + ctx.prompt exist.
3. Run mock planner → verify ctx.plan exists.
4. Run a mock subagent's respond() → verify ctx.state.<exec_id> exists and is owned-by-subagent.
5. Call `memory(get, key='ctx.<sid>.intent')` as a subagent → returns content.
6. Call `memory(save_fact, key='ctx.<sid>.intent', content='attack')` as a subagent → rejected.
7. Archive the session → ctx rows removed after TTL.

### Regression tests

- Existing recall tests continue to pass.
- Existing `save_fact` tests for non-ctx categories unchanged.

## Rollout

Phases are independently mergeable. Suggested order:

| Phase | PR | Ships | Acceptance |
|---|---|---|---|
| 1 | `memory-layer-ctx` | get action, scope filter, permission gate | all Phase 1 unit tests pass; no regression on recall |
| 2 | `session-ctx-writers` | auto-population hooks | integration test passes; new ctx facts appear during a real session but nothing reads them yet |
| 4a | `session-ctx-shard` | shard file + subagent system-prompt wiring | shard bytes appear in subagent prompt dumps |
| 4b | `session-ctx-preamble` | dynamic preamble in spawn.rs | preamble bytes appear in subagent task, agents start calling `memory(get)` |
| 5 | `distillation-firewall` | distiller write+read guards | firewall unit tests pass; `category='ctx'` rows never appear in distilled patterns |
| 6 | `agent-prompts-ctx` | AGENTS.md one-liner updates | no behavior change required — just discoverability |

After phase 4b merges, the system is fully wired — agents can use ctx. Phase 5 hardens; Phase 6 docs.

## Open questions / risks

1. **`respond()` payload shape.** We need a convention for which fields go into the state-handoff frontmatter vs narrative body. Propose: respond accepts optional `handoff: { artifacts, imports_used, notes }` object. If absent, the hook writes frontmatter from execution metadata only. Agents that want to be good handoff citizens add the object.

2. **TTL default.** 7 days feels right for debugging old sessions but might be too long at scale. Configurable via `settings.json → execution.ctx.ttlDays`. Default 7.

3. **Shard loading for subagents.** Current `gateway-templates/src/lib.rs` assembles shards for root via `load_system_prompt_from_paths`. Need to extend the subagent prompt assembly path (in `gateway-execution/src/invoke/setup.rs` or equivalent) to append `session_ctx.md` after each subagent's own AGENTS.md. Verify this exists or add it.

4. **Prompt cache stability.** Shard content must be byte-stable across requests for the provider's prompt cache to hit. Putting dynamic values in the user message (not system prompt) preserves this. Confirm by checking z.ai and Anthropic caching behavior.

5. **Recall query cost.** New WHERE clause with `NOT (category = 'ctx')` adds a filter to every recall. Should be index-friendly (category is already selected in queries). Measure before and after.

## Implementation plan — file list

### New files

```
gateway/gateway-execution/src/session_ctx/mod.rs         (~40)
gateway/gateway-execution/src/session_ctx/writer.rs      (~180)
gateway/gateway-execution/src/session_ctx/preamble.rs    (~50)
gateway/gateway-database/src/ctx_queries.rs              (~80)
gateway/templates/shards/session_ctx.md                  (150 lines)
docs/specs/2026-04-17-session-ctx-memory-bundle.md       (this file)
gateway/gateway-execution/tests/session_ctx_e2e.rs       (~150)
```

### Modified files

```
gateway/gateway-database/src/memory_repository.rs        (+60, permission + get + filter)
runtime/agent-tools/src/tools/memory/mod.rs              (+40, new get action)
runtime/agent-tools/src/tools/respond.rs                 (+10, state_handoff hook)
gateway/gateway-execution/src/middleware/intent_analysis.rs (+5, intent_snapshot hook)
gateway/gateway-execution/src/delegation/spawn.rs        (+20, plan_snapshot + preamble + prepend)
gateway/gateway-execution/src/invoke/setup.rs            (+10, append session_ctx shard to subagent prompt)
gateway/gateway-execution/src/distillation.rs            (+30, firewall)
gateway/src/http/chat.rs                                 (+5, session_meta hook)
gateway/src/session_archiver.rs (or equivalent)          (+5, ttl_cleanup hook)
gateway/gateway-services/src/settings.rs                 (+15, CtxConfig)
agents/<each>/AGENTS.md (8 files, template + user)       (+3 lines each)
```

### Total

~640 new lines of Rust + ~150-line shard + ~24 lines of prompt edits across 8 agents × 2 locations. Estimated 10-13 hours of focused work, ~1.5 days.

## Success criteria

After all phases merge:

1. A fresh session produces `ctx.<sid>.intent`, `ctx.<sid>.prompt`, `ctx.<sid>.plan`, and at least one `ctx.<sid>.state.<exec_id>` per completed subagent.
2. Subagents can call `memory(action='get', key='ctx.<sid>.intent')` and receive the bytes.
3. Subagents trying to overwrite root-owned ctx keys are rejected with a clear error.
4. `memory(action='recall', query='...')` without a session filter never returns ctx facts.
5. Distilled pattern facts never contain content sourced from ctx.
6. A replayed GOOG-like session with the new bundle shows code-agent in Step 4 calling `memory(get, key='ctx.<sid>.state.<step3_exec_id>')` and using prior artifacts instead of writing new `goog-*-.py` variants. (Empirical — measured on the next real run, not unit-testable.)
7. TTL cleanup removes ctx rows N days after session completion.

## Sign-off checklist

Before I start Phase 1, please confirm:

- [ ] Agree with `category='ctx'` + `scope='session'` naming.
- [ ] Agree with key convention `ctx.<sid>.<sub_key>`.
- [ ] Agree with root-owned vs subagent-owned sub-key split.
- [ ] Agree with 2 KB cap on state handoff content.
- [ ] Agree with 7-day default TTL (configurable).
- [ ] Agree with the phase order (1 → 2 → 4a → 4b → 5 → 6).
- [ ] Agree to defer Phase 3 (AST code inventory) for now.
