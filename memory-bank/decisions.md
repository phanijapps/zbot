# Agent Zero — Decisions

## Technology Choices

### Why Rust
- Memory safety without GC
- Excellent async story (tokio)
- Great tooling (cargo, clippy)
- Single binary distribution

### Why SQLite + WAL Mode + r2d2 Pool
- Zero configuration, portable (single file), ACID transactions
- WAL mode enables concurrent readers with single writer
- r2d2 pool (max 8, min idle 2) replaces single Mutex — eliminates serialization bottleneck
- Sync rusqlite + pool chosen over async sqlx: simpler, DB operations are simple enough that sync + pool is fine
- PragmaCustomizer applies WAL/perf pragmas to every pooled connection

### Why Single Daemon
- Simpler deployment and debugging
- Shared state without IPC complexity
- Single port configuration
- Memory efficiency

### Why No Desktop Wrapper
- Browsers are more capable than custom webviews
- Easier deployment (no native installers)
- Better developer experience (standard web tools)
- Cross-platform without platform-specific builds

### Why React + Vite
- React 19 for UI components
- Vite for fast dev server and bundling
- Tailwind CSS v4 + Radix UI for styling and accessible primitives

### Why Instructions in AGENTS.md
- Human-readable and editable
- Version control friendly
- Markdown rendering in UI
- Separates behavior from configuration

## Architecture Decisions

### Tool Tiers (Core 11, Optional, Action)
- **Core (11)**: shell, read, write, edit, memory, ward, todo, list_skills, load_skill, grep, glob — always available
- **Action (3)**: respond, delegate_to_agent, list_agents — always available, drive agent behavior
- **Optional**: python, web_fetch, ui_tools, knowledge_graph, create_agent, introspection — configurable per agent
- Tiers prevent agents from accidentally accessing dangerous tools without explicit configuration

### Memory: 4-Tier Hierarchy
| Tier | Path | Purpose |
|------|------|---------|
| Global Shared | `agents_data/shared/*.json` | user_info, workspace, patterns, session_summaries |
| Agent | `agents_data/{agent_id}/memory.json` | Per-agent private context |
| Ward | `wards/{ward_id}/.ward_memory.json` | Project-specific context |
| Session | `agent_data/{session_id}/` | Ephemeral: attachments, scratchpad |

File locking (fs2 crate) protects shared memory from concurrent access.

### Code Wards Over Session-Scoped Dirs
**Problem**: Per-session `code/sess-{uuid}/` directories were isolated and ephemeral — no project continuity.
**Decision**: Named persistent directories (wards) that agents create, name, and navigate autonomously.
**Key principles**: Agent autonomy (agent decides ward name/location), shared Python venv across all wards, per-ward node_modules (Node convention), simplicity (wards are directories, no metadata database).

### Agent Autonomy in Ward Selection
The agent — not the user — decides which ward to work in. System prompt instructs: list wards, match to task, create new one if needed, use `scratch` for one-offs.

### Gateway Crate Decomposition (13 Crates)
Gateway was a 73-file monolith (~15,675 LOC). Extracted into focused crates:
- `gateway-events` — EventBus, GatewayEvent, HookContext
- `gateway-database` — DatabaseManager, pool, schema, ConversationRepository
- `gateway-templates` — Prompt assembly, shard injection
- `gateway-connectors` — ConnectorRegistry, dispatch
- `gateway-services` — AgentService, ProviderService, McpService, SkillService, SettingsService
- `gateway-execution` — ExecutionRunner, delegation, lifecycle, streaming, BatchWriter
- `gateway-hooks` — Hook trait, HookRegistry, CliHook, CronHook
- `gateway-cron` — CronJobConfig, CronService
- `gateway-bus` — GatewayBus trait, SessionRequest, SessionHandle
- `gateway-ws-protocol` — ClientMessage, ServerMessage, SubscriptionScope
- `gateway/src/` — Thin shell: HTTP routes, WebSocket handler, AppState

Dependency direction: events ← database ← services ← execution ← gateway (thin shell). No reverse dependencies.

### Parallel Tool Execution (Always Parallel)
When the LLM returns multiple tool calls, they all execute concurrently via `futures::future::join_all`. Sequential loop replaced with concurrent execution. Events emitted in original order (start upfront, results in order). `take_actions()` atomic capture prevents race conditions.

### Real Streaming (mpsc Channel Bridge)
5ms/char simulation removed. Tokens stream in real-time from LLM to UI via mpsc channel bridge. Intermediate text alongside tool calls now visible.

### Batch DB Writes (BatchWriter)
Background task decouples DB writes from streaming. Token updates coalesced by execution_id, logs batched. Flushes every 100ms or when 10+ items queued. Prevents hot-path DB writes from blocking stream processing.

### RwLock Service Caching
ProviderService, McpService, and SettingsService use RwLock caches (providers.json, mcps.json, settings.json). Reads don't block each other. Cache invalidated on writes.

### AGENTS.md as Documentation Standard
Every crate directory has an AGENTS.md describing what it does, its key files, and what it does NOT handle. These serve as both human documentation and AI context.

## Patterns We Did NOT Adopt

These were considered during the Codex gap analysis and explicitly rejected:

| Pattern | Why Skipped |
|---------|-------------|
| Platform-native sandboxing | AZ is a web platform, not a CLI. Users explicitly configure agent tools. Different threat model. |
| Interactive approval (exec policy) | Would require WebSocket round-trips + UI modals. AZ agents run in background. Tool tiers are sufficient. |
| TOML config hierarchy (7 layers) | AZ has a single config directory per installation. JSON/YAML is simpler and sufficient. |
| OpenTelemetry | `tracing` crate is sufficient. OTEL adds infrastructure requirements not needed at current scale. |
| Git ghost commits (undo) | Different UX for web platform vs interactive CLI. Not critical path. |
| Starlark policy language | Over-engineered. Simple JSON allowlists are sufficient. |
| sqlx compile-time query checking | Overkill for AZ's current schema complexity. |

## Development Workflow

Distilled patterns from building AgentZero:

1. **Plans with concrete data models** — Show actual schemas/types, not prose. File-level specificity. Phase grouping by architectural layer.
2. **Right level of detail** — Specify what to create, not how. File paths guide but don't micromanage. Assumes competence on implementation details.
3. **Layer-by-layer implementation** — Follow the dependency graph: `framework/ → runtime/ → services/ → gateway/ → apps/`. Backend before frontend.
4. **Test each phase** — `cargo check --workspace` after Rust changes, `npm run build` after TypeScript. Don't batch all testing at the end.
5. **Read before write** — Check existing patterns, understand current state, avoid duplicating functionality.
6. **Anti-patterns**: Starting without reading context. Solving problems not asked. Ignoring existing patterns. Plans without data models. Skipping root cause analysis.
