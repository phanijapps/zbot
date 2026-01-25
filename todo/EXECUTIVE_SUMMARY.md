# AgentZero Visual Workflow IDE - Executive Summary

## TL;DR

**Framework Choice**: XY Flow (React Flow v12+)

**Key Insight**: Your working orchestrator pattern uses `.subagents/` folders. The IDE should manage this structure directly, NOT maintain a separate `flow.json`.

**Recommendation**: Tear down current visual workflow builder and rebuild with proper integration.

---

## The Problem

```
CURRENT STATE (Broken)
┌─────────────────────────────────────┐
│  Visual IDE     │  Rust Backend    │
│  ┌───────────┐  │  ┌───────────┐   │
│  │ flow.json │  │  │.subagents/│   │
│  └───────────┘  │  └───────────┘   │
│       ↓         │        ↓         │
│   NOT USED      │    EXECUTED      │
└─────────────────────────────────────┘
The IDE output is ignored by execution!
```

---

## The Solution

```
TARGET STATE (Integrated)
┌─────────────────────────────────────┐
│  Visual IDE                        │
│  ┌───────────────────────────────┐ │
│  │      XY Flow Canvas           │ │
│  │   [Orchestrator] ─► [Sub-A]   │ │
│  │                  └► [Sub-B]   │ │
│  └───────────────────────────────┘ │
│            │  SAVE                 │
│            ▼                       │
│  ┌───────────────────────────────┐ │
│  │ ~/.config/zeroagent/agents/   │ │
│  │   my-orchestrator/            │ │
│  │   ├── config.yaml             │ │
│  │   ├── AGENTS.md ← Generated   │ │
│  │   └── .subagents/             │ │
│  │       ├── sub-a/              │ │
│  │       └── sub-b/              │ │
│  └───────────────────────────────┘ │
│            │  EXECUTE              │
│            ▼                       │
│  ┌───────────────────────────────┐ │
│  │   Rust Backend (unchanged)    │ │
│  │   execute_agent_stream()      │ │
│  └───────────────────────────────┘ │
└─────────────────────────────────────┘
Visual graph ↔ folder structure = single source of truth
```

---

## Why XY Flow?

| Feature | XY Flow | Custom Canvas |
|---------|---------|---------------|
| Custom Nodes | ✅ React components | 🔧 Build from scratch |
| Connection Validation | ✅ Built-in | 🔧 Manual |
| Minimap | ✅ Included | 🔧 Manual |
| Pan/Zoom | ✅ Optimized | 🔧 Manual |
| TypeScript | ✅ Native | ✅ |
| Community | ✅ 20k+ stars | ❌ None |
| Maintenance | ✅ Active | 🔧 You |

---

## Implementation Timeline

| Week | Deliverable |
|------|-------------|
| 1-2 | Basic canvas with Orchestrator + Subagent nodes |
| 3-4 | Properties panel, node configuration |
| 5-6 | Tauri commands for save/load folder structure |
| 7-8 | Execution visualization |
| 9-10 | Polish, templates, documentation |

---

## Files to Create

```
src/features/workflow-ide/
├── WorkflowIDEPage.tsx
├── types/workflow.ts
├── stores/workflowStore.ts
├── components/
│   ├── WorkflowEditor.tsx
│   ├── WorkflowToolbar.tsx
│   ├── nodes/
│   │   ├── OrchestratorNode.tsx
│   │   ├── SubagentNode.tsx
│   │   └── index.ts
│   └── panels/
│       ├── NodePalette.tsx
│       └── PropertiesPanel.tsx
└── hooks/
    └── useTauriWorkflow.ts
```

---

## Tauri Commands Needed

```rust
// src-tauri/src/commands/workflow.rs

#[tauri::command]
pub async fn get_orchestrator_structure(agent_id: String) -> Result<WorkflowGraph, String>
// Reads .subagents/ folder and returns visual graph

#[tauri::command]  
pub async fn save_orchestrator_structure(agent_id: String, graph: WorkflowGraph) -> Result<(), String>
// Writes .subagents/ folders from visual graph
// Generates AGENTS.md with delegation instructions

#[tauri::command]
pub async fn validate_workflow(graph: WorkflowGraph) -> Result<ValidationResult, String>
// Checks for cycles, missing configs, etc.
```

---

## Key Design Decisions

### 1. No flow.json
The `.subagents/` folder structure IS the workflow definition.

### 2. Generate AGENTS.md
Visual connections → orchestrator instructions:
```markdown
## Your Team
- recipe-finder: Finds matching recipes
- format-output: Formats cooking instructions

## Workflow
1. Call recipe-finder with ingredients
2. Call format-output with results
```

### 3. LLM-Driven Execution
The visual graph shows **potential** paths.
The LLM decides **actual** execution at runtime.
This is different from traditional workflow engines.

### 4. Execution Visualization
Map `StreamEvent::ToolCallStart/ToolResult` to visual nodes.
Show which node is currently running.

---

## Quick Start Commands

```bash
# 1. Install XY Flow
npm install @xyflow/react

# 2. Create directory structure
mkdir -p src/features/workflow-ide/{components/nodes,components/panels,stores,hooks,types}

# 3. Copy starter files from STARTER_IMPLEMENTATION.md

# 4. Add route to App.tsx
# <Route path="/workflow/:agentId" element={<WorkflowIDEPage />} />

# 5. Run and test
npm run tauri dev
# Navigate to /workflow/test-agent
```

---

## Success Criteria (v1.0)

- [ ] XY Flow canvas with pan/zoom
- [ ] Orchestrator and Subagent nodes
- [ ] Drag from palette to canvas
- [ ] Connect nodes with edges
- [ ] Properties panel for configuration
- [ ] Save generates `.subagents/` structure
- [ ] Load reads from `.subagents/` structure
- [ ] Generated `AGENTS.md` with instructions

---

## Documents Provided

| File | Purpose |
|------|---------|
| `IMPLEMENTATION_PLAN.md` | Full analysis and 10-week roadmap |
| `STARTER_IMPLEMENTATION.md` | Copy-paste code to bootstrap |
| `EXECUTIVE_SUMMARY.md` | This file - quick reference |

---

## Questions for Phani

1. Do you want to keep the old IDE during transition, or replace immediately?
2. Should we support "external agent" references (agents outside `.subagents/`)?
3. What's the priority: more node types or execution visualization first?
4. Any specific templates you want (Pipeline, Swarm, Router)?
