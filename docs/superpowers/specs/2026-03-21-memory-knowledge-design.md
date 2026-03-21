# Memory & Knowledge System Design

## Problem

The agent has a memory system (49 facts, all with embeddings) but doesn't learn effectively:
- Knowledge graph is empty (0 entities, 0 relationships) — distillation isn't populating it
- Only 1 out of 49 facts is a genuine learning (the rest are resource indexes)
- Ward memory is a siloed JSON file (`.ward_memory.json`) nobody reads
- Recall only fires at fresh session start, doesn't use the knowledge graph
- No memory taxonomy — everything is a flat "fact" with no distinction between enduring knowledge and stale data
- Compaction can discard recalled context mid-session

## Design

### Memory Taxonomy

Replace the current flat categories (`preference, decision, pattern, entity, instruction, correction`) with a structured taxonomy:

| Category | What it stores | Examples | Decay |
|---|---|---|---|
| `user` | User preferences, style, capabilities | "Prefers professional HTML reports", "Experienced engineer" | None — permanent |
| `pattern` | How-to knowledge, error workarounds, successful workflows | "yfinance MultiIndex: flatten with `[c[0] for c in columns]`", "data-analyst + yf-data works for stock analysis" | Slow — reinforced by reuse |
| `domain` | Domain knowledge with hierarchical subdomains | `domain.finance.lmnd.outlook`, `domain.literature.fahrenheit451.themes` | Medium — can go stale |
| `correction` | User corrections to agent behavior | "Don't load all skills at once", "Fix code, don't create _v2" | None — permanent |

Key format uses dot-notation for hierarchy: `{category}.{domain}.{subdomain}.{topic}`

Examples:
```
user.report_style = "Professional HTML with charts and detailed explanations"
pattern.yfinance.multiindex = "Flatten MultiIndex columns: df.columns = [c[0] for c in df.columns]"
pattern.workflow.stock_analysis = "data-analyst + yf-data + yf-signals + coding skills"
domain.finance.lmnd.sector = "Insurance - Property & Casualty, market cap ~$4.9B"
domain.finance.lmnd.outlook = "Bullish short-term, RSI overbought at 74.9"
domain.finance.lmnd.data_available = "Ward financial-analysis/stocks/lmnd: prices.csv, fundamentals.json"
domain.literature.fahrenheit451.themes = "Censorship, self-expression, technology, happiness"
correction.coding.no_v2 = "Never create _v2 or _improved copies. Fix the original file."
```

### Knowledge Graph Taxonomy

The graph captures **relationships** that flat facts can't express:

**Entity types:**
- `person` — users, contacts
- `company` — companies analyzed (LMND, AMD, etc.)
- `project` — wards and their purposes
- `tool` — skills, agents, MCPs
- `concept` — domain concepts (RSI, options chain, etc.)
- `file` — important ward files (core modules, data files)

**Relationship types:**
- `is_in` — company → sector ("LMND" → "insurance")
- `has_module` — project → file ("financial-analysis" → "core/data_fetch.py")
- `exports` — file → concept ("core/data_fetch.py" → "get_ohlcv()")
- `uses` — agent/project → tool ("stock analysis" → "yf-data skill")
- `prefers` — person → concept ("user" → "HTML reports")
- `related_to` — concept → concept ("RSI" → "overbought signal")
- `analyzed_by` — company → project ("LMND" → "financial-analysis ward")

### Ward Memory Normalization

**Delete `.ward_memory.json`** as a concept. Ward memory is:

1. **AGENTS.md** — the human-readable index of what's in the ward. Structure, modules, conventions, data files. Updated by root after every session.

2. **Memory facts in DB** — searchable summaries that point to ward content:
   ```
   domain.finance.lmnd.data_available = "Ward financial-analysis/stocks/lmnd: prices.csv (1yr OHLCV), fundamentals.json, options_chain.csv"
   ```

3. **Knowledge graph** — ward relationships:
   ```
   financial-analysis → has_module → core/data_fetch.py
   core/data_fetch.py → exports → get_ohlcv(), save_json()
   LMND → analyzed_by → financial-analysis ward
   ```

The agent reads AGENTS.md for details, recalls DB facts for discovery, traverses the graph for connections.

### Episodic Facts Strategy

**Ward files hold data.** CSV, JSON, intermediate results — these are reference material.

**DB holds insights.** Distillation extracts conclusions from the data:
- "LMND bullish short-term, RSI 74.9" → `domain.finance.lmnd.outlook`
- Not the raw price history — that stays in `stocks/lmnd/prices.csv`

**Decay via recency scoring.** The hybrid search already applies recency decay: `1.0 / (1.0 + days_old * 0.01)`. Old domain facts naturally rank lower. No explicit expiration needed — stale facts get outranked by fresh ones on the same topic (upsert by key updates the timestamp).

### Distillation Improvements

The distillation prompt needs to extract all three tiers:

**Facts** — with the new taxonomy:
```json
{"category": "pattern", "key": "pattern.yfinance.multiindex", "content": "Flatten MultiIndex: [c[0] for c in df.columns]", "confidence": 0.9}
{"category": "domain", "key": "domain.finance.lmnd.outlook", "content": "Bullish short-term, support at $58", "confidence": 0.7}
{"category": "domain", "key": "domain.finance.lmnd.data_available", "content": "Ward financial-analysis/stocks/lmnd: prices.csv, fundamentals.json", "confidence": 1.0}
```

**Entities** — richer types:
```json
{"name": "LMND", "type": "company", "properties": {"sector": "insurance", "ticker": "LMND"}}
{"name": "core/data_fetch.py", "type": "file", "properties": {"ward": "financial-analysis", "exports": ["get_ohlcv", "save_json"]}}
{"name": "financial-analysis", "type": "project", "properties": {"domain": "finance"}}
```

**Relationships:**
```json
{"source": "LMND", "target": "insurance", "type": "is_in"}
{"source": "financial-analysis", "target": "core/data_fetch.py", "type": "has_module"}
{"source": "core/data_fetch.py", "target": "get_ohlcv", "type": "exports"}
{"source": "LMND", "target": "financial-analysis", "type": "analyzed_by"}
```

### Recall Improvements

**Current:** Only fires at fresh session start. Doesn't use graph. Max 10 facts.

**Proposed:**

1. **Use `recall_with_graph()`** — already implemented, just not wired. Switch runner to call it instead of `recall()`. Enriches recalled facts with entity connections.

2. **Ward-aware recall** — when the intent analysis recommends a ward, recall facts scoped to that domain:
   ```
   recall("LMND analysis")
   → domain.finance.lmnd.* facts
   → pattern.workflow.stock_analysis
   → graph: LMND → is_in → insurance, LMND → analyzed_by → financial-analysis
   ```

3. **Recall at continuation too** — not just fresh sessions. When the root resumes after delegations, recall domain-relevant facts to refresh context.

### Compaction & Memory Protection

**Level 1 (exists):** Recalled facts injected as `system` message — preserved by compaction.

**Level 2 (exists):** Pre-compaction "MEMORY FLUSH" warning gives agent one turn to save important facts.

**Level 3 (stretch goal):** Memory-aware compaction:
- Before trimming messages, extract key facts via mini-distillation
- Save to memory_facts
- Replace trimmed messages with compact summary referencing saved facts
- Recalled facts system message pinned (never trimmed)

### File Changes

| File | Change |
|---|---|
| `gateway/gateway-execution/src/distillation.rs` | Update DEFAULT_DISTILLATION_PROMPT with new taxonomy, richer entity/relationship extraction |
| `runtime/agent-tools/src/tools/memory.rs` | Update valid_categories to new taxonomy (`user, pattern, domain, correction`), remove `.ward_memory.json` fallback |
| `gateway/gateway-execution/src/runner.rs` | Switch `recall()` to `recall_with_graph()`, add recall at continuation |
| `gateway/gateway-execution/src/recall.rs` | Enhance formatting to include graph context clearly |
| `gateway/templates/shards/memory_learning.md` | Update with new taxonomy and examples |
| `runtime/agent-tools/src/tools/ward.rs` | Remove `.ward_memory.json` creation/reading, AGENTS.md is the ward memory |

### What NOT to change

- `memory_facts` table schema — no schema migration needed. Category is a TEXT column, keys are TEXT. New taxonomy works with existing schema.
- `MemoryFactStore` trait — stays the same.
- Hybrid search algorithm — already good.
- Embedding infrastructure — already working.
- Knowledge graph schema — already supports the entity/relationship types we need.

### Testing

| Test | What it verifies |
|---|---|
| Distillation extracts new categories | Session with stock analysis → domain.finance.* facts |
| Distillation populates knowledge graph | Entities and relationships created after session |
| Graph-enriched recall | recall_with_graph returns facts + connected entities |
| Ward file summaries indexed | "data_available" fact points to ward files |
| Old categories rejected | save_fact("preference", ...) returns error |
| Recall at continuation | Facts available when root resumes after delegations |
| Ward memory normalized | No .ward_memory.json created, AGENTS.md used instead |
