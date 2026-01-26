# Conditional Pipeline Pattern

## Overview

A workflow pattern where the flow branches based on conditions, with some steps being optional depending on the output of a decision node.

## Visual Representation

```
                    ┌─────────────────────────┐
                    │         START           │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │     Decision Node       │
                    └───────────┬─────────────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
          [Condition A]                   [Condition B]
                │                               │
                ▼                               │
   ┌─────────────────────┐                      │
   │   Optional Step     │                      │
   └───────────┬─────────┘                      │
               │                                │
               └────────────┬───────────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │     Merge Point         │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │          END            │
                └─────────────────────────┘
```

## Use Cases

- **Validation with fallback**: Check data, process differently if invalid
- **Feature detection**: Detect capabilities, skip unsupported steps
- **Error handling**: Normal path vs error recovery path
- **Optimization**: Skip expensive operations when not needed

## Example: Chef Bot

**Step 1 - Input:**
- START triggers the workflow

**Step 2 - Decision:**
- **Inventory Checker** - Validates ingredients and determines if substitutions are needed

**Step 3 - Conditional Branch:**
- Path A: "Substitution needed" → **Ingredient Substituter** → **Instruction Formatter**
- Path B: "No substitution needed" → **Instruction Formatter** (skip substituter)

**Step 4 - Continuation:**
- **Instruction Formatter** → **Recipe Finder** → END

## Layout JSON Reference

```json
{
  "positions": {
    "start-{id}": { "x": 540.0, "y": -60.0 },
    "subagent-decision": { "x": 135.0, "y": 60.0 },
    "subagent-optional": { "x": 30.0, "y": 240.0 },
    "subagent-merge": { "x": 555.0, "y": 420.0 },
    "subagent-final": { "x": 420.0, "y": 585.0 },
    "end-{id}": { "x": 495.0, "y": 765.0 }
  },
  "edges": [
    { "source": "start-{id}", "target": "subagent-decision" },
    { "source": "subagent-decision", "target": "subagent-optional", "label": "Condition A" },
    { "source": "subagent-decision", "target": "subagent-merge", "label": "Condition B" },
    { "source": "subagent-optional", "target": "subagent-merge" },
    { "source": "subagent-merge", "target": "subagent-final" },
    { "source": "subagent-final", "target": "end-{id}" }
  ]
}
```

## Future: Conditional Gateway Node

This pattern currently relies on edge labels to indicate conditions. A dedicated **Conditional Gateway** node type would provide:

- Explicit condition evaluation
- Multiple output ports with named conditions
- Runtime branching based on previous node output
- Visual distinction (diamond shape in BPMN)

## Characteristics

| Property | Value |
|----------|-------|
| Pattern Type | Conditional / Branching |
| Parallelism | None (sequential with branches) |
| Decision Point | Decision node with labeled edges |
| Complexity | Medium |
| Skip Support | Yes (optional steps) |
