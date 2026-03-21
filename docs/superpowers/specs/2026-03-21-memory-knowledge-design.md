# Memory & Knowledge System Design

## Problem

The agent has a memory system (49 facts, all with embeddings) but doesn't learn effectively:
- Knowledge graph is empty (0 entities, 0 relationships) — distillation isn't populating it
- Only 1 out of 49 facts is a genuine learning (the rest are resource indexes)
- Ward memory is a siloed JSON file (`.ward_memory.json`) nobody reads
- Recall only fires at fresh session start, doesn't use the knowledge graph
- No memory taxonomy — everything is a flat "fact" with no distinction between enduring knowledge and stale data
- Entity dedup broken — distillation creates new UUIDs each time instead of upserting by name

## Design

### Memory Taxonomy

Replace the current flat categories (`preference, decision, pattern, entity, instruction, correction`) with:

| Category | What it stores | Examples | Decay |
|---|---|---|---|
| `user` | User preferences, style, capabilities | "Prefers professional HTML reports" | None — permanent |
| `pattern` | How-to knowledge, error workarounds, successful workflows | "yfinance MultiIndex: flatten with `[c[0] for c in columns]`" | Slow — reinforced by reuse |
| `domain` | Domain knowledge with hierarchical subdomains | `domain.finance.lmnd.outlook`, `domain.literature.fahrenheit451.themes` | Medium — recency decay |
| `instruction` | Standing orders, workflow rules | "Always write unit tests", "Use professional chart formatting" | None — permanent |
| `correction` | User corrections to agent behavior | "Don't load all skills at once", "Fix code, don't create _v2" | None — permanent |

Key format: `{category}.{domain}.{subdomain}.{topic}` (dot-notation hierarchy)

### Knowledge Graph Taxonomy

**Entity types** (aligned with existing `EntityType` enum):
- `person` — users, contacts
- `organization` — companies analyzed (LMND, AMD, etc.) — NOT "company"
- `project` — wards and their purposes
- `tool` — skills, agents, MCPs
- `concept` — domain concepts (RSI, options chain, etc.)
- `file` — important ward files (add to `EntityType` enum)

**Relationship types** (use existing enum names where possible, `Custom` for new):
- `related_to` — general association (existing)
- `uses` — agent/project uses tool (existing)
- `created` — entity created by project/session (existing)
- `part_of` — entity belongs to larger entity (existing)
- `is_in` — company in sector (Custom)
- `has_module` — project has file (Custom)
- `exports` — file exports function (Custom)
- `prefers` — user prefers approach (Custom)
- `analyzed_by` — entity analyzed by project (Custom)

**Entity dedup fix**: Before creating an entity, lookup by `(agent_id, entity_type, name)`. If found, increment `mention_count` and update `last_seen_at`. If not found, create new. This prevents duplicate entities across sessions.

### Ward Memory Normalization

**Delete `.ward_memory.json`** as a concept. Remove from:

1. `runtime/agent-tools/src/tools/ward.rs` — remove `.ward_memory.json` creation/reading
2. `runtime/agent-tools/src/tools/memory.rs` — remove `"ward"` from scope enum, delete the `"ward"` arm from `resolve_memory_path()`
3. `memory-bank/architecture.md`, `memory-bank/decisions.md` — update docs

Ward memory is now:
- **AGENTS.md** — human-readable index (structure, modules, conventions)
- **Memory facts in DB** — searchable summaries pointing to ward content
- **Knowledge graph** — ward relationships (has_module, exports, etc.)

### Episodic Facts Strategy

**Ward files hold data** (CSV, JSON, intermediate results).
**DB holds insights** extracted by distillation:
```
domain.finance.lmnd.outlook = "Bullish short-term, RSI overbought at 74.9"
domain.finance.lmnd.data_available = "Ward financial-analysis/stocks/lmnd: prices.csv, fundamentals.json"
```

Decay via recency scoring (already in hybrid search): `1.0 / (1.0 + days_old * 0.01)`.

### Distillation Improvements

**Prompt update**: New taxonomy, richer extraction, raised cap from 10 to 20.

**On-disk prompt migration**: Delete existing `config/distillation_prompt.md` so the new default gets written on next run. Document this in release notes.

**Entity dedup in code**: In `distillation.rs`, before `store_entity()`, call `search_entities(agent_id, name)`. If found, reuse the existing entity ID and bump mention_count. If not, create new.

**Distillation output schema**:
```json
{
  "facts": [
    {"category": "pattern", "key": "pattern.yfinance.multiindex", "content": "...", "confidence": 0.9}
  ],
  "entities": [
    {"name": "LMND", "type": "organization", "properties": {"sector": "insurance"}}
  ],
  "relationships": [
    {"source": "LMND", "target": "insurance", "type": "is_in"}
  ]
}
```

Max 20 facts, 20 entities, 20 relationships per session.

### Recall Improvements

**1. Wire `recall_with_graph()`**: The `MemoryRecall` is already constructed with `with_graph()` in state.rs. Change runner.rs to call the graph-enriched method instead of basic `recall()`.

**2. Ward-aware recall**: When intent analysis recommends a ward, prepend the ward/domain name to the recall query so BM25 + vector search ranks domain facts higher:
```
recall_query = f"{ward_name} {user_message}"
```

**3. Recall at continuation**: Thread `memory_recall` into `invoke_continuation()` signature. Recall domain-relevant facts when root resumes after delegations.

### Compaction & Memory Protection

**Level 1 (exists):** Recalled facts as system message — preserved by compaction.
**Level 2 (exists):** Pre-compaction memory flush warning.
**Level 3 (stretch):** Memory-aware compaction — mini-distillation before trimming.

### File Changes

| File | Change |
|---|---|
| `gateway/gateway-execution/src/distillation.rs` | Update DEFAULT_DISTILLATION_PROMPT (new taxonomy, 20 cap). Entity dedup (lookup before insert). |
| `runtime/agent-tools/src/tools/memory.rs` | Update valid_categories to `[user, pattern, domain, instruction, correction]`. Remove `"ward"` scope. |
| `runtime/agent-tools/src/tools/ward.rs` | Remove `.ward_memory.json` creation/reading. |
| `gateway/gateway-execution/src/runner.rs` | Switch `recall()` to `recall_with_graph()`. Thread `memory_recall` into `invoke_continuation`. |
| `gateway/gateway-execution/src/recall.rs` | Ward-aware recall query enrichment. |
| `services/knowledge-graph/src/types.rs` | Add `File` to EntityType enum. |
| `services/knowledge-graph/src/storage.rs` | Entity lookup-before-insert for dedup. |
| `gateway/templates/shards/memory_learning.md` | Update with new taxonomy (atomic with validator change). |
| `memory-bank/architecture.md` | Remove `.ward_memory.json` references. |
| `memory-bank/decisions.md` | Remove `.ward_memory.json` references. |

### Migration

- Delete `config/distillation_prompt.md` on upgrade (or let user delete manually)
- Existing facts with old categories (`preference`, `decision`, `entity`) remain searchable — no migration needed, they just won't match the new validator for new saves
- Knowledge graph: no schema change — entity dedup is code-level (lookup before insert)
- `.ward_memory.json` files left on disk but no longer read/written

### Testing

| Test | What it verifies |
|---|---|
| Distillation extracts new categories | Session → domain.finance.* facts in DB |
| Distillation populates knowledge graph | Entities and relationships created |
| Entity dedup works | Same entity name across sessions → single entity, mention_count incremented |
| Graph-enriched recall | recall_with_graph returns facts + connected entities |
| Ward file summaries indexed | "data_available" fact points to ward files |
| Old categories rejected by validator | save_fact("preference", ...) returns error |
| New categories accepted | save_fact("domain", ...) succeeds |
| Ward scope removed | memory(scope="ward") returns error |
| Recall at continuation | Facts available when root resumes after delegations |
| Memory shard matches validator | memory_learning.md examples use new categories |
