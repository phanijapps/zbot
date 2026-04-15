# Memory Tab — Command Deck Redesign & Hybrid Search

**Date:** 2026-04-15
**Status:** Design approved, awaiting plan
**Branch target:** `feature/memory-tab-command-deck`

## Problem

The current Memory tab (`apps/ui/src/features/memory/MemoryTab.tsx`) surfaces a single agent's `memory_facts` as a flat list filtered by category. Three gaps make it inadequate:

1. **No ward lens.** Wards hold wiki articles, procedures, and session episodes that the Memory tab can't show. Users have to navigate elsewhere (or query the DB) to see ward-scoped knowledge.
2. **Search is keyword-only.** The UI calls `search_memory_facts_fts` (`gateway/src/http/memory.rs:173`) even though the backend already implements `search_memory_facts_hybrid` (FTS + sqlite-vec + rank fusion). Semantic queries miss obvious matches.
3. **No temporal discipline in the UI.** Financial analyses decay; historical is still useful but should read as historical. The current list treats everything as equally fresh.

## Solution

**Command Deck layout** — ward-first three-column shell: wards rail (left), tabbed content per ward (center), write + stats rail (right). Content fades by recency. Search upgrades to hybrid by default.

Carries over the existing style tokens (`--color-*`, `--spacing-*`, `--radius-*`) and existing category vocabulary (instruction / policy / preference / pattern / correction / decision). No new design language — a reshape of existing primitives.

## Scope

### In scope

- Three-column Command Deck layout replacing `MemoryTab.tsx`
- Per-ward content tabs: **Facts · Wiki · Procedures · Episodes · Graph↗**
- Top bar with hybrid search + scope chips + time-warp slider
- Right-rail persistent write surface (`+ Fact`, `+ Instruction`, `+ Policy`) using existing `save_fact` transport
- Temporal fade: `TODAY` / `LAST 7 DAYS` / `HISTORICAL` grouping, opacity decays with age
- HTTP swap: `search_memory_facts` → call `_hybrid`, accept optional `fts_only`/`semantic_only` mode
- Wiki FTS table (new): enables hybrid search over wiki articles
- "Why it matched" badge per result: `hybrid` / `fts` / `vec` / `title`
- Graph↗ tab button opens existing `GraphView` scoped to current ward

### Out of scope

- Constellation layout (option B) — existing `GraphView` already covers this; enhancements go on `GraphView`, not a new surface
- Procedures FTS — description fields are too short to benefit; vector-only is fine
- Agent Diary MCP integration (separate concern)
- Ontology-pack validator (deferred — tracked in a separate spec)
- Retroactive backfill of per-book `knowledge-graph/*.json` files into graph (deferred)

## Architecture

### Component tree

```
MemoryTab (redesigned)
├── SearchBar                  — hybrid search, mode toggle, shortcuts
├── ScopeChips                 — ward / category / confidence filters
├── WardRail                   — left column: ward list + active indicator + counts
├── ContentDeck                — center column
│   ├── WardSummary            — breadcrumb + ward metadata
│   ├── ContentTabs            — Facts / Wiki / Procedures / Episodes / Graph↗
│   ├── ContentList            — grouped by recency bucket with opacity decay
│   │   └── MemoryItemCard     — reusable item row with kind badge + age + "why" badge
│   └── EmptyState             — per-tab empty copy
└── WriteRail                  — right column: Add buttons + live ward stats
    └── AddDrawer              — modal-ish panel to compose + save a fact/instruction/policy
```

### New files

- `apps/ui/src/features/memory/command-deck/MemoryTab.tsx` — shell
- `apps/ui/src/features/memory/command-deck/WardRail.tsx`
- `apps/ui/src/features/memory/command-deck/SearchBar.tsx`
- `apps/ui/src/features/memory/command-deck/ScopeChips.tsx`
- `apps/ui/src/features/memory/command-deck/ContentDeck.tsx`
- `apps/ui/src/features/memory/command-deck/ContentList.tsx`
- `apps/ui/src/features/memory/command-deck/MemoryItemCard.tsx`
- `apps/ui/src/features/memory/command-deck/WriteRail.tsx`
- `apps/ui/src/features/memory/command-deck/AddDrawer.tsx`
- `apps/ui/src/features/memory/command-deck/hooks.ts` — `useWards`, `useWardContent`, `useHybridSearch`, `useTimewarp`
- `apps/ui/src/features/memory/command-deck/types.ts`
- `apps/ui/src/features/memory/command-deck/__tests__/*.test.tsx`

The existing `MemoryTab.tsx` (241 lines) is kept at the old path but no longer wired into the router. The new export lives at `apps/ui/src/features/memory/command-deck/MemoryTab.tsx`, and the router imports from there. Move the old file to `MemoryTabLegacy.tsx` once the new one lands green.

### Backend changes

#### HTTP handler upgrade (`gateway/src/http/memory.rs`)

`search_memory_facts` currently calls `search_memory_facts_fts`. Swap to `search_memory_facts_hybrid` when query contains no quotes. Accept a new optional query parameter `mode=fts|hybrid|semantic` (default `hybrid`). Quoted substrings within the query force FTS semantics for those tokens.

#### New endpoint — `/api/wards/:ward_id/content`

Aggregates facts + wiki + procedures + episodes for one ward in one round-trip.

Response shape:
```json
{
  "ward_id": "literature-library",
  "summary": { "title": "…", "description": "…", "updated_at": "…" },
  "facts":      [ { id, content, category, confidence, created_at, age_bucket } ],
  "wiki":       [ { id, title, snippet, updated_at, age_bucket } ],
  "procedures": [ { id, name, description, success_count, last_used, age_bucket } ],
  "episodes":   [ { id, task_summary, outcome, created_at, age_bucket } ],
  "counts":     { "facts": 142, "wiki": 7, "procedures": 4, "episodes": 18 }
}
```

`age_bucket` is `today | last_7_days | historical` computed server-side.

#### New endpoint — `/api/memory/search`

Replaces the per-type search pattern. Accepts:
```
{ "query": "…", "mode": "hybrid|fts|semantic", "types": ["facts","wiki","procedures","episodes"], "ward_ids": [...], "filters": { category, confidence_gte } }
```

Returns grouped results:
```json
{
  "facts":      { "hits": [ { item, score, match_source } ], "latency_ms": 180 },
  "wiki":       { "hits": [...], "latency_ms": 95 },
  "procedures": { ... },
  "episodes":   { ... }
}
```

`match_source` ∈ `hybrid | fts | vec | title`.

#### New wiki FTS table

```sql
CREATE VIRTUAL TABLE ward_wiki_articles_fts USING fts5(
  title,
  content,
  content='ward_wiki_articles',
  content_rowid='rowid'
);
-- triggers to keep in sync with ward_wiki_articles
```

Add `search_wiki_hybrid(query, ward_id, limit)` in `wiki_repository.rs` mirroring the existing hybrid pattern.

### Data flow

```
User types query
  ↓
SearchBar debounces 250ms, calls /api/memory/search
  ↓
Backend (for each type ∈ selected types, in parallel):
  hybrid = FTS results ∪ vector results → reciprocal-rank fusion → top K
  ↓
Results grouped per type, annotated with match_source
  ↓
UI renders grouped list with "why" badges, fades by age_bucket
```

Ward-switch flow:
```
Click ward in WardRail → setActiveWardId(id)
  ↓
useWardContent(id) → /api/wards/:ward_id/content (with cache on current agent)
  ↓
ContentDeck paints tabs with counts; ContentList for active tab shows grouped items
```

### Opacity decay rule (CSS)

```css
.memory-item            { opacity: 1;    /* today */ }
.memory-item.decay-1    { opacity: 0.7;  /* last 7 days */ }
.memory-item.decay-2    { opacity: 0.45; /* historical - last 30 days */ }
.memory-item.decay-3    { opacity: 0.28; /* historical - older */ }
.memory-item.pinned     { opacity: 1 !important; }
```

Decay bucket is server-computed; CSS is the only mapping. `pinned=true` facts opt out.

## Error handling

- **Search failure** (backend error): show the top error in-band in the results area, keep grouped headers visible with empty states ("Vector index unavailable — using FTS fallback" if embedding backend is unreachable).
- **Empty ward content**: per-tab empty state with a hint ("No procedures yet — the book-reader skill will create them as it runs").
- **Write failures**: inline error in the WriteRail near the button that triggered the write. Don't clear the form content.
- **Stale ward snapshot**: after a write, invalidate `useWardContent(id)` cache entry; refetch happens on next render. No optimistic insert for v1 (keeps truth in the backend).

## Testing

### Unit / component

- `SearchBar` — mode toggle state, quote detection, filter chip parsing (`ward:foo category:bar`)
- `ContentList` — correct grouping by `age_bucket`, CSS class for opacity, "pinned" override
- `MemoryItemCard` — renders kind badge for each category, match-source badge color mapping
- `WardRail` — active state, count badges, sort order (active wards first, global last)
- `WriteRail` — clicking `+ Fact` opens `AddDrawer`, successful save triggers refetch

### Integration (playwright / vitest + jsdom)

- Full flow: load tab → pick ward → switch tab → search → click result → modal shows full text
- Write flow: `+ Instruction` → fill form → save → appears in TODAY bucket
- Time-warp: drag slider to 30d → older items recover full opacity; newer items still on top

### Backend

- `search_memory_facts_hybrid` vs `_fts` produce different sort orders for a known-ambiguous query; hybrid surfaces semantic match above weak FTS match (regression test)
- New `/api/wards/:ward_id/content` returns age_bucket correctly for fixtures across all 4 buckets
- Wiki FTS table stays in sync with `ward_wiki_articles` after insert/update/delete (trigger test)

## Migration / rollout

1. Ship behind feature flag `memory_tab_command_deck` (default off, toggle in Settings → Advanced for dogfooding).
2. Land backend endpoints first; existing UI continues to work.
3. Land new Memory tab; gate via flag.
4. Internal dogfood for a week.
5. Flip flag on by default; move `MemoryTab.tsx` → `MemoryTabLegacy.tsx`.
6. Remove legacy after one more week, if no regressions reported.

## Performance

- `/api/wards/:ward_id/content` joins four tables but returns at most ~100 rows per type. Total payload <200KB typical. One network round-trip per ward switch; use `staleTime: 30s` on the hook so rapid tab-switching doesn't spam.
- `/api/memory/search` fans out per type in parallel on the server. p95 target <400ms for hybrid across all four types on a 10k-fact palace.
- Opacity decay is CSS-only — no JS tick loop.

## Accessibility

- Every `onClick` on a non-button element gets `role="button"`, `tabIndex={0}`, Enter/Space keydown handler (project rule `typescript-accessibility.md`).
- Ward rail is a `<nav>` with `<ul>`/`<li>` semantics; active ward gets `aria-current="true"`.
- Search input has a visible `<label>` (sr-only) pointing to the input id.
- Time-warp slider uses `<input type="range">` with `aria-valuemin/max/now` bound to age-bucket days.
- Color contrast ≥ 4.5:1 against `--color-background-surface` for all text; faded items (`.decay-*`) still maintain ≥ 3:1.

## Open questions

None outstanding after the brainstorming round. All direction choices locked:

- Layout: Command Deck + Temporal Fade ✓
- Ward lens scope: facts + wiki + procedures + episodes ✓
- Write surface: persistent right rail (not floating action) ✓
- Search: hybrid default, mode toggle, inline filters, grouped by type, "why" badges visible ✓
- Style: piggy-back on existing tokens; no new design language ✓
