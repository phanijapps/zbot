# Specs

Active feature specs for AgentZero.

| Spec | Status | Summary |
| --- | --- | --- |
| [Tool Waste Visibility](tool-waste-visibility/spec.md) | Done | Makes blocked hooks, invalid tool arguments, planner skill drift, and tool durations visible in existing session telemetry. |
| [Mission Control Performance](mission-control-performance/spec.md) | Draft | Makes Mission Control load bounded summary data first, then lazy-load selected-session detail as the database grows. |
| [GitHub Release Installer](github-release-installer/spec.md) | Draft | Defines GitHub Release installers and artifact packaging for Linux, macOS, and Windows. |
| [Runtime Context Control](runtime-context-control/spec.md) | Draft | Consolidates live conversation compaction into runtime middleware while preserving `knowledge.db` durable memory. |
| [Memory Hygiene](memory-hygiene/spec.md) | Draft | Adds durable-memory guards for recall embedding input, handoff persistence, KG relationship integrity, and hygiene observability. |
| [Durable Ward Memory](durable-ward-memory/spec.md) | Draft | Defines Layer 4 as `knowledge.db` first-level indexing over durable executable ward workspaces, with preserved ward/file/artifact route hints. |
