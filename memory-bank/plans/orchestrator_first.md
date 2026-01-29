# Orchestrator-First Architecture

## The Insight

**The orchestrator IS the workflow engine.** Users don't need to manually draw workflows - they give goals, and the AI orchestrator:
1. Interprets intent
2. Creates a plan (task graph)
3. Routes tasks to the right capabilities
4. Executes and coordinates
5. Refines if needed
6. Delivers results

This eliminates the need for a visual workflow IDE.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         ORCHESTRATOR AGENT                               │
│                                                                          │
│   User Goal ──► Interpret ──► Plan ──► Route ──► Execute ──► Deliver    │
│                                           │                              │
│                                           ▼                              │
│                              ┌────────────────────────┐                 │
│                              │  CAPABILITY REGISTRY   │                 │
│                              ├────────────────────────┤                 │
│                              │  Skills    │  Tools    │                 │
│                              │  MCPs      │  Agents   │                 │
│                              └────────────────────────┘                 │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Capability Registry

A unified registry of all capabilities the orchestrator can use:

```rust
/// Unified capability that the orchestrator can invoke
pub enum Capability {
    /// Built-in tools (file, search, web, etc.)
    Tool(Arc<dyn Tool>),

    /// Skills (loaded from SKILL.md files)
    Skill(SkillDefinition),

    /// MCP server tools (external integrations)
    Mcp { server_id: String, tool: McpTool },

    /// Sub-agents (specialized AI agents)
    Agent(AgentDefinition),
}

/// Capability metadata for routing decisions
pub struct CapabilityMeta {
    pub name: String,
    pub description: String,
    pub category: CapabilityCategory,
    pub tags: Vec<String>,
    pub risk_level: RiskLevel,
    pub cost: CostEstimate,  // Token/compute cost hint
}

pub enum CapabilityCategory {
    // Skills
    Analyze,
    Retrieve,
    Plan,
    Build,
    Validate,

    // Tools
    WebSearch,
    CodeRuntime,
    FileSystem,
    DataTransform,
    TestVerify,

    // MCPs
    BrowserAutomation,
    RepoNavigation,
    DocumentProcessing,
    MediaProcessing,
    CloudOperations,

    // Agents
    Researcher,
    Implementer,
    Tester,
    Writer,
    Reviewer,
}
```

---

## Orchestrator Core

```rust
/// The central orchestrator that routes tasks to capabilities
pub struct Orchestrator {
    /// All available capabilities
    registry: CapabilityRegistry,

    /// LLM for planning and routing
    llm: Arc<dyn Llm>,

    /// Execution context
    context: Arc<dyn InvocationContext>,

    /// Task execution history
    trace: ExecutionTrace,
}

impl Orchestrator {
    /// Main entry point - process a user goal
    pub async fn process(&self, goal: &str) -> Result<OrchestratorResult> {
        // 1. Interpret intent and constraints
        let intent = self.interpret(goal).await?;

        // 2. Create plan (task graph)
        let plan = self.plan(&intent).await?;

        // 3. Execute plan with routing and coordination
        let result = self.execute(plan).await?;

        // 4. Compose final answer
        self.compose(result).await
    }

    /// Create execution plan
    async fn plan(&self, intent: &Intent) -> Result<TaskGraph> {
        // LLM generates task breakdown with dependencies
        // Each task is tagged with capability requirements
    }

    /// Execute task graph
    async fn execute(&self, mut plan: TaskGraph) -> Result<ExecutionResult> {
        loop {
            // Get next executable tasks (dependencies met)
            let ready_tasks = plan.ready_tasks();

            if ready_tasks.is_empty() {
                break;
            }

            // Execute tasks (potentially in parallel)
            for task in ready_tasks {
                // Route to best capability
                let capability = self.route(&task).await?;

                // Execute
                let output = self.invoke(capability, &task).await?;

                // Quality check
                if !self.quality_check(&task, &output).await? {
                    // Refine and retry
                    plan.refine_task(&task, &output)?;
                    continue;
                }

                // Mark complete
                plan.complete_task(&task, output)?;
            }

            // Check if goal is met
            if plan.goal_met() {
                break;
            }
        }

        plan.into_result()
    }

    /// Route task to best capability
    async fn route(&self, task: &Task) -> Result<Capability> {
        // 1. Filter by category/tags
        let candidates = self.registry.filter(&task.requirements);

        // 2. Rank by relevance, cost, risk
        let ranked = self.rank_capabilities(candidates, task);

        // 3. Select best
        ranked.first().cloned().ok_or(Error::NoCapability)
    }
}
```

---

## Task Graph

```rust
/// A directed acyclic graph of tasks
pub struct TaskGraph {
    tasks: HashMap<TaskId, Task>,
    dependencies: HashMap<TaskId, Vec<TaskId>>,
    outputs: HashMap<TaskId, Value>,
    status: HashMap<TaskId, TaskStatus>,
}

pub struct Task {
    pub id: TaskId,
    pub description: String,
    pub requirements: TaskRequirements,
    pub inputs: Vec<TaskInput>,
    pub expected_output: OutputSpec,
}

pub struct TaskRequirements {
    /// Capability categories that can handle this task
    pub categories: Vec<CapabilityCategory>,
    /// Specific capability names (if known)
    pub preferred: Vec<String>,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Max execution time
    pub timeout: Option<Duration>,
}

pub enum TaskStatus {
    Pending,
    Ready,      // Dependencies met
    Running,
    Completed,
    Failed { error: String, retries: u32 },
    Skipped,
}
```

---

## Execution Trace

Full observability of what the orchestrator did:

```rust
pub struct ExecutionTrace {
    pub goal: String,
    pub intent: Intent,
    pub plan: TaskGraph,
    pub steps: Vec<ExecutionStep>,
    pub duration: Duration,
    pub token_usage: TokenUsage,
}

pub struct ExecutionStep {
    pub task_id: TaskId,
    pub capability: String,
    pub input: Value,
    pub output: Value,
    pub duration: Duration,
    pub status: StepStatus,
    pub quality_score: Option<f32>,
}
```

---

## What This Replaces

| Old (Workflow IDE) | New (Orchestrator) |
|--------------------|-------------------|
| Visual node editor | AI plans automatically |
| Manual connections | AI routes to capabilities |
| Static workflows | Dynamic task graphs |
| User designs flow | User states goal |
| XY Flow dependency | None |
| `.workflow-layout.json` | Not needed |
| Workflow execution engine | Orchestrator |

---

## Simplified Product

### Before (Complex)
```
User → Workflow IDE → Design Flow → Connect Nodes → Execute → Result
         ↓
    Learn XY Flow
    Understand agents
    Connect properly
    Debug visually
```

### After (Simple)
```
User → State Goal → Orchestrator Plans & Executes → Result
         ↓
    Just describe what you want
    AI figures out how
    See execution trace if curious
```

---

## UI Simplification

### Remove
- `src/features/workflow-ide/` - Visual flow builder
- XY Flow dependency
- Workflow layout files
- Node configuration panels

### Keep/Enhance
- `src/features/agent-channels/` - Chat interface (primary UI)
- Agent management (configure capabilities, not workflows)
- Execution trace viewer (see what orchestrator did)
- TODO panel (orchestrator creates tasks)

### New
- **Capability Browser** - See available skills, tools, MCPs, agents
- **Trace Viewer** - Visualize task graph execution (read-only)
- **Plan Preview** - Before execution, show planned approach

---

## Default Agent Becomes Orchestrator

Every agent is an orchestrator by default:

```yaml
# agents/assistant/config.yaml
name: assistant
display_name: Personal Assistant
description: General-purpose orchestrator

# Orchestrator settings (new)
orchestrator:
  planning_model: claude-sonnet  # For planning
  execution_model: claude-haiku  # For simple tasks
  max_plan_depth: 3              # Nested sub-plans
  parallel_execution: true       # Run independent tasks in parallel
  quality_threshold: 0.8         # Min quality score to accept

# Available capabilities (what this orchestrator can use)
capabilities:
  skills:
    - "*"  # All skills
  tools:
    - "*"  # All tools
  mcps:
    - "*"  # All enabled MCPs
  agents:
    - researcher
    - implementer
    - writer
```

---

## Skills as First-Class Capabilities

Skills aren't just "instructions to load" - they're capabilities with defined interfaces:

```markdown
---
name: code-review
description: Review code for quality, security, and best practices
category: Validate
tags: [code, review, quality]
inputs:
  - name: code
    type: string
    description: Code to review
  - name: language
    type: string
    description: Programming language
outputs:
  - name: issues
    type: array
    description: List of issues found
  - name: suggestions
    type: array
    description: Improvement suggestions
  - name: score
    type: number
    description: Quality score 0-100
---

# Code Review Skill

## Instructions
When reviewing code, analyze for:
1. Security vulnerabilities
2. Performance issues
3. Code style violations
4. Logic errors
...
```

---

## Sub-Agents as Capabilities

Sub-agents are pre-configured orchestrators for specific domains:

```yaml
# agents/researcher/config.yaml
name: researcher
display_name: Research Agent
description: Deep research on any topic
category: Researcher

orchestrator:
  planning_model: claude-sonnet
  execution_model: claude-haiku

capabilities:
  tools:
    - web_fetch
    - web_search
    - memory
    - grep
    - read
  mcps:
    - browser
  skills:
    - summarize
    - analyze
```

When the main orchestrator delegates to `researcher`, it invokes this agent as a sub-task.

---

## Implementation Plan

### Phase 1: Core Orchestrator
1. Define `Capability` enum and `CapabilityRegistry`
2. Implement `TaskGraph` with dependency tracking
3. Implement basic `Orchestrator` with plan/route/execute
4. Add `ExecutionTrace` for observability

### Phase 2: Capability Integration
1. Wrap existing tools as capabilities
2. Convert skills to capability format
3. Wrap MCP tools as capabilities
4. Define sub-agent capability interface

### Phase 3: Planning & Routing
1. Implement LLM-based planning
2. Implement capability routing with ranking
3. Add quality checking
4. Add plan refinement on failure

### Phase 4: UI Adaptation
1. Remove workflow IDE
2. Add capability browser
3. Add trace viewer
4. Enhance chat with plan preview

---

## Files to Create

```
crates/zero-core/src/
├── capability.rs      # Capability enum and registry
├── task.rs            # Task and TaskGraph
├── orchestrator.rs    # Orchestrator core
├── trace.rs           # ExecutionTrace

crates/zero-agent/src/
├── orchestrator_agent.rs  # OrchestratorAgent implementation

src/features/
├── capabilities/      # Capability browser UI
├── trace-viewer/      # Execution trace visualization
```

## Files to Remove

```
src/features/workflow-ide/           # Entire directory
src/shared/types/workflow.ts         # Workflow types
application/workflow-executor/       # Workflow execution
```

---

## Migration Path

1. **Keep both temporarily** - Workflow IDE still works
2. **Add orchestrator mode** - Flag to use orchestrator vs workflow
3. **Default to orchestrator** - New agents use orchestrator
4. **Deprecate workflow IDE** - Stop maintaining
5. **Remove** - Clean removal after validation

---

## Benefits

| Aspect | Workflow IDE | Orchestrator |
|--------|-------------|--------------|
| Learning curve | High | Low |
| Flexibility | Fixed flows | Dynamic |
| Maintenance | Complex | Simple |
| Code size | Large | Small |
| User effort | Design flows | State goals |
| AI leverage | Execution only | Planning + Execution |
| Error recovery | Manual redesign | Auto-refine |
