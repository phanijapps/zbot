# Parallel Fan-Out → Merge Pattern

## Overview

A workflow pattern where multiple subagents process in parallel, then their outputs merge into a single aggregator agent.

## Visual Representation

```
                    ┌─────────────────────────┐
                    │         START           │
                    └───────────┬─────────────┘
                                │
            ┌───────────────────┼───────────────────┐
            │                   │                   │
            ▼                   ▼                   ▼
   ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
   │   Subagent A    │ │   Subagent B    │ │   Subagent C    │
   └────────┬────────┘ └────────┬────────┘ └────────┬────────┘
            │                   │                   │
            └───────────────────┼───────────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │       Aggregator        │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │          END            │
                    └─────────────────────────┘
```

## Use Cases

- **Research tasks**: Multiple researchers gather data, one synthesizer combines findings
- **Multi-perspective analysis**: Get different viewpoints, then reconcile
- **Validation workflows**: Multiple validators check, aggregator makes final decision
- **Content generation**: Multiple drafters, one editor finalizes

## Example: Chef Bot

**Phase 1 - Parallel Processing (Fan-Out):**
- **Ingredient Substituter** - Analyzes possible substitutions
- **Instruction Formatter** - Prepares formatting guidelines
- **Inventory Checker** - Validates available ingredients

**Phase 2 - Aggregation (Merge):**
- **Recipe Finder** - Uses combined context to find and present recipes

## Layout JSON Reference

```json
{
  "positions": {
    "start-{id}": { "x": 330.0, "y": -15.0 },
    "subagent-a": { "x": 45.0, "y": 225.0 },
    "subagent-b": { "x": 420.0, "y": 225.0 },
    "subagent-c": { "x": 765.0, "y": 225.0 },
    "subagent-aggregator": { "x": 405.0, "y": 435.0 },
    "end-{id}": { "x": 510.0, "y": 585.0 }
  },
  "edges": [
    { "source": "start-{id}", "target": "subagent-a" },
    { "source": "start-{id}", "target": "subagent-b" },
    { "source": "start-{id}", "target": "subagent-c" },
    { "source": "subagent-a", "target": "subagent-aggregator" },
    { "source": "subagent-b", "target": "subagent-aggregator" },
    { "source": "subagent-c", "target": "subagent-aggregator" },
    { "source": "subagent-aggregator", "target": "end-{id}" }
  ]
}
```

## Characteristics

| Property | Value |
|----------|-------|
| Pattern Type | Scatter-Gather / Map-Reduce |
| Parallelism | High (Phase 1) |
| Sync Point | Aggregator node |
| Complexity | Medium |
