# Specs

Active feature specs for AgentZero.

| Spec | Status | Summary |
| --- | --- | --- |
| [YFinance Market Analysis Skill Consolidation](yfinance-market-analysis-skill/spec.md) | Shipped | Consolidates bundled yfinance workflows into one primary skill while keeping old `yf-*` IDs as compatibility wrappers. |
| [Tool Waste Visibility](tool-waste-visibility/spec.md) | Done | Makes blocked hooks, invalid tool arguments, planner skill drift, and tool durations visible in existing session telemetry. |
| [Mission Control Performance](mission-control-performance/spec.md) | Draft | Makes Mission Control load bounded summary data first, then lazy-load selected-session detail as the database grows. |
| [GitHub Release Installer](github-release-installer/spec.md) | Draft | Defines GitHub Release installers and artifact packaging for Linux, macOS, and Windows. |
| [Release On Main](release-on-main/spec.md) | Implementing | Automates the daily CalVer release bump and tag when changes land on `main`, while preserving the manual release script. |
| [Agent Handoff Notes](agent-handoff-notes/spec.md) | Implementing | Adds current-session agent discovery and one-way handoff notes over existing steering without implementing full Pattern 4 peer messaging. |
| [Runtime Context Control](runtime-context-control/spec.md) | Draft | Consolidates live conversation compaction into runtime middleware while preserving `knowledge.db` durable memory. |
| [Rig Engine Migration](rig-engine-migration/spec.md) | Implementing | Replaces the active `zero-*` framework/runtime engine with a Rig-backed execution facade while preserving gateway/UI, config, memory, and parity contracts. |
| [Memory Hygiene](memory-hygiene/spec.md) | Draft | Adds durable-memory guards for recall embedding input, handoff persistence, KG relationship integrity, and hygiene observability. |
| [Durable Ward Memory](durable-ward-memory/spec.md) | Draft | Defines Layer 4 as `knowledge.db` first-level indexing over durable executable ward workspaces, with preserved ward/file/artifact route hints. |
| [Vault Ward Browser](vault-ward-browser/spec.md) | Shipped | Adds a read-only Vault tab for browsing ward filesystem trees and previewing common files through bounded local-only APIs. |
| [Ward Vault In Research](ward-vault-in-research/spec.md) | Implementing | Embeds a read-only ward-scoped Vault explorer/search pane inside Research after a session has an active ward. |
| [Simplified Provider Model Configuration](simplified-provider-model-configuration/spec.md) | Implemented | Replaces broad model metadata maintenance with 200k input / 32k output defaults plus agent and Advanced overrides. |
| [MCP OAuth](mcp-oauth/spec.md) | Implementing | Adds OAuth metadata, authorization flow endpoints, token storage, and runtime bearer injection for protected remote MCP servers. |
| [Subagent Capability Policy](subagent-role-gating/spec.md) | Implementing | Enforces root, executor, reviewer, and ward-agent tool capabilities with an explicit reviewer-agent identity. |
| [Builder Delegation Hygiene](builder-delegation-hygiene/spec.md) | Implementing | Adds delegation modes so builder-agent can distinguish direct artifacts, ward hygiene, ward-backed builds, and step execution. |
