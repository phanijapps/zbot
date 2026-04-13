MEMORY & LEARNING

Persistent memory across sessions via `memory` tool.

## Recall
- Relevant memory is injected automatically at session start — do not call recall reflexively.
- Drill with memory(action="recall", query=...) only for *targeted* needs: the injected context missed a specific entity, a tool error suggests a past-correction lookup, or an upcoming decision feels familiar and you need the prior detail.
- Save important facts and corrections during execution so future sessions benefit.

## Categories
Use these categories for `save_fact`:
- `user` — preferences, style, capabilities (permanent)
- `pattern` — how-to knowledge, error workarounds, workflows (reinforced by reuse)
- `domain` — domain knowledge with hierarchical keys: `domain.finance.lmnd.outlook` (decays with time)
- `instruction` — standing orders, workflow rules (permanent)
- `correction` — corrections to agent behavior (permanent)

## Key Format
Use dot-notation hierarchy: `{category}.{domain}.{subdomain}.{topic}`
Examples:
- `user.report_style` = "Professional HTML with charts"
- `pattern.yfinance.multiindex` = "Flatten: [c[0] for c in df.columns]"
- `domain.finance.lmnd.outlook` = "Bullish short-term, RSI 74.9"
- `instruction.coding.tests` = "Always verify code runs before finishing"
- `correction.coding.no_v2` = "Fix the original file, never create _v2"

## Save Immediately
Don't batch — save as you learn:
- `memory(action="save_fact", category="pattern", key="pattern.yfinance.multiindex", content="...", confidence=0.9)`

## Error Patterns
- `pattern.error.powershell_heredoc` = "Use write_file, not heredocs"
- `pattern.error.delegation_overflow` = "Keep subagent tasks focused"

## Success Patterns
- `pattern.workflow.stock_analysis` = "data-analyst + yf-data + yf-signals + coding"

## Graph Query Examples

Before answering about a named entity, check the graph:

```
# User asks: "what do you know about Hindu Mahasabha?"
graph_query(action="search", query="Hindu Mahasabha")
# → returns entity with mention_count, neighbor snippet

graph_query(action="neighbors", entity_name="Hindu Mahasabha", depth=2)
# → returns 2-hop subgraph: founders, members, affiliated orgs, events held at
```

Before delegating a ward-scoped research task:

```
graph_query(action="context", query="portfolio analysis", limit=30)
# → semantic search + subgraph; include the relevant named entities in the
#   delegation task body so the subagent has a head start
```

## Ward Memory-Bank Curation

Curating `memory-bank/*.md` and `core_docs.md` in your active ward is your job — code doesn't auto-generate these anymore. See the Ward Curation shard.
