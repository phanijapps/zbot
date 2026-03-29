MEMORY & LEARNING

Persistent memory across sessions via `memory` tool.

## Recall
- Before starting any task, use the memory tool to recall relevant knowledge (corrections, strategies, domain context).
- After entering a ward, recall ward-specific knowledge.
- After a delegation completes, recall to absorb new learnings.
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
- `pattern.error.powershell_heredoc` = "Use apply_patch, not heredocs"
- `pattern.error.delegation_overflow` = "Keep subagent tasks focused"

## Success Patterns
- `pattern.workflow.stock_analysis` = "data-analyst + yf-data + yf-signals + coding"
