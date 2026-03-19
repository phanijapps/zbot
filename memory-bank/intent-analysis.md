# Intent Analysis System

## Overview

The `analyze_intent` tool is a **pure analysis** tool that discovers hidden intents and recommends resources (skills, agents, wards). It does NOT auto-load or inject content — the calling agent decides whether to act on recommendations.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         analyze_intent Flow                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   User Message ─────────────────────────────────────────────────────────────►│
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐    │
│   │                    Discovery Phase                                   │    │
│   │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │    │
│   │  │   Skills     │  │   Agents     │  │    Wards     │               │    │
│   │  │  (cached)    │  │  (cached)    │  │  (disk scan) │               │    │
│   │  └──────────────┘  └──────────────┘  └──────────────┘               │    │
│   └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│   ┌─────────────────────────────────────────────────────────────────────┐    │
│   │                    LLM Analysis (PRIMARY)                            │    │
│   │                                                                      │    │
│   │   Input: message + ALL skills + ALL agents                          │    │
│   │   Output: LlmIntentAnalysis {                                       │    │
│   │     primary_intent, hidden_intents,                                 │    │
│   │     recommended_skills, recommended_agents,                         │    │
│   │     suggested_ward, execution_strategy,                             │    │
│   │     use_execution_graph, rewritten_prompt                           │    │
│   │   }                                                                  │    │
│   └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                     (LLM failed? Fallback)                                   │
│                              │                                               │
│                              ▼                                               │
│   ┌─────────────────────────────────────────────────────────────────────┐    │
│   │                    Heuristic Analysis (FALLBACK)                     │    │
│   │                                                                      │    │
│   │   Keyword matching + semantic search (if fact_store available)      │    │
│   └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│   ┌─────────────────────────────────────────────────────────────────────┐    │
│   │                    Response (Pure Analysis)                          │    │
│   │                                                                      │    │
│   │   Returns:                                                           │    │
│   │   - primary_intent / hidden_intents                                 │    │
│   │   - discovered_resources (skills, agents, wards)                    │    │
│   │   - ward_recommendation                                             │    │
│   │   - execution_plan (strategy, steps, required_first_action)         │    │
│   │   - NO AUTO-LOADING — agent must call load_skill explicitly         │    │
│   └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Key Design Principles

### 1. LLM is Primary
- When `llm_client` is available, LLM analysis is ALWAYS used
- LLM receives ALL available resources, not pre-filtered subset
- LLM identifies hidden intents that keyword matching would miss
- Fallback to heuristics only if LLM fails or is unavailable

### 2. Pure Analysis (No Side Effects)
- `analyze_intent` returns recommendations only
- Does NOT auto-load skills
- Does NOT inject content into context
- Agent explicitly calls `load_skill` when needed

### 3. Resource Discovery
- Skills: Cached from `index_resources`, auto-index if stale
- Agents: Cached from `index_resources`, auto-index if stale
- Wards: Direct disk scan (fast enough for small counts)

### 4. Execution Strategy
LLM determines strategy:
- **Simple**: Direct tool execution
- **Medium**: Use `update_plan` for progress tracking
- **High**: Use `execution_graph` for parallel/sequential orchestration

## LLM Prompt Structure

```markdown
## System Prompt (INTENT_ANALYSIS_PROMPT)
You are an intent analyzer. Given a user request and available resources,
identify hidden intents and recommend the best execution strategy.

## User Prompt
### User Request
{message}

### ALL Available Skills
- skill_name: description
- skill_name [CONFIGURED]: description  (already in agent's context)

### ALL Available Agents
- agent_id: description

## Response Format (JSON)
{
  "primary_intent": "...",
  "hidden_intents": ["..."],
  "recommended_skills": ["skill1", "skill2"],
  "recommended_agents": ["agent1"],
  "suggested_ward": "generic-ward-name",
  "execution_strategy": "simple|medium|high",
  "use_execution_graph": true|false,
  ...
}
```

## Response Schema

```typescript
interface AnalyzeIntentResponse {
  primary_intent: string;
  hidden_intents: string[];
  domain: string;
  explicit_goals: string[];
  implicit_goals: string[];
  rewritten_prompt: string;

  discovered_resources: {
    skills: Skill[];
    agents: Agent[];
    wards: Ward[];
  };

  ward_recommendation: {
    action: "create_generic" | "use_scratch" | "use_existing";
    ward_name: string;
    reason: string;
  };

  execution_plan: {
    strategy: "simple" | "medium" | "high";
    use_execution_graph: boolean;
    required_first_action?: string;  // "delegate" | "load_skill" | "create_ward"
    execution_steps: ExecutionStep[];
    complexity: string;
  };

  analysis_source: "llm" | "heuristic";
}
```

## Usage in System Prompt

From `instructions_starter.md`:

```markdown
## AUTONOMOUS BEHAVIOR PROTOCOL

1. **Discover & Auto-Load Resources**: Call `analyze_intent(message, auto_load=true)`
   - This discovers relevant skills, agents, and wards
   - **Automatically loads** high-relevance skills into your context
   - Returns execution strategy recommendation
   - Review the loaded skill instructions that appear in the result

2. **MANDATORY: Follow analyze_intent Recommendations**:
   - If `analyze_intent` recommends agents: **YOU MUST delegate to them**
   - If `analyze_intent` recommends skills: **YOU MUST use them**
   - If `analyze_intent` says use execution_graph: **YOU MUST use it**
```

## File Locations

| Component | Path |
|-----------|------|
| Tool Implementation | `runtime/agent-tools/src/tools/intent.rs` |
| Skill Indexer | `runtime/agent-tools/src/tools/indexer/skill.rs` |
| Agent Indexer | `runtime/agent-tools/src/tools/indexer/agent.rs` |
| System Prompt | `gateway/templates/instructions_starter.md` |
| Executor Integration | `gateway/gateway-execution/src/invoke/executor.rs` |

## Evolution History

### v1 (Initial): Heuristic Keyword Matching
- Simple keyword matching against skill descriptions
- No hidden intent detection
- Manual skill loading

### v2: LLM-Enhanced with Auto-Loading
- Added LLM analysis
- Auto-loaded skills via `auto_load` parameter
- Injected skill content into context

### v3 (Current): Pure LLM Analysis
- LLM is primary analyzer
- Removed `auto_load` parameter
- Returns recommendations only
- Agent explicitly calls `load_skill`
- Cleaner separation of concerns
