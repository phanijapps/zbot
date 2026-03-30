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
| Ward | `wards/{ward_id}/AGENTS.md` | Project-specific context (ward memory) |
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

### Intent Analysis: Autonomous Middleware (Not a Tool, Not Pre-Collected Arrays)
**Problem**: Earlier versions tried two approaches that both failed: (1) a tool the agent was supposed to call (but it was never registered), and (2) pre-collecting all resources into arrays to pass to the LLM (wasted tokens on irrelevant resources).
**Decision**: Intent analysis is an autonomous middleware that indexes resources into `memory_facts` with local embeddings, performs semantic search, and sends only top-N relevant resources to the LLM. Not a tool agents call. Not a full catalog dump.
**Rationale**: Autonomous indexing + semantic search means the LLM only sees relevant resources (8 skills, 5 agents, 5 wards max). No wasted tokens. No registration issues. Middleware runs before the root agent's first LLM call — transparent and automatic. See `memory-bank/intent-analysis.md` for full documentation.

### Modular System Prompt Config (SOUL.md, OS.md, Overridable Shards)
**Problem**: A single `INSTRUCTIONS.md` file mixed identity, platform commands, and behavior rules. No way to override individual shards without forking the embedded defaults.
**Decision**: Split system prompt into modular config files at `~/Documents/zbot/config/`:
- `SOUL.md` — agent identity/personality
- `INSTRUCTIONS.md` — execution rules
- `OS.md` — auto-generated platform-specific commands (Windows/Mac/Linux)
- `shards/` — overridable shards (defaults written to disk on first run)
- `distillation_prompt.md` — customizable distillation prompt
**Rationale**: Users can customize any piece independently. OS-specific commands are auto-generated (no manual maintenance). Shards can be overridden by placing a file with the same name in `config/shards/`. Extra user shards are automatically included. Assembly: `gateway/gateway-templates/src/lib.rs`.

### ThrottledLlmClient: Per-Provider Concurrent Request Limiting
**Problem**: Multiple concurrent delegations + root agent can burst many simultaneous LLM calls to the same provider, triggering 429 rate limits.
**Decision**: Wrap LLM clients with `ThrottledLlmClient` that uses a shared `tokio::sync::Semaphore` per provider. All executors sharing the same provider share the semaphore.
**Rationale**: Simple, composable. Configured via `maxConcurrentRequests` in `providers.json` (default: 3). Wrapping chain: `OpenAiClient -> RetryingLlmClient -> ThrottledLlmClient`. Implementation: `runtime/agent-runtime/src/llm/throttle.rs`.

### Model Capabilities Registry: Data-Driven Over Hardcoded
**Problem**: Model context windows were hardcoded in a match statement (`get_model_context_window`). No capability metadata — agents could enable thinking on models that don't support it. Adding new models required code changes.
**Decision**: Three-layer model registry: bundled JSON (embedded in binary) > local overrides (`config/models.json`) > unknown-model fallback. Each model has capabilities (tools, vision, thinking, embeddings, voice, imageGeneration, videoGeneration) and context window (input/output tokens).
**Rationale**: Data-driven — new models added without code changes. Local overrides for custom/private models. Bundled registry updated with releases. Executor validates `thinking_enabled` against model capabilities (warn + disable). Context window resolution replaces hardcoded lookup. Implementation: `gateway/gateway-services/src/models.rs`, `gateway/templates/models_registry.json`.

### Provider Default Model: Explicit Over Positional
**Problem**: Default model was `provider.models[0]` — an implicit positional convention. Reordering the models array silently changed which model every agent used.
**Decision**: Added `defaultModel` field to Provider struct. Priority: explicit `defaultModel` > first in `models` array > `"gpt-4o"` fallback.
**Rationale**: Explicit is always better than implicit. Users can set their preferred default without worrying about array order. Implementation: `gateway/gateway-services/src/providers.rs`.

### Settings UI: Deprecate Optional Tool Toggles
**Problem**: Optional tool toggles (python, webFetch, todos, fileTools, uiTools, createAgent) confused non-technical users. Most didn't work correctly — TS types referenced non-existent backend keys (`grep`, `glob`, `loadSkill`). The toggles gave a false sense of control.
**Decision**: Remove the Optional Tools section from the Settings UI. Keep introspection enabled via `settings.json`. Backend `ToolSettings` struct unchanged — tools can still be enabled programmatically or via direct JSON editing. UI focuses on the two settings that actually matter: context protection (offload) and logging.
**Rationale**: Simpler surface for non-technical users. The tools that matter (shell, apply_patch, memory, ward) are always on. The deprecated toggles were rarely used and poorly implemented.

### Providers Page: Card Grid Over Split Panel
**Problem**: The original split-panel layout (sidebar list + detail pane) was a developer tool pattern. No inline editing, no guided setup, no capability visibility. Non-technical users didn't know what to do.
**Decision**: Card grid with responsive 2-column layout. Each card shows provider name, status badge (Connected/Not tested/Active), model chips with capability badges. Click opens a slide-over detail panel with view/edit toggle. Empty state shows Top-3 preset cards (OpenAI, Anthropic, Ollama) with inline "type API key and connect" flow.
**Rationale**: Cards give visual overview. Inline connect reduces the happy path to 2 clicks. Slide-over preserves context (grid visible behind). View/Edit toggle prevents accidental changes. 9 provider presets with pre-filled baseUrl and models. Implementation: `apps/ui/src/features/settings/` (moved from integrations during UI revamp).

### UI Revamp: 7 Pages → 3 Consolidated Pages
**Problem**: UI had 7+ separate pages (Settings, Providers, MCPs, Skills, Agents, Workers, Schedules) with inconsistent layouts, no help text, and jargon-heavy labels. Non-technical users couldn't navigate.
**Decision**: Consolidate into 3 tabbed pages:
- **Settings** (Providers | General | Logging) — system setup, providers as default tab
- **Agents** (My Agents | Skills Library | Schedules) — agent management with card grid
- **Integrations** (Tool Servers | Plugins & Workers) — MCPs renamed to "Tool Servers", workers+plugins unified
**Rationale**: Maps to user mental model: Setup → Build → Connect. Tabs reduce navigation decisions. Every section gets contextual help text and rich empty states for onboarding. Card grid + slide-over pattern used consistently across all pages.

### UI Revamp: Design System — Warm Editorial Palette
**Problem**: Existing UI used generic system fonts and basic colors. No distinctive visual identity.
**Decision**: Warm copper accent (`#c8956c` dark, `#a07d52` light), cream/charcoal backgrounds, sidebar always dark in both themes. CSS custom properties for full themability. Backwards-compat aliases for all replaced tokens during migration.
**Rationale**: User preferred existing font stack over Fraunces serif for headings. Sidebar-always-dark provides visual anchor. Token aliases prevent silent CSS breakage. Implementation: `apps/ui/src/styles/theme.css`, `components.css`.

### Plugins vs Workers: Separate APIs, Shared Runtime
**Problem**: Plugins (auto-discovered from `~/Documents/zbot/plugins/`) and bridge workers (WebSocket-connected) both appear in `/api/bridge/workers` when running, causing the UI to show plugins as workers.
**Decision**: Use `/api/plugins` as source of truth for plugin listing (state, version, auto_restart, config). Enrich with bridge worker data (capabilities, resources) when plugin is connected. Filter bridge workers list to exclude entries matching known plugin IDs.
**Rationale**: Plugin `adapter_id` equals plugin `id` (e.g. `"slack"`, NOT `"plugin:slack"`). A running plugin appears in both APIs. UI must deduplicate. Implementation: `apps/ui/src/features/integrations/WebIntegrationsPanel.tsx`.

### Score Threshold + Per-Category Caps for Semantic Search
**Problem**: Sending all indexed resources to the LLM wastes tokens on irrelevant results and produces noisy recommendations.
**Decision**: Apply a minimum relevance score threshold (0.15) and per-category caps (8 skills, 5 agents, 5 wards) when selecting resources for intent analysis.
**Rationale**: Recall 50 results from `memory_facts`, filter by score, cap per category. LLM sees only relevant resources. Thresholds tuned empirically — 0.15 filters noise without losing useful matches.

### Child Session Lifecycle Fix
**Problem**: Delegated subagent sessions were left in `running` state after completion, creating orphan sessions.
**Decision**: Child sessions are now explicitly marked `completed` when the subagent finishes execution.
**Rationale**: Clean lifecycle prevents orphaned sessions from accumulating in the database. Combined with delegation semaphore (max 3 concurrent) to prevent resource exhaustion.

### Session Distillation: Lower Threshold, Broader Trigger
**Problem**: Distillation only fired after `invoke()` and required 10+ messages, missing useful short sessions and continuation sessions.
**Decision**: Fire distillation after both `invoke()` and `invoke_continuation()`. Lower min messages threshold from 10 to 4.
**Rationale**: Many useful sessions are short (4-6 messages). Continuation sessions often contain important follow-up decisions. Distillation prompt is customizable via `config/distillation_prompt.md`.

### Recall: Tool-Call Based, Not Hidden Injection
**Problem**: Hidden memory injection is invisible — agents can't learn when or what to recall, and developers can't debug what was injected.
**Decision**: Recall is an explicit tool call (`memory recall`). The agent decides when and what to recall. Results appear in the conversation as tool output.
**Rationale**: Visible in transcripts, debuggable in logs, learnable by the agent (it sees what works). Nudges at session start, ward entry, and post-delegation prompt the agent to recall without forcing it.

### Corrections as Rules (NEVER/ALWAYS)
**Problem**: LLMs treat "correction: don't do X" as a suggestion. They follow "NEVER do X" more reliably.
**Decision**: Top correction facts are formatted as imperative rules ("NEVER use `rm -rf` without confirmation", "ALWAYS check ward before file operations") and injected first in recall results.
**Rationale**: Rule-formatted corrections have higher compliance in practice. Filtered by query relevance so only applicable corrections appear.

### Graph Traversal via SQLite Recursive CTE
**Problem**: Knowledge graph expansion for recall needs graph traversal. Neo4j is the standard but adds an external dependency.
**Decision**: 2-hop BFS via SQLite recursive CTE behind a `GraphTraversal` trait. Trait-based so Neo4j can be swapped in later.
**Rationale**: Pi 4 safe (<10ms for 2-hop expansion), zero extra dependencies. SQLite CTE handles the current graph size (198+ entities, 333+ relationships) easily. Neo4j swap is a trait implementation change, not a redesign.

### Temporal Decay with Per-Category Half-Lives
**Problem**: Not all facts stale at the same rate. Domain knowledge ("project uses React 19") stales faster than corrections ("NEVER skip tests").
**Decision**: Per-category half-lives configured in `recall_config.json`: domain facts (30d), preferences (60d), corrections (90d), strategies (90d). Facts past their half-life decay exponentially in recall scoring; past 2x half-life they move to `memory_facts_archive`.
**Rationale**: Keeps recall fresh without losing corrections. Archive preserves audit trail.

### Session Offload to JSONL.gz
**Problem**: Session transcripts are the largest data in SQLite. After distillation, raw transcripts are dead weight.
**Decision**: `zero sessions archive --older-than N` compresses transcripts to JSONL.gz files. `sessions.archived` column tracks state. `zero sessions restore <id>` reverses the process.
**Rationale**: Keeps SQLite lean for Pi 4. Transcripts are already distilled — the extracted facts, episodes, and entities survive independently. Restorable if needed.

### Entity Dedup as __global__
**Problem**: The same entity (e.g., "React") was duplicated per agent — "React" x4 across root, researcher, coder, reviewer.
**Decision**: Entities are stored under `__global__` agent scope. Cross-agent dedup during distillation merges identical entities.
**Rationale**: Single source of truth. Graph queries return one "React" node with all relationships, not four disconnected copies.

### Execution Intelligence Dashboard Over Flat Logs
**Problem**: The log viewer was 845 lines of flat session/log rendering. No visual structure, no execution shape, no interactivity.
**Decision**: Visual observability dashboard with KPI sparkline cards, inline mini waterfalls in the session list, expandable full waterfall timelines with delegation spans and tool dots, hover tooltips, click-through detail panels, and auto-refresh for running sessions.
**Rationale**: Operators need to see execution shape at a glance — which delegations happened, where time was spent, what tools were called. Flat logs require scrolling and mental reconstruction.

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
