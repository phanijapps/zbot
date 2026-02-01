# Simplify Tools & Improve Delegation

## Problem
1. **Too many tools (24)** - Overwhelming for LLM, increases token usage
2. **No agent discovery** - Agent can't list available subagents
3. **delegate_to_agent exists** but agent doesn't know WHO to delegate to

---

## Plan

### Part 1: Add `list_agents` Tool

Create a new tool that returns available agents with their capabilities.

**File:** `application/agent-tools/src/tools/agent.rs`

```rust
pub struct ListAgentsTool;

impl Tool for ListAgentsTool {
    fn name(&self) -> &str { "list_agents" }

    fn description(&self) -> &str {
        "List available agents you can delegate tasks to. Returns agent IDs, names, and descriptions."
    }

    // Returns: [{ id, name, description }]
}
```

**Implementation:**
- Read from cached agent list (passed via ToolContext state)
- Runner caches agents at executor creation time
- No network call needed during tool execution

---

### Part 2: Refactor `delegate_to_agent` Response

Update existing tool to return structured response:

**Current response:**
```json
{
  "status": "delegated",
  "child_conversation_id": "...",
  ...
}
```

**New response:**
```json
{
  "convid": "<child_conversation_id>",
  "status": "delegated",
  "message": "Task delegated to research-agent"
}
```

When subagent completes, callback includes:
```json
{
  "convid": "<child_conversation_id>",
  "response": "<subagent's final response>"
}
```

---

### Part 3: Default vs Optional Tools

**Default Tools (10 core - enabled for all agents):**
| Tool | Purpose |
|------|---------|
| `shell` | Run any command (can use for grep, find, python) |
| `read` | Read files |
| `write` | Write files |
| `edit` | Edit files |
| `memory` | Persist/recall information |
| `web_fetch` | Fetch web content |
| `respond` | Send message to user |
| `delegate_to_agent` | Delegate to subagent |
| `list_agents` | Discover available agents |
| `todo` | Track task progress |

**Optional Tools (configurable in Settings > Advanced):**
| Tool | Category | Description |
|------|----------|-------------|
| `grep` | Search | Regex search in files |
| `glob` | Search | Find files by pattern |
| `python` | Execution | Run Python scripts |
| `load_skill` | Skills | Load skill instructions |
| `request_input` | UI | Request user input |
| `show_content` | UI | Display content to user |
| `list_entities` | Knowledge Graph | List graph entities |
| `search_entities` | Knowledge Graph | Search entities |
| `get_entity_relationships` | Knowledge Graph | Get relationships |
| `add_entity` | Knowledge Graph | Add entity |
| `add_relationship` | Knowledge Graph | Add relationship |
| `create_agent` | Agent | Create new agent |
| `list_skills` | Introspection | List available skills |
| `list_tools` | Introspection | List available tools |
| `list_mcps` | Introspection | List MCP servers |

**Settings UI (Advanced Options):**
```
┌─────────────────────────────────────────────────────┐
│  Settings > Advanced > Tools                        │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Default tools are always enabled (10 tools)        │
│                                                     │
│  ▼ Additional Tools                                 │
│                                                     │
│  Search Tools                                       │
│  ☐ grep - Regex search in files                    │
│  ☐ glob - Find files by pattern                    │
│                                                     │
│  Execution Tools                                    │
│  ☐ python - Run Python scripts                     │
│  ☐ load_skill - Load skill instructions            │
│                                                     │
│  UI Tools                                           │
│  ☐ request_input - Request user input              │
│  ☐ show_content - Display content to user          │
│                                                     │
│  Knowledge Graph Tools                              │
│  ☐ Enable all (5 tools)                            │
│                                                     │
│  Introspection Tools                                │
│  ☐ list_skills, list_tools, list_mcps              │
│                                                     │
│  Agent Tools                                        │
│  ☐ create_agent - Create new agents                │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

### Part 4: Implementation Steps

#### Step 1: Create `list_agents` tool
- Add `ListAgentsTool` to `agent.rs`
- Cache agent list in runner, pass to ToolContext state
- Register tool in default tools

#### Step 2: Update delegation response format
- Modify `DelegateTool` response structure
- Update callback message format in `handle_subagent_completion`

#### Step 3: Create tool configuration system
- Add `ToolSettings` struct to store enabled optional tools
- Store in `{config_dir}/settings.json`
- Create functions: `core_tools()` and `optional_tools(settings)`

#### Step 4: Update runner to use tool settings
- Load tool settings when creating executor
- Register core tools + enabled optional tools
- Pass to executor

#### Step 5: Add Settings UI for tools
- Create `WebToolSettingsPanel.tsx` component
- Add to Settings page under "Advanced" section
- Checkboxes grouped by category
- Save to backend via API

#### Step 6: Add API endpoint for tool settings
- `GET /api/settings/tools` - Get current tool settings
- `PUT /api/settings/tools` - Update tool settings

---

## Data Structures

**Tool Settings (settings.json):**
```json
{
  "tools": {
    "optional": {
      "grep": false,
      "glob": false,
      "python": false,
      "load_skill": true,
      "request_input": false,
      "show_content": false,
      "knowledge_graph": false,
      "create_agent": false,
      "introspection": false
    }
  }
}
```

**Rust struct:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolSettings {
    pub grep: bool,
    pub glob: bool,
    pub python: bool,
    pub load_skill: bool,
    pub request_input: bool,
    pub show_content: bool,
    pub knowledge_graph: bool,  // enables all 5 KG tools
    pub create_agent: bool,
    pub introspection: bool,    // enables list_skills, list_tools, list_mcps
}
```

---

## File Changes

| File | Action |
|------|--------|
| `application/agent-tools/src/tools/agent.rs` | Add ListAgentsTool |
| `application/agent-tools/src/tools/mod.rs` | Split into core_tools() + optional_tools() |
| `application/agent-runtime/src/tools/delegate.rs` | Update response format |
| `application/gateway/src/execution/runner.rs` | Load settings, register tools |
| `application/gateway/src/execution/delegation.rs` | Update callback format |
| `application/gateway/src/services/settings.rs` | NEW - Settings service |
| `application/gateway/src/http/settings.rs` | NEW - Settings API endpoints |
| `src/features/settings/WebToolSettingsPanel.tsx` | NEW - UI component |

---

## Tool Count Summary

| Category | Count | Tools |
|----------|-------|-------|
| Core (always enabled) | 10 | shell, read, write, edit, memory, web_fetch, respond, delegate_to_agent, list_agents, todo |
| Optional (configurable) | 15 | grep, glob, python, load_skill, request_input, show_content, 5x KG, create_agent, 3x introspection |
| **Total available** | **25** | All tools preserved |
