# AgentZero Visual Workflow IDE - Implementation Plan

## Executive Summary

After analyzing the AgentZero codebase, I recommend **rebuilding the Visual Workflow IDE using XY Flow (React Flow v12+)** with a focus on properly integrating with the existing **working orchestrator + subagent pattern** in Rust.

The current implementation stores `flow.json` but is disconnected from how agent orchestration actually works. The new IDE should be the visual interface for managing the `.subagents/` folder structure that the Rust backend already supports.

---

## Part 1: Current State Analysis

### What's Working (Keep These)

```
✅ Zero Framework Crates (crates/zero-*)
   - Core traits: Agent, Tool, Session, Event
   - LLM clients: OpenAI-compatible
   - MCP integration: stdio, HTTP, SSE
   - Middleware: summarization, context editing

✅ Application Layer (application/)
   - agent-runtime: YAML config, executor, MCP managers
   - agent-tools: File ops, search, Python, Knowledge Graph
   - daily-sessions, knowledge-graph, search-index

✅ Orchestrator + Subagent Pattern
   - .subagents/ folder auto-discovery
   - SubagentTool with bidirectional isolation
   - LLM-driven delegation with context/task/goal

✅ Agent Execution
   - execute_agent_stream Tauri command
   - Tool calling loop with max iterations
   - Streaming events to frontend
```

### What Needs Rebuilding (Current IDE Problems)

```
❌ Visual Workflow Builder (src/features/agents/*)
   Problem: Stores flow.json but doesn't map to .subagents/ structure
   Problem: Node types (Trigger, Parallel, etc.) don't match execution model
   Problem: No real integration with working orchestrator pattern
   Problem: Custom canvas implementation - limited features

❌ flow.json Approach
   Problem: Separate from config.yaml - two sources of truth
   Problem: Doesn't generate .subagents/ structure
   Problem: Visual state disconnected from execution state
```

### The Gap

```
┌─────────────────────────────────────────────────────────────────┐
│                    CURRENT STATE (Disconnected)                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Visual IDE                         Rust Backend               │
│   ┌──────────────┐                  ┌──────────────┐           │
│   │  flow.json   │    ────✖────    │ .subagents/  │           │
│   │  (separate)  │                  │ (working)    │           │
│   └──────────────┘                  └──────────────┘           │
│                                                                 │
│   The IDE creates flow.json but the backend reads .subagents/  │
│   These are NOT connected - IDE output is ignored by execution │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    TARGET STATE (Integrated)                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Visual IDE                         Rust Backend               │
│   ┌──────────────┐                  ┌──────────────┐           │
│   │  XY Flow     │    ────────────▶ │ .subagents/  │           │
│   │  Canvas      │    generates     │ config.yaml  │           │
│   │              │◀──────────────   │ AGENTS.md    │           │
│   └──────────────┘    reads         └──────────────┘           │
│                                                                 │
│   IDE directly manages the folder structure backend uses       │
│   Single source of truth - visual = execution                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 2: Framework Recommendation

### Recommended: XY Flow (React Flow v12+)

```
┌─────────────────────────────────────────────────────────────────┐
│                      WHY XY FLOW?                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ✅ Mature & Battle-Tested                                     │
│     - Used by Stripe, Typeform, Notion, many AI companies      │
│     - 20k+ GitHub stars, active development                    │
│                                                                 │
│  ✅ Perfect Feature Set                                        │
│     - Custom nodes with React components                       │
│     - Connection validation (typed ports)                      │
│     - Subflows for grouping                                    │
│     - Minimap, controls, background grid                       │
│     - Copy/paste, undo/redo support                            │
│                                                                 │
│  ✅ TypeScript Native                                          │
│     - Full type safety                                         │
│     - Matches your existing stack                              │
│                                                                 │
│  ✅ Performance                                                │
│     - Virtualized rendering                                    │
│     - Handles 1000+ nodes                                      │
│                                                                 │
│  ✅ Extensibility                                              │
│     - Plugin system                                            │
│     - Custom edges, handles, controls                          │
│                                                                 │
│  ✅ Integration Ready                                          │
│     - Works with Zustand (your state management)               │
│     - Works with Radix UI (your component library)             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Alternative Considered: Rete.js

```
Pros: Visual programming focus, node editor toolkit
Cons: Smaller community, less documentation, steeper learning curve
Decision: XY Flow is better for this use case
```

---

## Part 3: New Architecture Design

### Node Type Mapping (Visual → Execution)

```
┌─────────────────────────────────────────────────────────────────┐
│                    NODE TYPE TAXONOMY                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  AGENT NODES (map to .subagents/ or main agent)                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ OrchestratorNode                                         │   │
│  │   - Main agent that coordinates                          │   │
│  │   - Has config.yaml, AGENTS.md                          │   │
│  │   - Output: Delegates to subagents                       │   │
│  │                                                           │   │
│  │ SubagentNode                                              │   │
│  │   - Creates .subagents/{name}/ folder                    │   │
│  │   - Has own config.yaml, AGENTS.md                       │   │
│  │   - Input: context, task, goal                           │   │
│  │   - Output: result text                                   │   │
│  │                                                           │   │
│  │ ExternalAgentNode                                         │   │
│  │   - Reference to existing agent (not subagent)           │   │
│  │   - Can call any agent in ~/.config/zeroagent/agents/    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  TOOL NODES (map to agent-tools or MCP)                        │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ BuiltinToolNode                                          │   │
│  │   - read, write, edit, grep, glob, python                │   │
│  │   - Enabled/disabled per agent                           │   │
│  │                                                           │   │
│  │ MCPServerNode                                             │   │
│  │   - Reference to configured MCP server                   │   │
│  │   - Shows available tools from server                    │   │
│  │   - Can filter which tools are exposed                   │   │
│  │                                                           │   │
│  │ SkillNode                                                 │   │
│  │   - Reference to skill in ~/.config/zeroagent/skills/   │   │
│  │   - Loads skill instructions on demand                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  CONTROL NODES (map to AGENTS.md instructions)                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ RouterNode (Visual only - generates instructions)        │   │
│  │   - Conditions defined visually                          │   │
│  │   - Generates: "If X, call subagent-A, else call B"     │   │
│  │                                                           │   │
│  │ ParallelNode (Visual only - generates instructions)      │   │
│  │   - Fan-out to multiple subagents                        │   │
│  │   - Generates: "Call A, B, C in parallel, merge results"│   │
│  │                                                           │   │
│  │ LoopNode (Visual only - generates instructions)          │   │
│  │   - Iteration with exit condition                        │   │
│  │   - Generates: "Repeat until condition met"              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  I/O NODES                                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ InputNode                                                 │   │
│  │   - Workflow entry point                                  │   │
│  │   - Defines expected input schema                         │   │
│  │                                                           │   │
│  │ OutputNode                                                │   │
│  │   - Workflow exit point                                   │   │
│  │   - Defines output format                                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Key Insight: LLM-Driven vs Graph-Driven

```
┌─────────────────────────────────────────────────────────────────┐
│                EXECUTION MODEL REALITY                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Your current system is LLM-DRIVEN, not graph-driven:          │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                                                           │   │
│  │   User Message                                            │   │
│  │        │                                                  │   │
│  │        ▼                                                  │   │
│  │   ┌─────────────┐                                        │   │
│  │   │ Orchestrator │ ◀── LLM decides which subagent        │   │
│  │   │    Agent     │                                        │   │
│  │   └──────┬──────┘                                        │   │
│  │          │                                                │   │
│  │    LLM DECIDES (not graph engine)                        │   │
│  │          │                                                │   │
│  │    ┌─────┴─────┬─────────┬─────────┐                    │   │
│  │    ▼           ▼         ▼         ▼                    │   │
│  │ ┌─────┐   ┌─────┐   ┌─────┐   ┌─────┐                  │   │
│  │ │Sub-A│   │Sub-B│   │Sub-C│   │Sub-D│                  │   │
│  │ └─────┘   └─────┘   └─────┘   └─────┘                  │   │
│  │                                                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  The visual graph shows POTENTIAL paths                        │
│  The LLM decides ACTUAL execution at runtime                   │
│                                                                 │
│  This is DIFFERENT from traditional workflow engines           │
│  (like Airflow, Prefect, n8n) where the graph IS execution    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### What the IDE Should Actually Do

```
┌─────────────────────────────────────────────────────────────────┐
│              IDE RESPONSIBILITIES                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. STRUCTURE MANAGEMENT                                       │
│     ┌─────────────────────────────────────────────────────┐   │
│     │ Create/edit .subagents/ folder structure            │   │
│     │ Generate config.yaml for each subagent              │   │
│     │ Generate AGENTS.md with proper instructions         │   │
│     └─────────────────────────────────────────────────────┘   │
│                                                                 │
│  2. INSTRUCTION GENERATION                                     │
│     ┌─────────────────────────────────────────────────────┐   │
│     │ Visual connections → AGENTS.md delegation rules     │   │
│     │ "When user asks about X, delegate to SubagentA"    │   │
│     │ "For parallel work, call A, B, C simultaneously"    │   │
│     └─────────────────────────────────────────────────────┘   │
│                                                                 │
│  3. TOOL/MCP ASSIGNMENT                                        │
│     ┌─────────────────────────────────────────────────────┐   │
│     │ Drag MCP servers to agents                          │   │
│     │ Enable/disable built-in tools                       │   │
│     │ Assign skills to agents                             │   │
│     └─────────────────────────────────────────────────────┘   │
│                                                                 │
│  4. EXECUTION VISUALIZATION                                    │
│     ┌─────────────────────────────────────────────────────┐   │
│     │ Show which node is currently executing              │   │
│     │ Display tool calls in real-time                     │   │
│     │ Show context flow between agents                    │   │
│     └─────────────────────────────────────────────────────┘   │
│                                                                 │
│  5. CONFIGURATION UI                                           │
│     ┌─────────────────────────────────────────────────────┐   │
│     │ Per-node property panels                            │   │
│     │ Provider/model selection                            │   │
│     │ Temperature, max tokens, etc.                       │   │
│     └─────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 4: Implementation Requirements

### Phase 1: Core Canvas (Week 1-2)

```
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 1: FOUNDATION                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1.1 Install & Setup XY Flow                                   │
│      npm install @xyflow/react                                 │
│                                                                 │
│  1.2 Create Base Components                                    │
│      src/features/workflow-ide/                                │
│      ├── WorkflowEditor.tsx        # Main editor               │
│      ├── WorkflowCanvas.tsx        # XY Flow wrapper           │
│      ├── WorkflowToolbar.tsx       # Actions bar               │
│      ├── NodePalette.tsx           # Draggable node list       │
│      ├── PropertiesPanel.tsx       # Node configuration        │
│      └── stores/                                                │
│          └── workflowStore.ts      # Zustand store             │
│                                                                 │
│  1.3 Define Core Node Types                                    │
│      nodes/                                                     │
│      ├── OrchestratorNode.tsx                                  │
│      ├── SubagentNode.tsx                                      │
│      └── index.ts                  # Node type registry        │
│                                                                 │
│  1.4 Basic Operations                                          │
│      - Drag from palette to canvas                             │
│      - Connect nodes                                           │
│      - Select/delete nodes                                     │
│      - Pan/zoom canvas                                         │
│                                                                 │
│  DELIVERABLE: Empty canvas with basic node manipulation        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 2: Node Implementation (Week 3-4)

```
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 2: NODES                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  2.1 Orchestrator Node                                         │
│      - Main agent configuration                                │
│      - Provider/model selection                                │
│      - System prompt editing                                   │
│      - Tool/MCP/Skill toggles                                  │
│                                                                 │
│  2.2 Subagent Node                                             │
│      - Name (becomes folder name)                              │
│      - Description                                             │
│      - Provider/model (can differ from orchestrator)           │
│      - Instructions editor (AGENTS.md content)                 │
│      - Input ports: context, task, goal                        │
│      - Output port: result                                     │
│                                                                 │
│  2.3 Tool Nodes                                                │
│      - Built-in tool node (read, write, etc.)                  │
│      - MCP server node (reference to configured server)        │
│      - Skill node (reference to skill)                         │
│                                                                 │
│  2.4 Control Nodes                                             │
│      - Router node (condition-based branching)                 │
│      - Parallel node (fan-out)                                 │
│      - Input/Output nodes                                      │
│                                                                 │
│  DELIVERABLE: All node types implemented with properties       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 3: Backend Integration (Week 5-6)

```
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 3: BACKEND                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  3.1 New Tauri Commands                                        │
│      src-tauri/src/commands/workflow.rs                        │
│      ├── get_orchestrator_structure(agent_id)                  │
│      │   → Returns visual graph from .subagents/ structure     │
│      │                                                          │
│      ├── save_orchestrator_structure(agent_id, graph)          │
│      │   → Generates .subagents/ folders from visual graph     │
│      │   → Creates config.yaml for each subagent               │
│      │   → Generates AGENTS.md with delegation instructions    │
│      │                                                          │
│      ├── validate_workflow(graph)                              │
│      │   → Checks for cycles, orphans, missing configs         │
│      │                                                          │
│      └── generate_agents_md(graph)                             │
│          → Creates orchestrator instructions from visual       │
│                                                                 │
│  3.2 Folder Structure Generation                               │
│      Visual Graph:                                              │
│      ┌──────────┐     ┌──────────┐     ┌──────────┐           │
│      │ Chef Bot │────▶│ Recipe   │────▶│ Format   │           │
│      │(orchestr)│     │ Finder   │     │ Output   │           │
│      └──────────┘     └──────────┘     └──────────┘           │
│                                                                 │
│      Generated Structure:                                       │
│      ~/.config/zeroagent/agents/chef-bot/                      │
│      ├── config.yaml                                           │
│      ├── AGENTS.md          ← Generated from graph             │
│      └── .subagents/                                           │
│          ├── recipe-finder/                                    │
│          │   ├── config.yaml                                   │
│          │   └── AGENTS.md                                     │
│          └── format-output/                                    │
│              ├── config.yaml                                   │
│              └── AGENTS.md                                     │
│                                                                 │
│  3.3 AGENTS.md Generation                                      │
│      From visual connections, generate:                        │
│      """                                                        │
│      You coordinate a team of specialized assistants:          │
│      - recipe-finder: Finds recipes matching ingredients       │
│      - format-output: Formats cooking instructions             │
│                                                                 │
│      Workflow:                                                  │
│      1. Call recipe-finder with ingredient context             │
│      2. Call format-output with recipe results                 │
│      """                                                        │
│                                                                 │
│  DELIVERABLE: Visual graph ↔ folder structure sync             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 4: Execution Integration (Week 7-8)

```
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 4: EXECUTION                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  4.1 Real-time Execution State                                 │
│      - Subscribe to streaming events from execute_agent_stream │
│      - Map tool_call events to visual nodes                    │
│      - Highlight currently executing node                      │
│      - Show tool results on edges                              │
│                                                                 │
│  4.2 Execution Visualization                                   │
│      ┌──────────┐     ┌──────────┐     ┌──────────┐           │
│      │ Chef Bot │────▶│ Recipe   │────▶│ Format   │           │
│      │  ✓ Done  │     │ 🔄 Running│    │ ○ Pending│           │
│      └──────────┘     └──────────┘     └──────────┘           │
│                           │                                     │
│                    ┌──────┴──────┐                             │
│                    │ Tool Result │                             │
│                    │ "Found 3    │                             │
│                    │  recipes"   │                             │
│                    └─────────────┘                             │
│                                                                 │
│  4.3 Debug Mode                                                │
│      - Step-through execution                                  │
│      - Inspect context/task/goal at each node                  │
│      - View raw LLM requests/responses                         │
│                                                                 │
│  DELIVERABLE: Visual execution feedback                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 5: Polish & Templates (Week 9-10)

```
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 5: POLISH                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  5.1 Workflow Templates                                        │
│      - Pipeline: A → B → C                                     │
│      - Swarm: Split → [A, B, C] → Merge                       │
│      - Router: Conditional branching                           │
│      - Loop: Iterative refinement                              │
│                                                                 │
│  5.2 UX Improvements                                           │
│      - Keyboard shortcuts (Ctrl+S, Delete, etc.)               │
│      - Undo/redo                                               │
│      - Copy/paste nodes                                        │
│      - Auto-layout                                             │
│      - Minimap navigation                                      │
│                                                                 │
│  5.3 Validation & Error Handling                               │
│      - Real-time validation messages                           │
│      - Missing configuration warnings                          │
│      - Connection type checking                                │
│                                                                 │
│  5.4 Documentation                                             │
│      - User guide                                              │
│      - Node type reference                                     │
│      - Tutorial: Building your first orchestrator              │
│                                                                 │
│  DELIVERABLE: Production-ready workflow IDE                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 5: File Structure

### New Directory Structure

```
src/features/workflow-ide/
├── index.ts                          # Public exports
├── WorkflowIDEPage.tsx               # Main page component
│
├── components/
│   ├── WorkflowEditor.tsx            # Main editor orchestrator
│   ├── WorkflowCanvas.tsx            # XY Flow wrapper
│   ├── WorkflowToolbar.tsx           # Save, Run, Validate buttons
│   ├── ExecutionOverlay.tsx          # Runtime execution state
│   │
│   ├── panels/
│   │   ├── NodePalette.tsx           # Draggable node list
│   │   ├── PropertiesPanel.tsx       # Selected node config
│   │   ├── ValidationPanel.tsx       # Errors/warnings
│   │   └── ExecutionConsole.tsx      # Logs and events
│   │
│   ├── nodes/                        # Custom XY Flow nodes
│   │   ├── BaseNode.tsx              # Shared node styling
│   │   ├── OrchestratorNode.tsx      # Main agent node
│   │   ├── SubagentNode.tsx          # Subagent node
│   │   ├── ToolNode.tsx              # Built-in tool
│   │   ├── MCPServerNode.tsx         # MCP server reference
│   │   ├── SkillNode.tsx             # Skill reference
│   │   ├── RouterNode.tsx            # Conditional branching
│   │   ├── ParallelNode.tsx          # Fan-out node
│   │   ├── InputNode.tsx             # Workflow input
│   │   ├── OutputNode.tsx            # Workflow output
│   │   └── index.ts                  # Node type registry
│   │
│   └── edges/                        # Custom edges
│       ├── DataEdge.tsx              # Data flow edge
│       ├── ControlEdge.tsx           # Control flow edge
│       └── index.ts
│
├── stores/
│   └── workflowStore.ts              # Zustand state management
│
├── hooks/
│   ├── useWorkflowState.ts           # Graph state management
│   ├── useWorkflowExecution.ts       # Execution state
│   ├── useTauriWorkflow.ts           # Tauri IPC
│   └── useAutoSave.ts                # Debounced saving
│
├── services/
│   └── workflowService.ts            # Tauri command wrappers
│
├── types/
│   └── workflow.ts                   # TypeScript interfaces
│
├── utils/
│   ├── graphToStructure.ts           # Visual → folder structure
│   ├── structureToGraph.ts           # Folder structure → visual
│   ├── generateAgentsMd.ts           # Generate AGENTS.md
│   └── validation.ts                 # Graph validation
│
└── templates/
    ├── pipeline.ts                   # Pipeline template
    ├── swarm.ts                      # Parallel swarm template
    ├── router.ts                     # Router template
    └── loop.ts                       # Loop template
```

### Backend Changes

```
src-tauri/src/
├── commands/
│   └── workflow.rs                   # NEW: Workflow management
│       ├── get_orchestrator_structure()
│       ├── save_orchestrator_structure()
│       ├── validate_workflow()
│       └── generate_agents_md()
│
└── domains/
    └── workflow_runtime/             # NEW: Workflow domain
        ├── mod.rs
        ├── structure_parser.rs       # Parse .subagents/ to graph
        ├── structure_generator.rs    # Generate .subagents/ from graph
        └── agents_md_generator.rs    # Generate AGENTS.md
```

---

## Part 6: Key Technical Decisions

### Decision 1: Single Source of Truth

```
DECISION: The .subagents/ folder structure IS the workflow definition

- NO separate flow.json file
- Visual graph is derived from folder structure
- Saving writes directly to folders
- Loading reads from folders

RATIONALE:
- Existing Rust backend already works with .subagents/
- No sync issues between visual and execution
- Human-readable/editable files
- Git-friendly (folders, not binary)
```

### Decision 2: AGENTS.md Generation

```
DECISION: Generate orchestrator AGENTS.md from visual connections

The visual graph:
  ┌─────────┐     ┌─────────┐
  │ Analyze │────▶│ Report  │
  └─────────┘     └─────────┘

Generates AGENTS.md:
  """
  ## Your Team
  - analyze: Analyzes the input data
  - report: Generates reports from analysis

  ## Workflow Instructions
  When processing requests:
  1. First delegate to 'analyze' with the user's data
  2. Take the analysis results and delegate to 'report'
  3. Present the final report to the user
  """

RATIONALE:
- LLM-driven execution needs instructions, not graph traversal
- Instructions are flexible (LLM can adapt)
- Human-readable and editable
```

### Decision 3: Execution Visualization

```
DECISION: Map streaming events to visual nodes

StreamEvent::ToolCallStart { tool_name: "recipe-finder", ... }
  → Highlight "recipe-finder" SubagentNode
  → Show "Running..." badge

StreamEvent::ToolResult { tool_id, result, ... }
  → Show result preview on edge
  → Mark node as "Complete"

RATIONALE:
- Real-time feedback during execution
- Debug complex workflows
- Understand LLM decision-making
```

### Decision 4: Node Types Simplification

```
DECISION: Focus on 4 core node types initially

1. OrchestratorNode - The main coordinating agent
2. SubagentNode     - Child agents that do specific work
3. ToolNode         - Built-in tools, MCPs, Skills (grouped)
4. InputNode        - Workflow entry point

RATIONALE:
- Current implementation uses orchestrator + subagents
- Control flow (Router, Parallel) can be v2 features
- Keep initial scope manageable
- Match what the Rust backend actually supports
```

---

## Part 7: Migration Plan

### Step 1: Keep Current IDE Working

```
DO NOT delete src/features/agents/ immediately

1. Create src/features/workflow-ide/ as NEW feature
2. Add route: /workflow/:agentId
3. Keep existing IDE at /agents/:agentId
4. Test new IDE in parallel
```

### Step 2: Gradual Feature Parity

```
Week 1-2: Basic canvas with nodes
Week 3-4: Node properties editing
Week 5-6: Folder structure generation
Week 7-8: Execution visualization
Week 9-10: Migration of remaining features
```

### Step 3: Switch Default

```
Once feature parity achieved:
1. Update "Open IDE" button to route to /workflow/:agentId
2. Keep old IDE accessible via Settings > Legacy IDE
3. After 2 releases, deprecate old IDE
4. After 4 releases, remove old IDE
```

---

## Part 8: Success Criteria

### Must Have (v1.0)

```
□ XY Flow canvas with pan/zoom
□ OrchestratorNode and SubagentNode
□ Drag from palette to canvas
□ Connect nodes with edges
□ Properties panel for selected node
□ Save generates .subagents/ folder structure
□ Load reads from .subagents/ folder structure
□ Generated AGENTS.md with delegation instructions
□ Basic execution state visualization
```

### Should Have (v1.1)

```
□ ToolNode (MCPs, Skills, Built-ins grouped)
□ RouterNode for conditional branching
□ Workflow templates (Pipeline, Swarm)
□ Undo/redo
□ Copy/paste
□ Keyboard shortcuts
□ Validation panel with errors/warnings
```

### Nice to Have (v2.0)

```
□ ParallelNode for explicit fan-out
□ LoopNode for iteration
□ Debug mode with step-through
□ Auto-layout algorithm
□ Export/import workflows
□ Workflow versioning
```

---

## Part 9: Getting Started (For Claude Code)

### Immediate Next Steps

```bash
# 1. Install XY Flow
cd /path/to/agentzero
npm install @xyflow/react

# 2. Create feature directory
mkdir -p src/features/workflow-ide/components/nodes
mkdir -p src/features/workflow-ide/stores
mkdir -p src/features/workflow-ide/hooks
mkdir -p src/features/workflow-ide/types

# 3. Start with minimal implementation
# Create WorkflowEditor.tsx with basic XY Flow setup
# Create OrchestratorNode.tsx and SubagentNode.tsx
# Create workflowStore.ts with Zustand

# 4. Add route in App.tsx
# <Route path="/workflow/:agentId" element={<WorkflowIDEPage />} />

# 5. Test with existing chef-bot orchestrator
```

### Files to Create First

```
1. src/features/workflow-ide/types/workflow.ts
   - Define TypeScript interfaces

2. src/features/workflow-ide/stores/workflowStore.ts
   - Zustand store for graph state

3. src/features/workflow-ide/components/WorkflowEditor.tsx
   - Main editor component

4. src/features/workflow-ide/components/nodes/SubagentNode.tsx
   - First custom node

5. src/features/workflow-ide/WorkflowIDEPage.tsx
   - Page wrapper with layout
```

---

## Summary

**Recommendation**: Rebuild the Visual Workflow IDE using XY Flow, with the key insight that it should manage the `.subagents/` folder structure directly rather than maintaining a separate `flow.json` file.

**Key Principles**:
1. Single source of truth (folder structure)
2. LLM-driven execution (generate instructions, not graph traversal)
3. Real-time execution visualization
4. Gradual migration (don't break existing functionality)

**Timeline**: 10 weeks to production-ready IDE

**Risk Mitigation**: Keep old IDE working during development, migrate features incrementally.
