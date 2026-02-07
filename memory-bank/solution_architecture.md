# AgentZero Solution Architecture (C4 Model)

## Level 1: System Context

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              EXTERNAL ACTORS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────────┐         │
│   │   User   │    │ Cron Job │    │  Webhook │    │ External API │         │
│   │  (Human) │    │(Scheduled)│   │ (Plugin) │    │   (Client)   │         │
│   └────┬─────┘    └────┬─────┘    └────┬─────┘    └──────┬───────┘         │
│        │               │               │                 │                  │
└────────┼───────────────┼───────────────┼─────────────────┼──────────────────┘
         │               │               │                 │
         └───────────────┴───────┬───────┴─────────────────┘
                                 │
                                 ▼
         ┌───────────────────────────────────────────────────┐
         │                                                   │
         │              AGENTZERO PLATFORM                   │
         │                                                   │
         │   AI agent orchestration with multi-turn          │
         │   conversations, tool execution, and              │
         │   subagent delegation                             │
         │                                                   │
         └───────────────────────┬───────────────────────────┘
                                 │
         ┌───────────────────────┼───────────────────────────┐
         │                       │                           │
         ▼                       ▼                           ▼
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────────────┐
│   LLM Providers │   │   MCP Servers   │   │   File System           │
│                 │   │                 │   │                         │
│  - OpenAI       │   │  - stdio        │   │  ~/Documents/agentzero/ │
│  - Anthropic    │   │  - HTTP/SSE     │   │  - agents/              │
│  - Local models │   │  - Custom tools │   │  - wards/ (code wards)  │
│                 │   │                 │   │  - skills/              │
└─────────────────┘   └─────────────────┘   └─────────────────────────┘
```

### System Context Description

| Actor | Description | Interaction |
|-------|-------------|-------------|
| **User** | Human interacting via Web Dashboard or CLI | HTTP/WebSocket on ports 18791/18790 |
| **Cron Job** | Scheduled tasks triggering agent execution | Internal trigger via CronHook |
| **Webhook** | External systems pushing events | HTTP POST to /api/webhooks |
| **External API** | Programmatic access to agent platform | REST API on port 18791 |
| **LLM Providers** | AI model inference services | HTTPS API calls |
| **MCP Servers** | Model Context Protocol tool servers | stdio/HTTP/SSE protocols |
| **File System** | Persistent configuration and data | Local filesystem access |

---

## Level 2: Container Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            AGENTZERO PLATFORM                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                         CLIENT APPLICATIONS                            │  │
│  │                                                                        │  │
│  │   ┌─────────────────────┐          ┌─────────────────────┐            │  │
│  │   │    Web Dashboard    │          │        CLI          │            │  │
│  │   │   (React + Vite)    │          │      (zero)         │            │  │
│  │   │                     │          │                     │            │  │
│  │   │   localhost:3000    │          │   Terminal app      │            │  │
│  │   │   (dev) or :18791   │          │   with TUI          │            │  │
│  │   └──────────┬──────────┘          └──────────┬──────────┘            │  │
│  │              │                                │                        │  │
│  └──────────────┼────────────────────────────────┼────────────────────────┘  │
│                 │                                │                           │
│                 └────────────────┬───────────────┘                           │
│                                  │                                           │
│                    HTTP :18791   │   WebSocket :18790                        │
│                                  │                                           │
│  ┌───────────────────────────────┴───────────────────────────────────────┐  │
│  │                           DAEMON (zerod)                               │  │
│  │                                                                        │  │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
│  │  │                         GATEWAY                                  │  │  │
│  │  │                                                                  │  │  │
│  │  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │  │  │
│  │  │   │   HTTP API   │  │  WebSocket   │  │   Static     │          │  │  │
│  │  │   │    (Axum)    │  │   Handler    │  │   Files      │          │  │  │
│  │  │   │              │  │              │  │   (Tower)    │          │  │  │
│  │  │   └──────┬───────┘  └──────┬───────┘  └──────────────┘          │  │  │
│  │  │          │                 │                                     │  │  │
│  │  │          └────────┬────────┘                                     │  │  │
│  │  │                   │                                              │  │  │
│  │  │          ┌────────┴────────┐                                     │  │  │
│  │  │          │    Event Bus    │  ◄─── Broadcast to subscribers      │  │  │
│  │  │          └────────┬────────┘                                     │  │  │
│  │  └───────────────────┼──────────────────────────────────────────────┘  │  │
│  │                      │                                                  │  │
│  │  ┌───────────────────┴──────────────────────────────────────────────┐  │  │
│  │  │                      RUNTIME LAYER                                │  │  │
│  │  │                                                                   │  │  │
│  │  │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │  │  │
│  │  │   │  Execution  │  │    Agent    │  │    Tool     │              │  │  │
│  │  │   │   Runner    │──│   Executor  │──│  Registry   │              │  │  │
│  │  │   │             │  │   (loop)    │  │             │              │  │  │
│  │  │   └──────┬──────┘  └─────────────┘  └──────┬──────┘              │  │  │
│  │  │          │                                  │                     │  │  │
│  │  │          │         ┌─────────────┐         │                     │  │  │
│  │  │          └─────────│ MCP Manager │─────────┘                     │  │  │
│  │  │                    │             │                               │  │  │
│  │  │                    └──────┬──────┘                               │  │  │
│  │  └───────────────────────────┼───────────────────────────────────────┘  │  │
│  │                              │                                          │  │
│  │  ┌───────────────────────────┴───────────────────────────────────────┐  │  │
│  │  │                      SERVICES LAYER                                │  │  │
│  │  │                                                                    │  │  │
│  │  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │  │  │
│  │  │   │  Execution   │  │   API Logs   │  │   Search     │            │  │  │
│  │  │   │    State     │  │   Service    │  │   Index      │            │  │  │
│  │  │   └──────────────┘  └──────────────┘  └──────────────┘            │  │  │
│  │  │                                                                    │  │  │
│  │  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │  │  │
│  │  │   │    Daily     │  │  Knowledge   │  │   Session    │            │  │  │
│  │  │   │   Sessions   │  │    Graph     │  │   Archive    │            │  │  │
│  │  │   └──────────────┘  └──────────────┘  └──────────────┘            │  │  │
│  │  └────────────────────────────────────────────────────────────────────┘  │  │
│  │                                                                          │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                           │                                    │
│  ┌────────────────────────────────────────┴────────────────────────────────┐  │
│  │                         DATA LAYER                                       │  │
│  │                                                                          │  │
│  │   ┌─────────────────────────────────────────────────────────────────┐   │  │
│  │   │                    SQLite Database                               │   │  │
│  │   │         ~/Documents/agentzero/conversations.db                   │   │  │
│  │   │                                                                  │   │  │
│  │   │   sessions │ agent_executions │ messages │ execution_logs        │   │  │
│  │   └─────────────────────────────────────────────────────────────────┘   │  │
│  │                                                                          │  │
│  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │  │
│  │   │   Tantivy    │  │   Parquet    │  │    Config    │                  │  │
│  │   │   Index      │  │   Archives   │  │    Files     │                  │  │
│  │   └──────────────┘  └──────────────┘  └──────────────┘                  │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
└────────────────────────────────────────────────────────────────────────────────┘
```

### Container Descriptions

| Container | Technology | Purpose |
|-----------|------------|---------|
| **Web Dashboard** | React 19 + TypeScript + Vite | Interactive agent chat, configuration management |
| **CLI** | Rust + TUI | Terminal-based agent interaction |
| **Gateway** | Rust + Axum | HTTP/WebSocket APIs, static file serving |
| **Runtime** | Rust (agent-runtime) | Agent executor loop, tool execution, MCP |
| **Services** | Rust microservices | State management, logging, search, archival |
| **SQLite** | rusqlite | Sessions, executions, messages, logs |
| **Tantivy** | Full-text search | Message search across conversations |
| **Parquet** | Columnar storage | Long-term message archival |

---

## Level 3: Component Diagrams

### 3.1 Gateway Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              GATEWAY                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        HTTP API LAYER                                │   │
│  │                                                                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │  Agents  │ │  Skills  │ │Providers │ │   MCPs   │ │  Health  │  │   │
│  │  │  CRUD    │ │  CRUD    │ │  CRUD    │ │  CRUD    │ │  Check   │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │   │
│  │                                                                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐               │   │
│  │  │Conversa- │ │ Webhooks │ │ Gateway  │ │ Settings │               │   │
│  │  │  tions   │ │ Handler  │ │   Bus    │ │          │               │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                      │                                      │
│  ┌───────────────────────────────────┴─────────────────────────────────┐   │
│  │                      WEBSOCKET LAYER                                 │   │
│  │                                                                      │   │
│  │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │   │
│  │  │     Handler      │  │   Subscriptions  │  │     Messages     │  │   │
│  │  │                  │  │                  │  │                  │  │   │
│  │  │ - Connection     │  │ - Session scope  │  │ - ClientMessage  │  │   │
│  │  │ - Routing        │  │ - Execution scope│  │ - ServerMessage  │  │   │
│  │  │ - Event Router   │  │ - All scope      │  │ - Event types    │  │   │
│  │  └──────────────────┘  └──────────────────┘  └──────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                      │                                      │
│  ┌───────────────────────────────────┴─────────────────────────────────┐   │
│  │                      EXECUTION LAYER                                 │   │
│  │                                                                      │   │
│  │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │   │
│  │  │  ExecutionRunner │  │     Lifecycle    │  │    Delegation    │  │   │
│  │  │                  │  │                  │  │                  │  │   │
│  │  │ - invoke()       │  │ - Session create │  │ - Spawn subagent │  │   │
│  │  │ - stop()         │  │ - Execution state│  │ - Callback route │  │   │
│  │  │ - continue()     │  │ - Completion     │  │ - Registry       │  │   │
│  │  └──────────────────┘  └──────────────────┘  └──────────────────┘  │   │
│  │                                                                      │   │
│  │  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐  │   │
│  │  │   Continuation   │  │      Events      │  │      Hooks       │  │   │
│  │  │                  │  │                  │  │                  │  │   │
│  │  │ - After delegate │  │ - Stream convert │  │ - Web/CLI/Cron   │  │   │
│  │  │ - Spawn turn     │  │ - Broadcast      │  │ - Response route │  │   │
│  │  └──────────────────┘  └──────────────────┘  └──────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                      │                                      │
│  ┌───────────────────────────────────┴─────────────────────────────────┐   │
│  │                      SERVICES LAYER                                  │   │
│  │                                                                      │   │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐       │   │
│  │  │   Agent    │ │  Provider  │ │   Skill    │ │    MCP     │       │   │
│  │  │  Service   │ │  Service   │ │  Service   │ │  Service   │       │   │
│  │  └────────────┘ └────────────┘ └────────────┘ └────────────┘       │   │
│  │                                                                      │   │
│  │  ┌────────────┐ ┌────────────┐                                      │   │
│  │  │  Runtime   │ │  Settings  │                                      │   │
│  │  │  Service   │ │  Service   │                                      │   │
│  │  └────────────┘ └────────────┘                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Runtime Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           AGENT RUNTIME                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       EXECUTOR (Main Loop)                           │   │
│  │                                                                      │   │
│  │   User Message                                                       │   │
│  │        │                                                             │   │
│  │        ▼                                                             │   │
│  │   ┌─────────────────┐                                                │   │
│  │   │  PreProcess     │  ◄── Middleware (summarize, edit context)      │   │
│  │   │  Messages       │                                                │   │
│  │   └────────┬────────┘                                                │   │
│  │            │                                                         │   │
│  │            ▼                                                         │   │
│  │   ┌─────────────────┐      ┌─────────────────┐                      │   │
│  │   │   LLM Client    │ ───► │   Stream Events │                      │   │
│  │   │   (OpenAI API)  │      │   (Token, etc.) │                      │   │
│  │   └────────┬────────┘      └─────────────────┘                      │   │
│  │            │                                                         │   │
│  │            ▼                                                         │   │
│  │   ┌─────────────────┐                                                │   │
│  │   │  Tool Calls?    │                                                │   │
│  │   └───┬─────────┬───┘                                                │   │
│  │       │ No      │ Yes                                                │   │
│  │       ▼         ▼                                                    │   │
│  │   ┌───────┐  ┌─────────────────┐                                    │   │
│  │   │ Done  │  │  Execute Tools  │                                    │   │
│  │   └───────┘  └────────┬────────┘                                    │   │
│  │                       │                                              │   │
│  │                       ▼                                              │   │
│  │              ┌─────────────────┐                                    │   │
│  │              │ Check Actions   │                                    │   │
│  │              │ (respond/delegate)                                   │   │
│  │              └────────┬────────┘                                    │   │
│  │                       │                                              │   │
│  │                       └───────► Continue Loop                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       TOOL REGISTRY                                  │   │
│  │                                                                      │   │
│  │   Built-in Tools              MCP Tools                             │   │
│  │   ┌──────────────┐            ┌──────────────┐                      │   │
│  │   │ read_file    │            │ McpToolset   │                      │   │
│  │   │ write_file   │            │              │                      │   │
│  │   │ list_dir     │            │ - stdio      │                      │   │
│  │   │ execute_cmd  │            │ - HTTP       │                      │   │
│  │   │ memory       │            │ - SSE        │                      │   │
│  │   │ delegate     │            │              │                      │   │
│  │   │ respond      │            └──────────────┘                      │   │
│  │   │ list_*       │                                                  │   │
│  │   └──────────────┘                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       MIDDLEWARE PIPELINE                            │   │
│  │                                                                      │   │
│  │   ┌──────────────────────────────────────────────────────────────┐  │   │
│  │   │  PreProcessMiddleware                                         │  │   │
│  │   │  - SummarizationMiddleware (compress history)                 │  │   │
│  │   │  - ContextEditingMiddleware (remove old tool results)         │  │   │
│  │   └──────────────────────────────────────────────────────────────┘  │   │
│  │                                                                      │   │
│  │   ┌──────────────────────────────────────────────────────────────┐  │   │
│  │   │  EventMiddleware                                              │  │   │
│  │   │  - Logging, metrics, event transformation                     │  │   │
│  │   └──────────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Framework Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            FRAMEWORK LAYER                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                          zero-core                                    │  │
│  │                                                                       │  │
│  │   Core Traits                     Core Types                         │  │
│  │   ┌─────────────┐                 ┌─────────────┐                    │  │
│  │   │ Agent       │                 │ Event       │                    │  │
│  │   │ Tool        │                 │ Content     │                    │  │
│  │   │ Toolset     │                 │ Part        │                    │  │
│  │   │ Session     │                 │ EventActions│                    │  │
│  │   │ State       │                 │ RunConfig   │                    │  │
│  │   └─────────────┘                 └─────────────┘                    │  │
│  │                                                                       │  │
│  │   Context Hierarchy               Capabilities                       │  │
│  │   ┌─────────────────────────┐     ┌─────────────┐                    │  │
│  │   │ ReadonlyContext         │     │ Capability  │                    │  │
│  │   │   └─ CallbackContext    │     │ Registry    │                    │  │
│  │   │        └─ ToolContext   │     │ Router      │                    │  │
│  │   │        └─ InvocationCtx │     └─────────────┘                    │  │
│  │   └─────────────────────────┘                                        │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐              │
│  │ zero-agent │ │  zero-llm  │ │ zero-tool  │ │  zero-mcp  │              │
│  │            │ │            │ │            │ │            │              │
│  │ LlmAgent   │ │ Llm trait  │ │ ToolContext│ │ McpToolset │              │
│  │ Orchestrator│ │ OpenAiLlm │ │ adapters   │ │ McpClient  │              │
│  │ Workflow   │ │ Streaming  │ │            │ │ Connection │              │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘              │
│                                                                             │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐              │
│  │zero-middle-│ │zero-session│ │zero-prompt │ │  zero-app  │              │
│  │   ware     │ │            │ │            │ │            │              │
│  │            │ │ Session    │ │ Template   │ │ Prelude    │              │
│  │ Pipeline   │ │ State      │ │ Variables  │ │ Bootstrap  │              │
│  │ PreProcess │ │ Service    │ │            │ │            │              │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Services Layer Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SERVICES LAYER                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                      EXECUTION STATE SERVICE                          │  │
│  │                                                                       │  │
│  │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │  │
│  │   │  Session    │───►│  Execution  │───►│  Messages   │              │  │
│  │   │  (sess-xxx) │    │  (exec-xxx) │    │             │              │  │
│  │   └─────────────┘    └─────────────┘    └─────────────┘              │  │
│  │                                                                       │  │
│  │   Responsibilities:                                                   │  │
│  │   - Session lifecycle (Queued→Running→Completed/Crashed)             │  │
│  │   - Execution tracking (root + delegated)                            │  │
│  │   - Delegation coordination (pending_delegations counter)            │  │
│  │   - Continuation management (continuation_needed flag)               │  │
│  │   - Token consumption tracking                                       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                        API LOGS SERVICE                               │  │
│  │                                                                       │  │
│  │   Log Categories: session | token | tool_call | tool_result |        │  │
│  │                   thinking | delegation | system | error              │  │
│  │                                                                       │  │
│  │   Responsibilities:                                                   │  │
│  │   - Event logging during execution                                   │  │
│  │   - Log categorization and filtering                                 │  │
│  │   - Session summary computation                                      │  │
│  │   - Parent-child session tracking                                    │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐   │
│  │   DAILY SESSIONS   │  │  KNOWLEDGE GRAPH   │  │   SEARCH INDEX     │   │
│  │                    │  │                    │  │                    │   │
│  │ - Per-agent daily  │  │ - Entity extract   │  │ - Tantivy FTS      │   │
│  │ - Prompt versions  │  │ - Relationships    │  │ - Date filtering   │   │
│  │ - Context chains   │  │ - Per-agent graphs │  │ - Agent scoping    │   │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘   │
│                                                                             │
│  ┌────────────────────────────────────────────────────────────────────┐    │
│  │                       SESSION ARCHIVE                               │    │
│  │                                                                     │    │
│  │   - Parquet columnar storage for long-term archival                │    │
│  │   - Efficient compression and predicate pushdown                   │    │
│  │   - Integration with search index                                  │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.5 Frontend Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          WEB DASHBOARD                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                       TRANSPORT LAYER                                 │  │
│  │                                                                       │  │
│  │   ┌────────────────────────────────────────────────────────────────┐ │  │
│  │   │  HttpTransport                                                  │ │  │
│  │   │                                                                 │ │  │
│  │   │  HTTP Client          WebSocket Client       Subscription Mgr  │ │  │
│  │   │  - REST CRUD          - Auto-reconnect       - Scope filtering │ │  │
│  │   │  - Agents/Skills      - Heartbeat            - Deduplication   │ │  │
│  │   │  - Providers/MCPs     - Commands             - Sequence track  │ │  │
│  │   └────────────────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                        FEATURE MODULES                                │  │
│  │                                                                       │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │  │
│  │  │    Agent     │  │     Ops      │  │    Skills    │                │  │
│  │  │              │  │              │  │              │                │  │
│  │  │ WebChatPanel │  │  Dashboard   │  │  CRUD Panel  │                │  │
│  │  │ AgentsPanel  │  │  Sessions    │  │              │                │  │
│  │  │ Subagent Act │  │  Monitoring  │  │              │                │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                │  │
│  │                                                                       │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │  │
│  │  │     Logs     │  │     MCPs     │  │ Integrations │                │  │
│  │  │              │  │              │  │              │                │  │
│  │  │  Log Viewer  │  │  Config UI   │  │  Providers   │                │  │
│  │  │  Filtering   │  │  Test Tools  │  │  Setup       │                │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                          HOOKS                                        │  │
│  │                                                                       │  │
│  │  useConversationEvents  useConnectionState  useGlobalEvents          │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Level 4: Key Data Flows

### 4.1 Agent Execution Flow

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│  User   │     │   Web   │     │ Gateway │     │ Runtime │     │   LLM   │
│         │     │Dashboard│     │         │     │         │     │Provider │
└────┬────┘     └────┬────┘     └────┬────┘     └────┬────┘     └────┬────┘
     │               │               │               │               │
     │  Type message │               │               │               │
     │──────────────►│               │               │               │
     │               │               │               │               │
     │               │  WS: invoke   │               │               │
     │               │──────────────►│               │               │
     │               │               │               │               │
     │               │               │ Create session│               │
     │               │               │ + execution   │               │
     │               │               │───────────────│               │
     │               │               │               │               │
     │               │               │ invoke()      │               │
     │               │               │──────────────►│               │
     │               │               │               │               │
     │               │               │               │  API request  │
     │               │               │               │──────────────►│
     │               │               │               │               │
     │               │               │               │◄──────────────│
     │               │               │               │  Stream tokens│
     │               │               │◄──────────────│               │
     │               │◄──────────────│  Token events │               │
     │◄──────────────│  Update UI    │               │               │
     │               │               │               │               │
     │               │               │◄──────────────│               │
     │               │◄──────────────│  Complete     │               │
     │◄──────────────│               │               │               │
```

### 4.2 Delegation Flow

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│  Root   │     │ Gateway │     │Subagent │     │   LLM   │     │  User   │
│  Agent  │     │         │     │         │     │         │     │Dashboard│
└────┬────┘     └────┬────┘     └────┬────┘     └────┬────┘     └────┬────┘
     │               │               │               │               │
     │ delegate_tool │               │               │               │
     │──────────────►│               │               │               │
     │               │               │               │               │
     │               │ Create child  │               │               │
     │               │ execution     │               │               │
     │               │───────────────│               │               │
     │               │               │               │               │
     │               │ DelegationStarted             │               │
     │               │───────────────────────────────────────────────►
     │               │               │               │               │
     │               │ spawn_delegated_agent()       │               │
     │               │──────────────►│               │               │
     │               │               │               │               │
     │  Root completes (pending=1)   │               │               │
     │◄──────────────│               │               │               │
     │               │               │  Execute      │               │
     │               │               │──────────────►│               │
     │               │               │◄──────────────│               │
     │               │               │               │               │
     │               │◄──────────────│               │               │
     │               │  Subagent done│               │               │
     │               │               │               │               │
     │               │ DelegationCompleted           │               │
     │               │───────────────────────────────────────────────►
     │               │               │               │               │
     │               │ Callback to   │               │               │
     │◄──────────────│ root context  │               │               │
     │               │               │               │               │
     │               │ SessionContinuationReady      │               │
     │               │───────────────│               │               │
     │               │               │               │               │
     │ Continuation  │               │               │               │
     │ turn (sees    │               │               │               │
     │ callback)     │               │               │               │
     │──────────────►│               │               │               │
```

### 4.3 Subscription Scope Filtering

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        EVENT ROUTING WITH SCOPES                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Event Bus                                                                 │
│       │                                                                     │
│       ▼                                                                     │
│   ┌───────────────────────────────────────────────────────────────────┐    │
│   │                   Subscription Manager                             │    │
│   │                                                                    │    │
│   │   ┌─────────────────────────────────────────────────────────────┐ │    │
│   │   │  Client A: scope="session"                                   │ │    │
│   │   │  - Receives: Root execution events only                      │ │    │
│   │   │  - Receives: DelegationStarted, DelegationCompleted          │ │    │
│   │   │  - Filters:  Subagent Token, ToolCall, etc.                  │ │    │
│   │   └─────────────────────────────────────────────────────────────┘ │    │
│   │                                                                    │    │
│   │   ┌─────────────────────────────────────────────────────────────┐ │    │
│   │   │  Client B: scope="execution:exec-123"                        │ │    │
│   │   │  - Receives: Only events for exec-123                        │ │    │
│   │   │  - Use case: Debug view of specific execution                │ │    │
│   │   └─────────────────────────────────────────────────────────────┘ │    │
│   │                                                                    │    │
│   │   ┌─────────────────────────────────────────────────────────────┐ │    │
│   │   │  Client C: scope="all"                                       │ │    │
│   │   │  - Receives: All events (backward compatible)                │ │    │
│   │   └─────────────────────────────────────────────────────────────┘ │    │
│   └───────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Database Schema

```sql
-- Session: Top-level user interaction container
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,                    -- sess-{uuid}
    status TEXT NOT NULL,                   -- queued|running|paused|completed|crashed
    source TEXT NOT NULL,                   -- web|cli|cron|api|plugin
    root_agent_id TEXT NOT NULL,
    title TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    total_tokens_in INTEGER DEFAULT 0,
    total_tokens_out INTEGER DEFAULT 0,
    pending_delegations INTEGER DEFAULT 0,  -- Count of running subagents
    continuation_needed INTEGER DEFAULT 0,  -- Flag for continuation after delegates
    ward_id TEXT                            -- Active code ward name
);

-- Execution: Single agent turn within a session
CREATE TABLE agent_executions (
    id TEXT PRIMARY KEY,                    -- exec-{uuid}
    session_id TEXT NOT NULL REFERENCES sessions(id),
    agent_id TEXT NOT NULL,
    parent_execution_id TEXT REFERENCES agent_executions(id),
    delegation_type TEXT NOT NULL,          -- root|sequential|parallel
    task TEXT,                              -- Task description for delegated agents
    status TEXT NOT NULL,                   -- queued|running|paused|crashed|cancelled|completed
    started_at TEXT,
    completed_at TEXT,
    tokens_in INTEGER DEFAULT 0,
    tokens_out INTEGER DEFAULT 0,
    checkpoint TEXT,                        -- JSON for crash recovery
    error TEXT
);

-- Messages: Chat history linked to executions
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    execution_id TEXT NOT NULL REFERENCES agent_executions(id),
    role TEXT NOT NULL,                     -- user|assistant|tool
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    token_count INTEGER DEFAULT 0,
    tool_calls TEXT,                        -- JSON array
    tool_results TEXT                       -- JSON array
);

-- Logs: Execution event logging
CREATE TABLE execution_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    level TEXT NOT NULL,                    -- debug|info|warn|error
    category TEXT NOT NULL,                 -- session|token|tool_call|tool_result|...
    message TEXT NOT NULL,
    metadata TEXT,                          -- JSON
    duration_ms INTEGER
);
```

---

## Technology Stack Summary

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Frontend** | React 19 + TypeScript + Vite | Web dashboard |
| **Styling** | Tailwind CSS v4 + Radix UI | UI components |
| **HTTP Server** | Axum (Rust) | REST API |
| **WebSocket** | tokio-tungstenite | Real-time streaming |
| **Async Runtime** | Tokio | Async I/O |
| **Database** | SQLite (rusqlite) | Persistence |
| **Search** | Tantivy | Full-text search |
| **Archive** | Parquet | Long-term storage |
| **LLM Client** | OpenAI-compatible | AI inference |
| **MCP** | stdio/HTTP/SSE | External tools |

---

## Deployment

### Ports

| Port | Service | Protocol |
|------|---------|----------|
| 18791 | HTTP API + Web UI | HTTP |
| 18790 | WebSocket | WS |
| 3000 | Vite dev server | HTTP (dev only) |

### Data Directory

```
~/Documents/agentzero/
├── conversations.db      # SQLite database (WAL mode, r2d2 pool)
├── agents/{name}/        # Agent configs (YAML + AGENTS.md)
├── wards/                # Code Wards (persistent project directories)
│   ├── .venv/            #   Shared Python venv
│   ├── scratch/          #   Default ward for quick tasks
│   └── {ward-name}/      #   Agent-named projects
├── skills/{name}/        # Skill definitions (SKILL.md)
├── providers.json        # LLM provider configs
├── mcps.json             # MCP server configs
└── archives/             # Parquet archives
```

### Running

```bash
# Development (2 terminals)
npm run daemon           # Backend with auto-reload
npm run dev              # Frontend with hot reload

# Production
npm run build
cargo run -p daemon -- --static-dir ./dist
```
