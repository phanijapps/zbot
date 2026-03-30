# Intent Analysis System

## Overview

Intent analysis is an **autonomous pre-execution middleware** that runs before the root agent's first LLM call. It indexes all available resources (skills, agents, wards) into `memory_facts` with local embeddings (fastembed), performs semantic search to find the most relevant resources for the user's request, sends only the top-N to a single LLM call, and injects the result as a `## Intent Analysis` section into the system prompt. Agents never call it; it happens automatically. Root agent only — subagents and continuations skip it.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Autonomous Intent Analysis Flow                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   User Message ──────────────────────────────────────────────────────────►  │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    Step 1: Index Resources                           │   │
│   │  (idempotent upsert into memory_facts via save_fact)               │   │
│   │                                                                       │   │
│   │   Skills → category:"skill", key:"skill:{name}"                    │   │
│   │   Agents → category:"agent", key:"agent:{id}"                      │   │
│   │   Wards  → category:"ward",  key:"ward:{name}" (reads AGENTS.md)  │   │
│   └────────────────────────────────┬────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    Step 2: Semantic Search                           │   │
│   │                                                                       │   │
│   │   recall_facts("root", user_message, 50)                            │   │
│   │   Filter by MIN_RELEVANCE_SCORE (0.15)                              │   │
│   │   Cap: 8 skills, 5 agents, 5 wards                                 │   │
│   └────────────────────────────────┬────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                    Step 3: LLM Analysis                              │   │
│   │                                                                       │   │
│   │   Input: user message + top-N skills + top-N agents + top-N wards  │   │
│   │   Output: IntentAnalysis {                                           │   │
│   │     primary_intent, hidden_intents (actionable instructions),        │   │
│   │     recommended_skills, recommended_agents,                          │   │
│   │     ward_recommendation { action, ward_name, subdirectory,          │   │
│   │                           structure, reason },                       │   │
│   │     execution_strategy { approach, graph, explanation },             │   │
│   │     rewritten_prompt                                                  │   │
│   │   }                                                                   │   │
│   └────────────────────────────────┬────────────────────────────────────┘   │
│                                    │                                         │
│                          (parse failed? skip enrichment)                     │
│                                    │                                         │
│                                    ▼                                         │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │              Inject into System Prompt                                │   │
│   │                                                                       │   │
│   │   Appends "## Intent Analysis" section to the base system prompt:   │   │
│   │   - Primary Intent                                                    │   │
│   │   - Hidden Intents (as numbered actionable instructions)             │   │
│   │   - Skills mapped to graph nodes (or simple list)                    │   │
│   │   - Recommended Agents                                                │   │
│   │   - Ward (name, action, subdirectory, directory layout)              │   │
│   │   - Execution Graph (mermaid, orchestration note, max cycles)        │   │
│   │   - Rewritten Request                                                 │   │
│   └────────────────────────────────┬────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼                                         │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │              Executor Starts with Enriched Prompt                     │   │
│   │                                                                       │   │
│   │   Root agent sees the full context from turn one — no tool call      │   │
│   │   needed, no conditional dispatch, LLM decides how to proceed        │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Key Design Principles

### 1. Pre-Execution Middleware, Not a Tool
- Runs as middleware before the root agent executor is constructed
- Agents never call it — enrichment is transparent and automatic
- Root-agent only: subagents and continuations do not trigger re-analysis

### 2. Autonomous Resource Discovery
- Indexes skills, agents, and wards into `memory_facts` with local embeddings (fastembed)
- Uses semantic search (`recall_facts`) to find relevant resources for the user message
- Only sends top-N results to the LLM (not the full catalog)
- Score threshold (0.15) and per-category caps filter noise

### 3. LLM Decides, No Conditional Logic
- The system prompt injection presents recommendations without branching code
- No `if recommended_skills then load_skill` wiring in the runner
- The agent reads the `## Intent Analysis` section and uses its own judgment

### 4. Recommend, Don't Inject
- The enrichment adds guidance (skill names, agent names, ward, graph shape), not loaded content
- Skills are not auto-loaded; agents are not auto-delegated
- The agent retains full autonomy to follow, modify, or override recommendations

### 5. Execution Strategy
LLM determines strategy via the `approach` field:
- **simple**: Direct execution, no graph needed (greetings, quick questions)
- **tracked**: Use `update_plan` for progress tracking on medium-complexity tasks
- **graph**: Use `execution_graph` for parallel/sequential orchestration with conditional edges

### 6. Ward Recommendation
LLM recommends a domain-level ward (not task-specific):
- **action**: `use_existing` or `create_new`
- **ward_name**: Domain-level reusable name (e.g., `financial-analysis`, `math-tutor`)
- **subdirectory**: Task-specific subdir (e.g., `stocks/spy`)
- **structure**: Directory layout map (`core/`, `output/`, task subdirs)
- After completing work, agents update AGENTS.md with what was built

## Semantic Search Configuration

| Parameter | Value | Purpose |
|-----------|-------|---------|
| `MIN_RELEVANCE_SCORE` | 0.15 | Minimum score to include a result |
| `MAX_SKILLS` | 8 | Maximum skills sent to LLM |
| `MAX_AGENTS` | 5 | Maximum agents sent to LLM |
| `MAX_WARDS` | 5 | Maximum wards sent to LLM |
| Fetch limit | 50 | Initial `recall_facts` limit before filtering |

## Response Schema

```typescript
interface IntentAnalysis {
  primary_intent: string;
  // Actionable instructions, not labels
  hidden_intents: string[];
  recommended_skills: string[];   // names from the available skills list
  recommended_agents: string[];   // names from the available agents list
  ward_recommendation: WardRecommendation;
  execution_strategy: ExecutionStrategy;
  rewritten_prompt: string;       // user message with all implicit intent made explicit
}

interface WardRecommendation {
  action: "use_existing" | "create_new";
  ward_name: string;              // domain-level reusable name
  subdirectory: string | null;    // task-specific subdir
  structure: Record<string, string>;  // directory path → purpose
  reason: string;
}

interface ExecutionStrategy {
  approach: "simple" | "tracked" | "graph";
  // Only present when approach === "graph"
  graph?: {
    nodes: Array<{
      id: string;
      task: string;
      agent: string;         // agent name or "root"
      skills: string[];      // "coding" required for any file-writing node
    }>;
    edges: Array<
      | { from: string; to: string }                                  // Direct
      | { from: string; conditions: Array<{ when: string; to: string }> }  // Conditional
    >;
    mermaid: string;         // Mermaid diagram string for the graph
    max_cycles?: number;     // Default 2 when omitted
  };
  explanation: string;       // Why this orchestration shape
}
```

## Injected System Prompt Section

`inject_intent_context` appends the following markdown to the base system prompt:

```markdown
## Intent Analysis

**Primary Intent**: {primary_intent}

**Hidden Intents** (address ALL of these):
1. {hidden_intent_1}
2. {hidden_intent_2}

**Skills** (load ONLY when the step requires it, unload after):
- Node A (task description): load `coding`, `data-analysis`

**Recommended Agents** (delegate to these):
- {agent_name}

**Ward**: `{ward_name}` ({action}) — {reason}
**Task directory**: `{subdirectory}`
**Directory layout** (create these, put files in the right place):
- `core/` — Shared reusable modules
- `output/` — Reports, charts, deliverables
**After completing work**: Update AGENTS.md with what was built and what's reusable.

**Execution Graph**:
```mermaid
{mermaid diagram}
```

**Orchestration**: {explanation}

**Max cycles**: {max_cycles}

**Rewritten request**: {rewritten_prompt}
```

## File Locations

| Component | Path |
|-----------|------|
| Middleware Module | `gateway/gateway-execution/src/middleware/intent_analysis.rs` |
| Runner Integration | `gateway/gateway-execution/src/runner.rs` |
| System Prompt Assembly | `gateway/gateway-templates/src/lib.rs` |

## Evolution History

### v1 (Initial): Heuristic Keyword Matching
- Simple keyword matching against skill descriptions
- No hidden intent detection
- Manual skill loading

### v2: LLM-Enhanced with Auto-Loading
- Added LLM analysis
- Auto-loaded skills via `auto_load` parameter
- Injected skill content into context

### v3: Pure LLM Analysis (Tool-Based)
- LLM is primary analyzer
- Removed `auto_load` parameter
- Returns recommendations only
- Agent explicitly calls `load_skill`
- Implementation: `runtime/agent-tools/src/tools/intent.rs`
- Issue: tool was never registered; agents were told to call it but couldn't

### v4: Pre-Execution Enrichment
- Moved out of the tool registry entirely
- Runs from the runner before root agent executor construction
- Injects `## Intent Analysis` section into system prompt
- No conditional branching in runner code — LLM reads the section and decides
- Hidden intents are actionable instructions, not labels
- Execution strategy includes graph/conditional edges for complex orchestration

### v5 (Current): Autonomous Middleware with Semantic Search
- Fully autonomous: indexes resources into `memory_facts`, searches semantically, calls LLM
- Uses local embeddings (fastembed) for resource indexing — no external API needed
- Score threshold (0.15) and per-category caps (8 skills, 5 agents, 5 wards) filter noise
- Only top-N relevant resources sent to LLM (not full catalog)
- Ward recommendation with directory structure for domain-level workspace organization
- `coding` skill required for any graph node that creates/modifies files
- Moved to `gateway/gateway-execution/src/middleware/intent_analysis.rs`
