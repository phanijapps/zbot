# Agent Hierarchy Design — Goal-Oriented Orchestration

## Research Summary

Based on analysis of Manus AI, Devin, CrewAI, and production multi-agent systems:

### Key Principles (from Manus)
- **File system as memory** — externalize state to files (todo.md, plans), not in-context
- **One action per iteration** — agent must observe result before next action
- **Planner as roadmap** — numbered steps with status, injected into context
- **Context engineering > prompt engineering** — agent failures are context failures

### Key Principles (from Devin)
- **Plan → Implement chunk → Test → Fix → Checkpoint** — not one-shot
- **Dedicated planning mode** — explore code before modifying it
- **Playbooks** — reusable prompt templates for common patterns
- **80% time savings, not 100%** — human review remains critical

### Key Principles (from production systems)
- **Hierarchical orchestration** — orchestrator decomposes, specialists execute
- **Plan-execute separation** — expensive model plans, cheap models execute
- **3-5 agents sweet spot** — beyond this, coordination > execution cost
- **Shared scratchpad** — agents see each other's progress (our ward memory-bank)

---

## Agent Hierarchy

```
User
  │
  ▼
Root (Orchestrator)
  ├── planner-agent (Planner) — decomposes goals into structured plans
  ├── code-agent (Ward Coder) — implements code inside wards
  ├── data-analyst (Analyst) — interprets data, generates insights
  ├── research-agent (Researcher) — gathers external information
  └── writing-agent (Writer) — creates documents and reports
```

### Why Planner is a Separate Agent (not root)

Root's job: receive goal → delegate → review → synthesize. If root also plans, it:
1. Spends 14 tool calls reading ward structure, loading skills, writing specs
2. Produces low-quality decomposition (dumps everything on one agent)
3. Doesn't read core_docs.md or memory-bank (orchestrators don't do that)

Planner's job: read the ward, understand what exists, produce a structured plan with agent assignments. This takes 1 delegation (~30-60s) but saves minutes of bad orchestration.

---

## Agent Configurations

### Root (Orchestrator)
**Identity:** Goal-oriented orchestrator. Receives user requests, delegates to specialists, reviews results, synthesizes deliverables.

**When to use planner:** Intent analysis says `approach=graph`. Root delegates to planner first, gets structured plan back, then executes each step.

**When NOT to use planner:** `approach=simple`. Root handles directly or delegates one focused task.

### planner-agent (NEW)
**Identity:** Reads ward context, decomposes goals into structured execution plans with agent assignments.

**Input:** User goal + ward name
**Output:** Structured plan (JSON or markdown) with steps, agent assignments, dependencies, acceptance criteria

**Key behaviors:**
- Enters ward, reads AGENTS.md + memory-bank/ docs
- Checks core/ for reusable modules
- Produces plan that reuses existing code
- Assigns each step to the right agent
- Identifies what can run sequentially vs what needs previous step output

### code-agent (Ward Coder)
**Identity:** Ward-centric developer. Reads ward docs, reuses core/ modules, follows spec-driven development.

**Key behaviors:**
- First action: enter ward, read AGENTS.md + core_docs.md
- Reuses core/ before creating
- Extracts reusable functions to core/ after task
- Updates memory-bank/core_docs.md
- For large task lists: process 15, hand off remainder

### data-analyst (Analyst)
**Identity:** Interprets existing data outputs. Does NOT write data pipelines — that's code-agent's job.

**Key behaviors:**
- Reads data files produced by code-agent
- Generates insights, statistics, visualizations
- Produces structured analysis (JSON + markdown)
- Loads domain skills dynamically

### research-agent (Researcher)
**Identity:** Gathers external information via web search, news, analyst reports.

**Key behaviors:**
- Uses web search tools and browser
- Synthesizes findings into structured format
- Cites all sources

### writing-agent (Writer)
**Identity:** Creates polished documents, reports, HTML from structured data.

**Key behaviors:**
- Reads data/analysis files
- Produces formatted output (markdown, HTML)
- Professional tone, structured sections

---

## Flow: Complex Task (approach=graph)

```
1. User: "Comprehensive analysis of PTON with options recommendations"
2. Intent analysis: approach=graph, ward=financial-analysis, skills=[coding, yf-data, yf-signals]
3. Root: recall → title → ward → delegate to planner-agent
4. Planner reads ward, checks core/, produces:
   {
     "steps": [
       {"id": 1, "task": "Fetch PTON OHLCV + quote data", "agent": "code-agent", "depends_on": [], "output": "pton/data/*.csv"},
       {"id": 2, "task": "Calculate technical indicators", "agent": "code-agent", "depends_on": [1], "output": "pton/technicals.json", "note": "reuse core/indicators.py"},
       {"id": 3, "task": "Fetch and analyze options chain", "agent": "code-agent", "depends_on": [1], "output": "pton/options.json", "note": "reuse core/options.py"},
       {"id": 4, "task": "Gather catalysts and news", "agent": "research-agent", "depends_on": [], "output": "pton/catalysts.md"},
       {"id": 5, "task": "Synthesize analysis with recommendations", "agent": "data-analyst", "depends_on": [2,3,4], "output": "pton/analysis.json"},
       {"id": 6, "task": "Generate HTML report", "agent": "writing-agent", "depends_on": [5], "output": "pton/report.html"}
     ]
   }
5. Root executes plan step by step (or parallel where deps allow)
6. Root synthesizes final response to user
```

## Flow: Simple Task (approach=simple)

```
1. User: "What's the weather like?"
2. Intent analysis: approach=simple
3. Root: responds directly (no planner, no delegation)
```

---

## Intent Analysis Integration

When `approach == "graph"`, inject into root's context:

```
**First step:** Delegate to planner-agent:
  "Plan this goal: {primary_intent}. Ward: {ward_name}. Check core/ for reusable modules."
The planner will return a structured execution plan. Execute each step by delegating to the assigned agent.
```

This replaces the SDLC block. Root doesn't need to know HOW to plan — it just delegates planning.

---

## Ward Memory as Shared Scratchpad (Manus-inspired)

The ward's memory-bank/ serves as the shared scratchpad between agents:
- `ward.md` — domain knowledge, patterns from past sessions
- `structure.md` — directory layout, tech stack (auto-generated)
- `core_docs.md` — reusable module documentation (auto-generated)
- `plan.md` — current execution plan (written by planner, read by all agents)

Every agent reads memory-bank/ before acting. Every agent updates it after acting. This is the continuity mechanism across delegations.

---

## Todo/Plan as Attention Mechanism (Manus-inspired)

Manus uses `todo.md` to keep the agent focused — continuously reciting objectives into context. Our equivalent: the planner writes `plan.md` with step statuses. Each agent sees the full plan in its context and knows where it fits.

Root updates plan status after each delegation completes:
```
[x] Step 1: Fetch data (code-agent) — DONE
[x] Step 2: Technical indicators (code-agent) — DONE
[ ] Step 3: Options analysis (code-agent) — IN PROGRESS
[ ] Step 4: Catalysts (research-agent) — PENDING
```
