# Specs

Active feature specs for AgentZero.

| Spec | Status | Summary |
| --- | --- | --- |
| [GitHub Release Installer](github-release-installer/spec.md) | Draft | Defines GitHub Release installers and artifact packaging for Linux, macOS, and Windows. |
| [Runtime Context Control](runtime-context-control/spec.md) | Draft | Consolidates live conversation compaction into runtime middleware while preserving `knowledge.db` durable memory. |
| [Memory Hygiene](memory-hygiene/spec.md) | Draft | Adds durable-memory guards for recall embedding input, handoff persistence, KG relationship integrity, and hygiene observability. |
| [Durable Ward Memory](durable-ward-memory/spec.md) | Draft | Defines Layer 4 as `knowledge.db` first-level indexing over durable executable ward workspaces, with preserved ward/file/artifact route hints. |
| [Subagent Capability Policy](subagent-role-gating/spec.md) | Implementing | Enforces root, executor, reviewer, and ward-agent tool capabilities with an explicit reviewer-agent identity. |
| [Builder Delegation Hygiene](builder-delegation-hygiene/spec.md) | Implementing | Adds delegation modes so builder-agent can distinguish direct artifacts, ward hygiene, ward-backed builds, and step execution. |
