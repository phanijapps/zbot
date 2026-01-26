# Agent Workflow Architecture

This document provides a comprehensive understanding of the AgentZero crate architecture for implementing agent workflows.

## Table of Contents

1. [Crate Overview](#crate-overview)
2. [Core Abstractions (zero-core)](#core-abstractions-zero-core)
3. [Agent Types (zero-agent)](#agent-types-zero-agent)
4. [LLM Integration (zero-llm)](#llm-integration-zero-llm)
5. [MCP Integration (zero-mcp)](#mcp-integration-zero-mcp)
6. [Session Management (zero-session)](#session-management-zero-session)
7. [Middleware System (zero-middleware)](#middleware-system-zero-middleware)
8. [Application Layer (zero-app)](#application-layer-zero-app)
9. [Knowledge Graph](#knowledge-graph)
10. [Built-in Tools (agent-tools)](#built-in-tools-agent-tools)
11. [Workflow Execution Flow](#workflow-execution-flow)
12. [Implementing Custom Workflows](#implementing-custom-workflows)

---

## Crate Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         zero-app                                     │
│              (Application Layer - Integration Point)                 │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
        ▼                       ▼                       ▼
┌───────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  zero-agent   │     │   zero-session  │     │ zero-middleware │
│ (Agent Types) │     │   (Sessions)    │     │  (Processing)   │
└───────┬───────┘     └────────┬────────┘     └─────────────────┘
        │                      │
        ▼                      ▼
┌───────────────┐     ┌─────────────────┐
│   zero-llm    │     │    zero-mcp     │
│ (LLM Clients) │     │ (MCP Protocol)  │
└───────┬───────┘     └────────┬────────┘
        │                      │
        └──────────┬───────────┘
                   ▼
           ┌─────────────┐
           │  zero-core  │
           │ (Core Traits│
           │  & Types)   │
           └─────────────┘

Application Crates:
┌─────────────────┐     ┌─────────────────┐
│  agent-tools    │     │ knowledge-graph │
│ (Built-in Tools)│     │ (Memory Layer)  │
└─────────────────┘     └─────────────────┘
```

---

## Core Abstractions (zero-core)

### Agent Trait

The central interface for all agents:

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn sub_agents(&self) -> &[Arc<dyn Agent>];
    async fn run(&self, ctx: Arc<dyn InvocationContext>) -> Result<EventStream>;
}
```

- **EventStream**: `Pin<Box<dyn Stream<Item = Result<Event>> + Send>>`
- Agents are composable via `sub_agents()`
- All execution is async and streaming

### Tool Trait

Interface for executable tools:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value>;  // JSON Schema
    fn response_schema(&self) -> Option<Value>;
    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value>;
}
```

### Context Hierarchy

```
ReadonlyContext (base - immutable execution info)
    │
    ├── invocation_id(), agent_name(), user_id()
    ├── app_name(), session_id(), branch()
    └── user_content()

    ▼
CallbackContext (adds state access)
    │
    ├── get_state(key) -> Option<Value>
    └── set_state(key, value)

    ▼
ToolContext (for tool execution)        InvocationContext (for agent execution)
    │                                       │
    ├── function_call_id()                  ├── agent(), session(), run_config()
    ├── actions(), set_actions()            ├── actions(), set_actions()
    └── inherits CallbackContext            ├── end_invocation(), ended()
                                            ├── add_content()
                                            └── inherits CallbackContext
```

### Event System

```rust
pub struct Event {
    pub id: String,                          // UUID
    pub timestamp: DateTime<Utc>,
    pub invocation_id: String,               // Groups events from one run
    pub branch: String,                      // Multi-path identifier
    pub author: String,                      // user, agent, tool, system
    pub content: Option<Content>,            // Message content
    pub actions: EventActions,               // Triggered actions
    pub turn_complete: bool,
    pub long_running_tool_ids: Vec<String>,
    pub metadata: HashMap<String, Value>,
}

pub struct EventActions {
    pub state_delta: HashMap<String, Value>,
    pub skip_summarization: bool,
    pub transfer_to_agent: Option<String>,   // Route to different agent
    pub escalate: bool,                      // Escalate to human
}
```

### Message Types

```rust
pub struct Content {
    pub role: String,        // user, assistant, system, tool
    pub parts: Vec<Part>,
}

pub enum Part {
    Text { text: String },
    FunctionCall { name: String, args: Value, id: Option<String> },
    FunctionResponse { id: String, response: String },
    Binary { mime_type: String, data: Vec<u8> },
}
```

### State Prefixes

```rust
pub const KEY_PREFIX_USER: &str = "user:";    // Persists across sessions
pub const KEY_PREFIX_APP: &str = "app:";      // Application-wide
pub const KEY_PREFIX_TEMP: &str = "temp:";    // Cleared each turn
```

---

## Agent Types (zero-agent)

### 1. LlmAgent (Leaf Agent)

Core LLM-based agent that responds using an LLM and tools.

```rust
pub struct LlmAgent {
    name: String,
    description: String,
    llm: Arc<dyn Llm>,
    tools: Arc<dyn Toolset>,
    system_instruction: Option<String>,
}
```

**Execution Loop:**
1. Build LLM request from conversation history
2. Send to LLM, get response
3. Extract tool calls from response
4. Execute tools sequentially
5. Add tool responses to history
6. Repeat until `turn_complete` or max iterations

### 2. SequentialAgent (Pipeline)

Execute sub-agents in order (A → B → C).

```rust
let pipeline = SequentialAgent::new("pipeline", vec![
    Arc::new(parse_agent),
    Arc::new(transform_agent),
    Arc::new(format_agent),
]);
```

**Use Cases:** ETL pipelines, data transformation, document analysis

### 3. ParallelAgent (Concurrent)

Execute sub-agents concurrently.

```rust
let team = ParallelAgent::new("team", vec![
    Arc::new(research_agent),
    Arc::new(analysis_agent),
    Arc::new(planning_agent),
]);
```

**Use Cases:** Information gathering, brainstorming, parallel processing

### 4. LoopAgent (Iterative)

Execute repeatedly until condition or max iterations.

```rust
let retry = LoopAgent::new("retry", vec![Arc::new(worker)])
    .with_max_iterations(3);
```

**Exit Conditions:**
- `max_iterations` reached
- `event.actions.escalate` set to true

**Use Cases:** Retry logic, polling, state machines

### 5. ConditionalAgent (Rule-Based Routing)

Route based on synchronous condition.

```rust
let router = ConditionalAgent::new(
    "router",
    |ctx| ctx.get_state("is_premium").and_then(|v| v.as_bool()).unwrap_or(false),
    Arc::new(premium_handler),
).with_else(Arc::new(basic_handler));
```

**Use Cases:** Feature flags, permission checking, A/B testing

### 6. LlmConditionalAgent (Intelligent Routing)

Use LLM to classify and route to appropriate agent.

```rust
let classifier = LlmConditionalAgent::builder("classifier", llm)
    .instruction("Classify as 'technical', 'general', or 'creative'.")
    .route("technical", tech_agent)
    .route("general", general_agent)
    .route("creative", creative_agent)
    .default_route(general_agent)
    .build()?;
```

**Use Cases:** Intent detection, skill routing, intelligent dispatch

### 7. CustomAgent (User-Defined)

Arbitrary async logic without LLM.

```rust
let custom = CustomAgent::builder("processor")
    .handler(|ctx| {
        Box::pin(async move {
            // Custom async logic
            let s = stream! { yield Ok(Event::new("id").with_content(...)) };
            Ok(Box::pin(s))
        })
    })
    .build()?;
```

**Use Cases:** System integration, external APIs, complex business logic

### Composition Patterns

```
Sequential:  Input → A → B → C → Output

Parallel:    Input → ┬→ A ──┐
                     ├→ B ──┼→ Output (interleaved)
                     └→ C ──┘

Conditional: Input → [Condition?] ─yes→ A → Output
                                  ─no→  B → Output

Loop:        Input → [Agent] → [Continue?] ─yes→ [Agent]
                                          ─no→  Output

Nested:      SequentialAgent:
               ├─ Agent1
               ├─ ParallelAgent: [Agent2a, Agent2b]
               └─ ConditionalAgent: if→Agent3a, else→Agent3b
```

---

## LLM Integration (zero-llm)

### Llm Trait

```rust
#[async_trait]
pub trait Llm: Send + Sync {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse>;
    async fn generate_stream(&self, request: LlmRequest) -> Result<LlmResponseStream>;
}
```

### LlmConfig

```rust
pub struct LlmConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,      // For OpenAI-compatible APIs
    pub organization_id: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

// Standard OpenAI
let config = LlmConfig::new("sk-...", "gpt-4o-mini");

// OpenAI-compatible (DeepSeek, Groq, etc.)
let config = LlmConfig::compatible("api-key", "https://api.deepseek.com", "deepseek-chat");
```

### Request/Response

```rust
pub struct LlmRequest {
    pub contents: Vec<Content>,
    pub system_instruction: Option<String>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

pub struct LlmResponse {
    pub content: Option<Content>,
    pub turn_complete: bool,
    pub usage: Option<TokenUsage>,
}
```

---

## MCP Integration (zero-mcp)

### Server Configuration

```rust
// Stdio-based (subprocess)
let server = McpServerConfig::stdio("claude", "Claude", "/path/to/server")
    .with_args(vec!["--arg1".into()])
    .with_env("API_KEY", "value");

// HTTP-based
let server = McpServerConfig::http("api", "API Server", "http://localhost:3000")
    .with_header("Authorization", "Bearer token");
```

### Tool Exposure

MCP tools are wrapped as Zero tools via `McpToolset`:

```rust
let toolset = McpToolsetBuilder::new()
    .with_server_id("server-id")
    .with_client(client)
    .with_filter(tool_filter)
    .build()
    .await?;

// Toolset implements Toolset trait
let tools = toolset.tools().await?;
```

### Tool Filtering

```rust
// By name
let filter = ToolFilter::with_name("exact_name");
let filter = ToolFilter::with_name_prefix("tool_");
let filter = ToolFilter::with_name_contains("search");

// By property
let filter = ToolFilter::with_property("category", "file");

// Combinations
let filter = filter1.or(filter2);
```

---

## Session Management (zero-session)

### InMemorySession

```rust
pub struct InMemorySession {
    id: String,
    app_name: String,
    user_id: String,
    state: InMemoryState,
    history: Vec<Content>,
}

let session = InMemorySession::new("session-1", "my-app", "user-123");
```

### Session Operations

```rust
// History management
session.add_content(Content::user("Hello"));
session.add_contents(vec![...]);
let history = session.conversation_history();
session.clear_history();

// State access
session.state().get("user:preference");
session.state_mut().set("temp:data".into(), json!(...));
```

### State Scoping

| Prefix | Scope | Persistence |
|--------|-------|-------------|
| `user:` | User preferences | Across sessions |
| `app:` | Application config | Application-wide |
| `temp:` | Ephemeral data | Single turn only |

---

## Middleware System (zero-middleware)

### Middleware Traits

```rust
// Pre-process messages before LLM
#[async_trait]
pub trait PreProcessMiddleware: Send + Sync {
    fn name(&self) -> &'static str;
    fn enabled(&self) -> bool;
    async fn process(
        &self,
        messages: Vec<MiddlewareMessage>,
        context: &MiddlewareContext,
    ) -> Result<MiddlewareEffect, String>;
}

// React to events during execution
#[async_trait]
pub trait EventMiddleware: Send + Sync {
    fn name(&self) -> &'static str;
    fn enabled(&self) -> bool;
    async fn on_event(
        &self,
        event: &MiddlewareEvent,
        context: &MiddlewareContext,
    ) -> Result<(), String>;
}
```

### Built-in Middleware

**SummarizationMiddleware:**
- Compresses long conversations
- Configurable triggers (tokens, messages, context fraction)
- Preserves recent messages while summarizing old ones

**ContextEditingMiddleware:**
- Clears old tool results
- Configurable keep policy
- Reduces context size without losing recent information

### Pipeline Execution

```rust
let pipeline = MiddlewarePipeline::new()
    .add_pre_processor(Box::new(summarization))
    .add_pre_processor(Box::new(context_editing))
    .add_event_handler(Box::new(logger));

// Pre-process phase
let processed = pipeline.process_messages(messages, &context, |event| {
    // Handle emitted events
}).await?;

// Event phase
pipeline.handle_event(&event, &context).await?;
```

---

## Application Layer (zero-app)

### ZeroApp Builder

```rust
let app = ZeroAppBuilder::new()
    .with_llm_config(LlmConfig::new("sk-...", "gpt-4o-mini"))
    .with_mcp_server(McpServerConfig::stdio(...))
    .with_middleware_config(MiddlewareConfig::default())
    .build()?;
```

### Creating Components

```rust
// Session
let session = app.create_session("session-1", "my-app", "user-123");

// Tool registry
let tools = app.create_tool_registry(session_id)?;
```

---

## Knowledge Graph

### Data Model

**Entities:**
- Types: Person, Organization, Location, Concept, Tool, Project, Custom
- Properties: HashMap<String, JSON> for flexible metadata
- Temporal tracking: first_seen_at, last_seen_at, mention_count

**Relationships:**
- Types: WorksFor, LocatedIn, RelatedTo, Created, Uses, PartOf, Mentions, Custom
- Bidirectional with source/target entity references

### Storage (SQLite)

```sql
-- Entities
CREATE TABLE kg_entities (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    name TEXT NOT NULL,
    properties TEXT,
    first_seen_at TEXT,
    last_seen_at TEXT,
    mention_count INTEGER DEFAULT 1
);

-- Relationships
CREATE TABLE kg_relationships (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    source_entity_id TEXT REFERENCES kg_entities(id),
    target_entity_id TEXT REFERENCES kg_entities(id),
    relationship_type TEXT NOT NULL,
    properties TEXT,
    ...
);
```

### Agent Tools

| Tool | Purpose |
|------|---------|
| `list_entities` | List all entities (optional type filter) |
| `search_entities` | Search by name (partial match) |
| `get_relationships` | Get connections for an entity |
| `add_entity` | Create new entity |
| `add_relationship` | Create relationship between entities |

---

## Built-in Tools (agent-tools)

| Category | Tool | Description |
|----------|------|-------------|
| **File** | Read | Read file contents |
| | Write | Write/create files |
| | Edit | Search/replace in files |
| **Search** | Grep | Regex search in files |
| | Glob | Find files by pattern |
| **Execution** | Python | Execute Python code |
| | Shell | Execute shell commands (bash/zsh/PowerShell) |
| | Load Skill | Load and execute skills |
| **UI** | Request Input | Ask user for input |
| | Show Content | Display content to user |
| **Knowledge** | List Entities | List knowledge graph entities |
| | Search Entities | Search entities |
| | Get Relationships | Get entity relationships |
| | Add Entity | Add to knowledge graph |
| | Add Relationship | Create relationships |
| **Agent** | Create Agent | Create new agents |

---

## Workflow Execution Flow

```
1. APPLICATION SETUP
   ├─ Create ZeroApp with LLM config, MCP servers, middleware
   └─ Initialize tool registry

2. SESSION CREATION
   ├─ Create session with unique ID
   └─ Session holds: conversation_history, state

3. AGENT CONSTRUCTION
   ├─ Build agent (LlmAgent, SequentialAgent, etc.)
   ├─ Inject LLM instance
   └─ Inject toolset

4. INVOCATION
   ├─ Create InvocationContext with session
   ├─ Call agent.run(ctx)
   └─ Agent returns EventStream

5. EXECUTION LOOP (LlmAgent)
   FOR EACH ITERATION (max 50):
   │
   ├─ Build LLM request from history
   ├─ Send to LLM
   ├─ EMIT assistant response event
   │
   ├─ IF tool calls:
   │   ├─ Execute each tool
   │   ├─ EMIT tool result events
   │   └─ Add to history
   │
   └─ IF turn_complete: EXIT

6. EVENT STREAMING
   ├─ Client receives events as async stream
   └─ Events: AgentResponse, ToolCall, ToolResult, StateUpdate

7. MIDDLEWARE (if configured)
   ├─ Pre-process: Modify messages before LLM
   └─ Post-event: Handle events after emission
```

---

## Implementing Custom Workflows

### Example: Research Pipeline

```rust
// Stage 1: Query analyzer
let analyzer = LlmAgent::new("analyzer", "Analyzes research queries", llm.clone(), tools.clone())
    .with_system_instruction("Extract key topics and questions from the query.");

// Stage 2: Parallel researchers
let researcher1 = LlmAgent::new("web_researcher", "Searches web", llm.clone(), web_tools);
let researcher2 = LlmAgent::new("doc_researcher", "Searches documents", llm.clone(), doc_tools);

let researchers = ParallelAgent::new("researchers", vec![
    Arc::new(researcher1),
    Arc::new(researcher2),
]);

// Stage 3: Synthesizer
let synthesizer = LlmAgent::new("synthesizer", "Synthesizes findings", llm.clone(), tools)
    .with_system_instruction("Combine research findings into coherent answer.");

// Complete pipeline
let pipeline = SequentialAgent::new("research_pipeline", vec![
    Arc::new(analyzer),
    Arc::new(researchers),
    Arc::new(synthesizer),
]);
```

### Example: Customer Support Router

```rust
let router = LlmConditionalAgent::builder("support_router", llm.clone())
    .instruction("Classify customer query: 'billing', 'technical', 'general'")
    .route("billing", Arc::new(billing_agent))
    .route("technical", Arc::new(technical_agent))
    .route("general", Arc::new(general_agent))
    .default_route(Arc::new(general_agent))
    .build()?;
```

### Example: Approval Loop

```rust
let draft_agent = LlmAgent::new("drafter", "Creates drafts", llm.clone(), tools);

let approval_loop = LoopAgent::new("approval", vec![Arc::new(draft_agent)])
    .with_max_iterations(3);

// Agent sets escalate=true when draft is approved
```

---

## Key Takeaways

1. **Composability**: Agents can contain sub-agents of any type, enabling complex workflows
2. **Streaming**: All execution is event-based and streaming-first
3. **State Management**: Hierarchical state with clear scoping (user/app/temp)
4. **Tool Integration**: Unified tool interface for built-in, MCP, and custom tools
5. **Middleware**: Cross-cutting concerns handled via pipeline
6. **Knowledge Graph**: Persistent memory layer for entity and relationship tracking
7. **Multi-tenancy**: Agent-scoped data isolation throughout the system

---

*This document serves as a comprehensive reference for implementing agent workflows in AgentZero.*
