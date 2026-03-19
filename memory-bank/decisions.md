# z-Bot — Decisions

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

### Tool Tiers (Core 7, Optional, Action) — Shell-First
- **Core (7)**: shell (w/ apply_patch), memory, ward, update_plan, list_skills, load_skill, grep — always available
- **Action (3)**: respond, delegate_to_agent, list_agents — always available, drive agent behavior
- **Optional**: read, write, edit, glob, todos, python, web_fetch, ui_tools, create_agent, introspection — configurable per agent
- Shell-first: the agent uses `shell` + `apply_patch` for file operations by default. Separate file tools (read/write/edit/glob) are opt-in.
- Old standalone knowledge_graph tools (5) removed — replaced by unified `graph` action in memory tool

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

### Memory Evolution: Brute-Force Cosine Over sqlite-vec
**Problem**: Hybrid search needs vector similarity. sqlite-vec requires loading a native extension (.dll/.so) at runtime — platform-specific, tricky distribution.
**Decision**: Compute cosine similarity in Rust. Load all embeddings from `embedding_cache` for the agent, compute similarity in-memory, take top-K.
**Rationale**: For <10K facts, brute-force is ~2-5ms. No extension loading, no platform concerns, no ANN index needed. Revisit if facts exceed 100K.

### Memory Evolution: Local Embeddings as Default
**Problem**: Embedding APIs cost money and require internet. Not all users have API keys configured.
**Decision**: Default to `fastembed` crate with `all-MiniLM-L6-v2` (384 dimensions, ~100MB ONNX model). Local ONNX inference on CPU — zero API calls, zero cost, works offline.
**Alternative**: OpenAI-compatible API (configurable per provider). User can switch to Ollama `nomic-embed-text` (768d) or OpenAI `text-embedding-3-small` (1536d) via `EmbeddingConfig`.

### Memory Evolution: Session Distillation Over Manual Memory
**Problem**: Agents forget to save important facts. Users shouldn't have to remind them.
**Decision**: After each session (>10 messages), fire-and-forget LLM call extracts durable facts (preferences, decisions, patterns, entities, instructions, corrections) into `memory_facts` with embeddings.
**Advantage**: We have full session transcripts (Session Tree).
**Safety**: Fire-and-forget (never blocks user), async spawn, only runs on sessions with sufficient signal (>10 messages).

### Memory Evolution: Hybrid Search Scoring
**Formula**: `(0.7 × vector_cosine + 0.3 × BM25_score) × confidence × recency_decay × mention_boost`
**Rationale**: Vector search handles semantic similarity ("preferred language" matches "coding in Rust"), FTS5 handles exact keyword matches ("SQLite" matches "SQLite"). Confidence, recency, and mention_count prevent stale or low-quality facts from dominating.

### Memory Evolution: Embedding Cache (Hash-Based Dedup)
**Problem**: Re-embedding unchanged content wastes API calls or compute time.
**Decision**: SHA-256 hash of text + model name as composite key in `embedding_cache` table. Before embedding, check cache.

### Memory Evolution: Pre-Compaction Memory Flush
**Problem**: When context window is trimmed, the agent loses access to potentially important information.
**Decision**: Before compaction, inject a `[MEMORY FLUSH]` system message warning the agent. Skip compaction for one turn to give the agent a chance to `save_fact`. On the next trigger, proceed with normal compaction.

### Knowledge Graph: Unified in Memory Tool
**Problem**: 5 standalone knowledge_graph tools cluttered the tool registry and duplicated the memory concept.
**Decision**: Remove standalone tools. Add `graph` action to existing memory tool. Entities and relationships are automatically extracted during session distillation — no manual management needed. The `graph` action provides query access when needed.

### Memory UI: Cross-Agent View by Default
**Problem**: Memory UI required selecting an agent first, showing "No memories found" even when memories existed.
**Decision**: Add `/api/memory` endpoint that lists ALL memories across all agents. UI shows all memories by default with optional agent filter. Stats computed from all memories. Agent badge shown when viewing across all agents.

### TriggerSource: Unified Session Origin Tracking
**Problem**: Sessions tracked their source (`web`, `cli`, `api`, `cron`, `connector`) but this wasn't documented for integration developers.
**Decision**: Document all invocation methods and their source values in architecture.md. Web sessions stay open for interactive use; CLI/Cron/API/Connector sessions auto-complete after execution. Source displayed in UI with badges and icons.

### Bridge Workers: STDIO Transport (Planned)
**Problem**: Bridge workers only support WebSocket transport. Some integration scenarios (embedded, local) benefit from subprocess STDIO communication.
**Decision**: Add STDIO transport following MCP server pattern. Gateway spawns worker processes configured in `bridge_workers.json`, communicates via newline-delimited JSON over stdin/stdout. Same protocol as WebSocket, different framing. Cross-platform (Windows, Linux, macOS).

### Plugins: STDIO with Bridge Protocol Reuse
**Problem**: Users want custom integrations (Slack, Discord, Strava) without modifying core codebase.
**Decision**: Node.js plugins in `~/Documents/zbot/plugins/`, spawned as child processes with STDIO transport. Reuse Bridge Protocol (hello/handshake, ping/pong, capability_invoke, inbound) — plugins appear as bridge workers to agents.
**Rationale**: Single protocol for all external integrations. Plugins can trigger agents via `inbound` messages and respond via `capability_invoke`. npm install on first start handles dependencies automatically.

### Plugins: Self-Contained Manifests
**Problem**: Configuration scattered across multiple files complicates plugin distribution.
**Decision**: Each plugin is a self-contained directory with `plugin.json` manifest (id, name, entry, env, auto_restart), `package.json` for npm dependencies, and `index.js` entry point.
**Rationale**: Drop-in deployment — copy directory, restart daemon, plugin auto-discovered and started. No central registry file to edit.

### Plugins: Per-Plugin User Configuration
**Problem**: Secrets (API tokens) shouldn't be in plugin manifests (checked into version control). Also, config orphaned when plugin deleted.
**Decision**: User config stored in `plugins/{plugin_id}/.config.json` — self-contained with plugin. Contains `settings` (non-sensitive) and `secrets` (masked in API responses, 0600 file permissions on Unix). Auto-created on discovery, deleted with plugin directory.
**Rationale**: Plugin authors ship `plugin.json` with env var references (`${SLACK_TOKEN}`); users set secrets via API. Dot-prefix (`config.json`) clearly separates user data from plugin code. Drop-in deployment — delete directory = clean uninstall.

### Plugins: Auto-Restart with Backoff
**Problem**: External services (Slack, Discord) have transient connection issues; plugins shouldn't require manual intervention.
**Decision**: `auto_restart: true` (default) with configurable `restart_delay_ms` (default 5000). Failed plugins restart automatically after delay.
**Rationale**: Hands-off operation. For intentional stops, auto_restart is skipped. Log messages indicate restart reason.

### Resource Indexing: Lazy On-Demand
**Problem**: Skills and agents need semantic search, but scanning directories at startup is wasteful.
**Decision**: Indexing happens on-demand when `index_resources` tool is called. First discovery falls back to disk scan if no index exists.
**Rationale**: Avoids startup overhead. Users/agents explicitly trigger reindex when they know files have changed. Mtime tracking enables staleness detection without full reindex.

### Resource Indexing: Dual Storage (Memory + Graph)
**Problem**: Semantic search needs text embeddings, but relationship queries need structured graph data.
**Decision**: Store skills/agents in both MemoryFactStore (for semantic search via BM25 + embeddings) and KnowledgeGraphStore (for entity relationships).
**Rationale**: Each storage is optimized for its access pattern. Memory provides hybrid search. Graph provides relationship traversal. Storing in both enables rich discovery without sacrificing performance.

### Semantic Search: Merge Strategy
**Problem**: Semantic search might miss exact keyword matches; keyword search can't find semantically similar terms.
**Decision**: `analyze_intent` tries semantic search first, then merges with keyword matching results, deduplicating by name.
**Rationale**: Best of both worlds. Semantic finds "web scraping" when user says "extract data from websites". Keyword ensures exact matches aren't missed. Highest relevance score wins on duplicates.

### Intent Analysis: LLM-First, Pure Analysis
**Problem**: Heuristic keyword matching misses hidden intents; auto-loading skills pollutes context; injection breaks tool boundaries.
**Decision**: `analyze_intent` is a **pure analysis** tool:
- LLM is PRIMARY analyzer (receives ALL resources, returns intelligent recommendations)
- Heuristics only as FALLBACK (LLM failed or unavailable)
- NO auto-loading — returns recommendations only
- Agent explicitly calls `load_skill` when needed
**Rationale**: Clean separation of concerns. LLM provides intelligent analysis (hidden intents, execution strategy, ward suggestions). Agent retains control over what to load. No surprise context injection. See `memory-bank/intent-analysis.md` for full documentation.

## Patterns We Did NOT Adopt

These were considered during the Codex gap analysis and explicitly rejected:

| Pattern | Why Skipped |
|---------|-------------|
| Platform-native sandboxing | z-Bot is a web platform, not a CLI. Users explicitly configure agent tools. Different threat model. |
| Interactive approval (exec policy) | Would require WebSocket round-trips + UI modals. z-Bot agents run in background. Tool tiers are sufficient. |
| TOML config hierarchy (7 layers) | z-Bot has a single config directory per installation. JSON/YAML is simpler and sufficient. |
| OpenTelemetry | `tracing` crate is sufficient. OTEL adds infrastructure requirements not needed at current scale. |
| Git ghost commits (undo) | Different UX for web platform vs interactive CLI. Not critical path. |
| Starlark policy language | Over-engineered. Simple JSON allowlists are sufficient. |
| sqlx compile-time query checking | Overkill for AZ's current schema complexity. |

## Development Workflow

Distilled patterns from building z-Bot:

1. **Plans with concrete data models** — Show actual schemas/types, not prose. File-level specificity. Phase grouping by architectural layer.
2. **Right level of detail** — Specify what to create, not how. File paths guide but don't micromanage. Assumes competence on implementation details.
3. **Layer-by-layer implementation** — Follow the dependency graph: `framework/ → runtime/ → services/ → gateway/ → apps/`. Backend before frontend.
4. **Test each phase** — `cargo check --workspace` after Rust changes, `npm run build` after TypeScript. Don't batch all testing at the end.
5. **Read before write** — Check existing patterns, understand current state, avoid duplicating functionality.
6. **Anti-patterns**: Starting without reading context. Solving problems not asked. Ignoring existing patterns. Plans without data models. Skipping root cause analysis.
