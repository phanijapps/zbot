# Tool System

Current as of 2026-05-29. This page describes how z-Bot exposes tools to the LLM at runtime.

## Runtime Shape

Tools implement `zero_core::Tool`:

| Method | Purpose |
|--------|---------|
| `name()` | Stable function name exposed to the model. |
| `description()` | Function description sent in the tool schema. |
| `parameters_schema()` | JSON Schema for arguments; the executor hardens it before sending to the provider. |
| `response_schema()` | Optional response schema. Most tools omit this today. |
| `permissions()` | Declarative risk/capability metadata. Hooks and guards enforce the current safety behavior. |
| `validate()` | Optional pre-execution validation. |
| `execute(ctx, args)` | Runs the tool and returns JSON. |

The live executor uses `agent_runtime::ToolRegistry`, an ordered `Vec<Arc<dyn Tool>>` registry with `register`, `get_all`, `tool_names`, and `find`. `zero_tool::ToolRegistry` also exists as a framework-level `Toolset` implementation, but the gateway execution path dispatches through the runtime registry.

## Built-in Tool Implementations

Built-in tool implementations live under `runtime/agent-tools/src/tools/`.

| Area | Tools |
|------|-------|
| Execution | `shell`, `write_file`, `edit_file`, `python`, `todos`, `update_plan`, `set_session_title`, `execution_graph`, skill load/list helpers |
| File/search | `read`, `write`, `edit`, `grep`, `glob` |
| Memory and wards | `memory`, `ward` |
| Intelligence | `graph_query`, `ingest`, `goal` |
| Delegation support | `list_agents`, `create_agent` |
| UI and media | `request_input`, `show_content`, `multimodal_analyze` |
| External resources | `web_fetch`, `query_resource` |
| Introspection | `list_tools`, `list_mcps`, `list_skills` |

`runtime/agent-tools/src/tools/mod.rs` still exposes factory functions:

| Function | Role |
|----------|------|
| `core_tools(fs, fact_store, ward_usage)` | Canonical crate-level list of always-on tools. |
| `optional_tools(fs, settings)` | Optional file, todos, Python, web, UI, agent creation, and introspection tools based on `ToolSettings`. |
| `builtin_tools_with_fs(fs)` | Legacy "everything enabled" helper. New code should prefer explicit registration. |

These factories are not the source of truth for the live gateway executor. The live registry is assembled manually in `gateway/gateway-execution/src/invoke/executor.rs` so root and delegated agents receive different tool sets.

## Live Gateway Registration

`ExecutorBuilder::build_tool_registry` creates a registry from the current execution mode and available adapters.

### Root Agent

The root agent is an orchestrator. In non-chat mode it runs with single-action mode enabled, so only the first model tool call in a turn is executed.

Always registered for root:

| Tool | Why root has it |
|------|-----------------|
| `shell` | Run commands when orchestration requires direct inspection or execution. |
| `memory` | Persist and recall user, agent, ward, and session facts. |
| `ward` | Select, inspect, and create project wards. |
| `update_plan` | Maintain the task checklist. |
| `set_session_title` | Set a readable session label. |
| `grep` | Search workspace content. |
| `respond` | End the turn with a user-facing response. |
| `delegate_to_agent` | Spawn specialist or ward agents. |
| `multimodal_analyze` | Vision fallback. |

Conditionally registered for root:

| Tool | Condition |
|------|-----------|
| `run_procedure` | A `ProcedureStore` is wired. It receives a snapshot registry so it cannot recursively call itself. |
| `steer_agent` | A steering registry is wired. |
| `wait_agent`, `kill_agent` | Agent result bus, state service, and conversation repository are wired. |
| `graph_query` | A `KnowledgeGraphStore` is wired. |
| `ingest` | An ingestion adapter is wired. |
| `goal` | A goal adapter is wired. |
| `read`, `glob` | `ToolSettings.file_tools` is true. |
| `query_resource` | A connector resource provider is available. |

Notably, root does not currently get `load_skill`, `list_skills`, `list_agents`, or `execution_graph` in the live gateway registry.

### Delegated Agents

Delegated agents do the specialist work and report back with `respond`.

Always registered for delegated agents:

| Tool | Why delegated agents have it |
|------|------------------------------|
| `shell` | Run commands in the selected ward. |
| `write_file`, `edit_file` | Create and modify files through dedicated file tools. |
| `load_skill`, `list_skills`, `list_mcps` | Load local operating instructions and inspect configured MCPs. |
| `grep` | Search workspace content. |
| `ward` | Change or inspect ward context. |
| `memory` | Save and recall facts. |
| `respond` | Return delegated result to the parent. |
| `multimodal_analyze` | Vision fallback. |

Conditionally registered for delegated agents:

| Tool | Condition |
|------|-----------|
| `graph_query` | A `KnowledgeGraphStore` is wired. |
| `ingest` | An ingestion adapter is wired. |
| `goal` | A goal adapter is wired. |

Write-capable delegated tools get the fact store through `with_fact_store` when it is available. That lets file edits feed the memory/knowledge extraction path.

## Settings and UI

Tool settings are stored in `{data_dir}/settings.json` under `tools` and are represented by `agent_tools::ToolSettings`.

| Setting | Current effect |
|---------|----------------|
| `python` | Deprecated. Kept in settings/API for compatibility, but ignored by the live gateway registry and `optional_tools`. Run Python through `shell`. |
| `webFetch` | Deprecated. Kept in settings/API for compatibility, but ignored by the live gateway registry and `optional_tools`. Prefer MCP/browser/search integrations or explicit scripts. |
| `uiTools` | Enables UI tools in the crate-level optional factory, but the live gateway registry does not currently register them. |
| `createAgent` | Enables `create_agent` in the crate-level optional factory, but the live gateway registry does not currently register it. |
| `introspection` | Enables `list_tools` and `list_mcps` in the crate-level optional factory. Delegated agents get `list_mcps` directly regardless of this flag. |
| `fileTools` | Live root registry adds `read` and `glob`; the crate-level optional factory would also add `write` and `edit`. |
| `todos` | Deprecated. Replaced by `update_plan`; kept in settings/API for compatibility, but ignored by the live gateway registry and `optional_tools`. |
| `offloadLargeResults` | Enables tool-result offload in `AgentExecutor`. |
| `offloadThresholdTokens` | Converted to characters by `threshold * 4` and used by the offload path. |

The Settings UI intentionally exposes only tool-result offload controls. The optional tool toggles remain in backend settings and API types, but they are not shown in the current UI.

### Deprecated Tools

The first deprecation batch is `todos`, `python`, and `web_fetch`. Their tool
implementations remain in the crate so older callers and tests can compile, but
they are no longer returned by `optional_tools()` and are not registered by the
live gateway executor. This makes deprecation effective without breaking old
`settings.json` files that still contain `todos`, `python`, or `webFetch`.

## MCP Tools

MCP servers are configured per agent. During executor build, `ExecutorBuilder::build_mcp_manager` asks `McpService` for the agent's configured MCP server IDs and starts each server through `agent_runtime::McpManager`.

MCP tools are not inserted into the built-in registry. They are listed dynamically when the executor builds the OpenAI-compatible tool schema:

1. For each configured MCP server, the executor calls `client.list_tools()`.
2. Each MCP tool name is exposed as `{normalized_server_id}__{normalized_tool_name}` to satisfy provider tool-name constraints.
3. The MCP parameter schema is normalized and hardened.
4. At execution time, a tool name containing `__` is routed back through `McpManager::execute_tool(server_id, actual_tool_name, args)`.

The separate `framework/zero-mcp` crate also provides `McpTool` and `McpToolset` wrappers that implement `zero_core::Tool`/`Toolset` over MCP clients. The live gateway path currently uses the runtime MCP manager path described above.

## Execution Flow

1. `AgentExecutor::build_tools_schema` serializes all built-in registry tools, then appends MCP tools from configured servers.
2. The LLM returns zero or more tool calls.
3. In single-action mode, extra tool calls are dropped before execution.
4. `before_tool_call` hooks can block a call. Delegated agents use this to reject shell commands that write files through redirects, heredocs, `tee`, and similar patterns.
5. Non-blocked calls run sequentially or in parallel depending on `ExecutorConfig.tool_execution_mode`. Parallel mode uses `join_all`.
6. Built-in calls dispatch with `ToolRegistry::find(name)`, set the per-call `function_call_id` on shared context, clear actions, execute the tool, and collect tool actions.
7. MCP calls dispatch through `McpManager`.
8. Results are serialized to JSON strings, optionally offloaded to `{config_dir}/temp`, and then appended as tool messages in original tool-call order.

## Safety Model

Current safety is a combination of declared permissions, tool-local guards, and executor hooks:

| Layer | Examples |
|-------|----------|
| `ToolPermissions` | Declarative risk/capability metadata for routing and warnings. |
| Tool guards | Path sanitization, network guardrails, shell command checks, timeouts. |
| Executor hooks | Delegated shell write bypass blocker; post-error guidance for failed tool calls. |
| Tool split | File mutation should use `write_file`/`edit_file` instead of shell redirects. |

Do not assume `permissions()` alone enforces policy. When adding risk-sensitive tools, implement concrete checks in the tool or execution hook.

## Adding a Tool

1. Implement `zero_core::Tool` in `runtime/agent-tools/src/tools/{area}.rs`.
2. Export it from `runtime/agent-tools/src/tools/mod.rs` and, if it is public API, from `runtime/agent-tools/src/lib.rs`.
3. Add parameters with a strict JSON Schema; the executor will add missing strictness, but the tool should still define the intended contract.
4. Register it in the live path in `gateway/gateway-execution/src/invoke/executor.rs` for root, delegated, or both.
5. If it needs stores or adapters, add explicit builder fields and `with_*` methods on `ExecutorBuilder`, then wire them from `invoke_bootstrap` or `AppState`.
6. If it should be user-configurable, update `ToolSettings`, settings HTTP types, transport types, and decide whether the Settings UI should expose it.
7. Add focused tests for schema, validation, execution behavior, and the live registry condition.

## Source Map

| File | Purpose |
|------|---------|
| `framework/zero-core/src/tool.rs` | Canonical `Tool` and `Toolset` traits. |
| `framework/zero-core/src/policy.rs` | `ToolPermissions` and policy metadata. |
| `framework/zero-tool/src/registry.rs` | Framework registry implementing `Toolset`; not the live gateway registry. |
| `runtime/agent-runtime/src/tools/registry.rs` | Live executor tool registry. |
| `runtime/agent-runtime/src/executor.rs` | Tool schema building, tool-call loop, hooks, built-in dispatch, MCP dispatch, result offload. |
| `runtime/agent-runtime/src/mcp/manager.rs` | Runtime MCP server registry and tool execution. |
| `runtime/agent-tools/src/tools/mod.rs` | Built-in tool exports, settings, and factory helpers. |
| `gateway/gateway-execution/src/invoke/executor.rs` | Root/delegated live registry assembly and MCP manager startup. |
| `gateway/gateway-execution/src/runner/invoke_bootstrap.rs` | Wires fact store, knowledge graph store, ingestion, goal, ward usage, connector, and other adapters into `ExecutorBuilder`. |
| `gateway/gateway-services/src/settings.rs` | Persistent app settings with `tools: ToolSettings`. |
| `gateway/src/http/settings.rs` | Tool settings HTTP API. |
| `apps/ui/src/features/settings/WebSettingsPanel.tsx` | Current Settings UI surface for result offload only. |
