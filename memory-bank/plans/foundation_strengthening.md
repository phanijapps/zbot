# Foundation Strengthening Plan

## Executive Summary

This plan focuses on strengthening the core and application crates before architectural changes. The goal is to create a robust, secure, and performant foundation that can support both the current Tauri app and the future server-based architecture.

---

## What AgentZero Has That Moltbot Doesn't

| Capability | AgentZero | Moltbot | Advantage |
|------------|-----------|---------|-----------|
| **Rust Core** | ✅ Memory-safe, no GC | TypeScript | Performance, reliability |
| **Visual Workflows** | ✅ XY Flow IDE | No visual builder | User experience |
| **MCP Integration** | ✅ Native support | No MCP | Extensibility |
| **Knowledge Graph** | ✅ Entity/relationship tools | No KG | Persistent context |
| **Agent Isolation** | ✅ Per-agent directories | Shared workspace | Security |
| **Clean Traits** | ✅ Agent/Tool/Session | Ad-hoc interfaces | Extensibility |
| **Native Desktop** | ✅ Tauri | Electron optional | Performance |

---

## Current Foundation Analysis

### zero-core Strengths
```
✅ Clean trait hierarchy (Agent, Tool, Session, Context)
✅ Event-based architecture (immutable logs)
✅ State management with prefixes (user:, app:, temp:)
✅ FileSystem abstraction for portability
✅ Lifecycle callbacks (before/after agent)
```

### agent-tools Strengths
```
✅ Comprehensive file I/O (read, write, edit)
✅ Search tools (grep, glob)
✅ Execution tools (python, shell)
✅ Strong shell security (40+ blocked commands)
✅ Knowledge graph integration
✅ TODO management
✅ Skill loading
```

### Gaps to Address
```
❌ No native web fetch tool (HTTP requests)
❌ No persistent memory tool (key-value store)
❌ No native browser automation tool
❌ No scheduled task tool (cron)
❌ No tool-level permissions/policies
❌ No resource quotas
❌ No audit logging
❌ System prompt lacks moltbot-style guidance
```

---

## Phase 1: Core Trait Enhancements

### 1.1 Add Tool Policy Framework

**New file: `crates/zero-core/src/policy.rs`**

```rust
/// Tool risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRiskLevel {
    /// Safe operations (read-only, no side effects)
    Safe,
    /// Potentially risky (file writes, network requests)
    Moderate,
    /// Dangerous operations (shell execution, system changes)
    Dangerous,
    /// Requires explicit user approval
    Critical,
}

/// Tool permission requirements
#[derive(Debug, Clone)]
pub struct ToolPermissions {
    /// Risk level of this tool
    pub risk_level: ToolRiskLevel,
    /// Required capabilities (e.g., "filesystem:read", "network:http")
    pub requires: Vec<String>,
    /// Whether tool can be auto-approved
    pub auto_approve: bool,
    /// Maximum execution time (seconds)
    pub max_duration: Option<u64>,
    /// Maximum output size (bytes)
    pub max_output: Option<usize>,
}

/// Policy context passed to tools
#[derive(Debug, Clone)]
pub struct PolicyContext {
    /// Granted permissions for this session
    pub granted: HashSet<String>,
    /// Denied permissions
    pub denied: HashSet<String>,
    /// Resource limits
    pub limits: ResourceLimits,
}

/// Resource limits for execution
#[derive(Debug, Clone, Default)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<u8>,
    pub max_execution_time_secs: Option<u64>,
    pub max_output_bytes: Option<usize>,
    pub max_network_requests: Option<u32>,
}
```

### 1.2 Extend Tool Trait

**Modify: `crates/zero-core/src/tool.rs`**

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value>;
    fn response_schema(&self) -> Option<Value> { None }

    // NEW: Permission requirements
    fn permissions(&self) -> ToolPermissions {
        ToolPermissions::default()
    }

    // NEW: Pre-execution validation
    fn validate(&self, args: &Value) -> Result<(), ZeroError> {
        Ok(())
    }

    async fn execute(
        &self,
        ctx: Arc<dyn ToolContext>,
        args: Value,
    ) -> Result<Value, ZeroError>;
}
```

### 1.3 Add Audit Logging

**New file: `crates/zero-core/src/audit.rs`**

```rust
/// Audit event for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub action: AuditAction,
    pub arguments: Value,
    pub result: AuditResult,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    ToolCalled,
    ToolCompleted,
    ToolFailed,
    PermissionDenied,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    Success { output_size: usize },
    Error { message: String },
    Denied { reason: String },
}

/// Audit logger trait
#[async_trait]
pub trait AuditLogger: Send + Sync {
    async fn log(&self, event: AuditEvent);
    async fn query(&self, filter: AuditFilter) -> Vec<AuditEvent>;
}
```

---

## Phase 2: New Core Tools

### 2.1 WebFetchTool (HTTP Requests)

**New file: `application/agent-tools/src/tools/web.rs`**

```rust
/// Tool for making HTTP requests
pub struct WebFetchTool;

impl WebFetchTool {
    /// Security: Blocked domains (localhost, internal IPs, etc.)
    const BLOCKED_HOSTS: &[&str] = &[
        "localhost", "127.0.0.1", "0.0.0.0",
        "169.254.", "10.", "172.16.", "192.168.",
        "metadata.google", "169.254.169.254", // Cloud metadata
    ];

    /// Security: Max response size (10 MB)
    const MAX_RESPONSE_SIZE: usize = 10 * 1024 * 1024;

    /// Security: Request timeout
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }

    fn description(&self) -> &str {
        "Make HTTP requests to fetch web content. Supports GET, POST, PUT, DELETE methods."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE"],
                    "default": "GET"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional HTTP headers"
                },
                "body": {
                    "type": "string",
                    "description": "Optional request body for POST/PUT"
                },
                "timeout_secs": {
                    "type": "integer",
                    "default": 30,
                    "maximum": 120
                }
            },
            "required": ["url"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions {
            risk_level: ToolRiskLevel::Moderate,
            requires: vec!["network:http".into()],
            auto_approve: true,
            max_duration: Some(120),
            max_output: Some(10 * 1024 * 1024),
        }
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value, ZeroError> {
        // Implementation with security checks
    }
}
```

### 2.2 MemoryTool (Persistent Key-Value Store)

**New file: `application/agent-tools/src/tools/memory.rs`**

```rust
/// Tool for persistent memory across sessions
/// Stores data in agent's data directory as JSON files
pub struct MemoryTool {
    fs: Arc<dyn FileSystemContext>,
}

impl MemoryTool {
    /// Actions supported
    const ACTIONS: &[&str] = &["get", "set", "delete", "list", "search"];

    /// Max memory entries per agent
    const MAX_ENTRIES: usize = 1000;

    /// Max entry size (100 KB)
    const MAX_ENTRY_SIZE: usize = 100 * 1024;
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str { "memory" }

    fn description(&self) -> &str {
        "Persistent memory for storing facts, notes, and context across sessions. \
         Use to remember important information about users, projects, or decisions."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "set", "delete", "list", "search"],
                    "description": "The memory operation to perform"
                },
                "key": {
                    "type": "string",
                    "description": "Memory key (for get, set, delete)"
                },
                "value": {
                    "type": "string",
                    "description": "Value to store (for set)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for organization"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for search action)"
                }
            },
            "required": ["action"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions {
            risk_level: ToolRiskLevel::Safe,
            requires: vec!["filesystem:write".into()],
            auto_approve: true,
            max_duration: Some(5),
            max_output: Some(1024 * 1024),
        }
    }
}
```

### 2.3 CronTool (Scheduled Tasks)

**New file: `application/agent-tools/src/tools/cron.rs`**

```rust
/// Tool for scheduling recurring tasks
pub struct CronTool;

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str { "cron" }

    fn description(&self) -> &str {
        "Schedule recurring tasks. Tasks are stored and executed at specified intervals."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "delete", "pause", "resume"],
                    "description": "The cron operation"
                },
                "name": {
                    "type": "string",
                    "description": "Task name"
                },
                "schedule": {
                    "type": "string",
                    "description": "Cron expression (e.g., '0 9 * * *' for 9 AM daily)"
                },
                "task": {
                    "type": "string",
                    "description": "Task description for the agent to execute"
                },
                "task_id": {
                    "type": "string",
                    "description": "Task ID (for delete/pause/resume)"
                }
            },
            "required": ["action"]
        }))
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions {
            risk_level: ToolRiskLevel::Moderate,
            requires: vec!["scheduler:write".into()],
            auto_approve: false, // Requires user approval
            max_duration: Some(5),
            max_output: Some(10 * 1024),
        }
    }
}
```

### 2.4 BrowserTool (Native Automation)

**New file: `application/agent-tools/src/tools/browser.rs`**

For now, delegate to Playwright MCP. In future, can implement native via chromiumoxide or headless_chrome.

```rust
/// Tool for browser automation (wrapper for Playwright MCP)
pub struct BrowserTool {
    mcp_client: Option<Arc<dyn McpClient>>,
}

impl BrowserTool {
    /// Check if Playwright MCP is available
    pub fn is_available(&self) -> bool {
        self.mcp_client.is_some()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str { "browser" }

    fn description(&self) -> &str {
        "Control a web browser for automation. Can navigate, click, type, screenshot, and extract content."
    }

    fn permissions(&self) -> ToolPermissions {
        ToolPermissions {
            risk_level: ToolRiskLevel::Dangerous,
            requires: vec!["browser:automation".into()],
            auto_approve: false,
            max_duration: Some(300),
            max_output: Some(5 * 1024 * 1024),
        }
    }
}
```

---

## Phase 3: System Prompt Improvements

### 3.1 New System Prompt Structure

**Inspired by moltbot's guidelines:**

```markdown
## Your Role
{BASE_INSTRUCTIONS}

---

## Tool Call Guidelines

**Style:**
- Call tools IMMEDIATELY when needed. Don't narrate routine operations.
- Narrate only for: multi-step work, complex problems, sensitive actions.
- Tool names are case-sensitive. Call tools exactly as listed.
- Never use placeholders in tool arguments.

**Before answering about prior work:**
- Search memory first: `memory({ action: "search", query: "relevant terms" })`
- Check session history if memory doesn't have it

**Self-modifications:**
- NEVER modify your own configuration unless explicitly asked
- Skills and instructions are read-only

---

## Reasoning Format (When Enabled)

When extended thinking is enabled, structure your reasoning:

```
<think>
Internal analysis, step-by-step reasoning, alternatives considered...
</think>

<final>
User-visible response only
</final>
```

---

## Skills (CRITICAL)

{AVAILABLE_SKILLS}

**Workflow:**
1. User requests something → Check if a skill matches
2. If skill matches → IMMEDIATELY load it: `load_skill({ file: "@skill:skill-name" })`
3. Read skill instructions BEFORE proceeding
4. Follow skill guidance exactly

---

## Available Tools

### Built-in Tools
{AVAILABLE_TOOLS_XML}

### MCP Tools
{AVAILABLE_MCP_TOOLS_XML}

---

## Memory Usage

Use the `memory` tool to persist important information:

```
memory({ action: "set", key: "user_preferences", value: "...", tags: ["preferences"] })
memory({ action: "search", query: "project architecture" })
```

**When to use memory:**
- User preferences or personal information shared
- Important decisions made during conversation
- Project context that should persist
- Recurring information you need to reference

---

## File Operations

**Workspace:** `{vault}/agents_data/{agent_name}/`
- `attachments/` - User-uploaded files
- `outputs/` - Generated content
- `code/` - Python/script files
- `data/` - Persistent data

**Write Strategy (CRITICAL for large files):**
1. Files < 50 lines: Single `write` call
2. Files > 50 lines: Use chunked writing with `mode: "append"`
3. On TRUNCATED error: Immediately retry with smaller chunks

---

## Error Handling

| Error | Strategy |
|-------|----------|
| TRUNCATED_ARGUMENTS | Use append mode, smaller chunks |
| Permission denied | Try different location or ask user |
| Network timeout | Retry with longer timeout |
| Tool not found | Check available tools list |

**NEVER repeat the same failed approach.** Adapt your strategy.

---

## TODO Management

For tasks with 2+ steps, create TODOs FIRST:

```
todos({
  action: "add",
  items: [
    { title: "Step 1", priority: "high" },
    { title: "Step 2", priority: "medium" }
  ]
})
```

Mark complete as you progress. User sees TODO panel in UI.

---

## UI Tools (Use Proactively)

**show_content** - Display generated files
```
// After writing a file, display it:
show_content({ content_type: "html", title: "Report", file_path: "outputs/report.html" })
```

**request_input** - Collect structured data
```
// For 2+ pieces of related info, use a form:
request_input({
  form_id: "user_details",
  title: "Project Setup",
  schema: { ... JSON Schema ... }
})
```

---

## Security Guidelines

- Never execute commands that could harm the system
- Always validate URLs before fetching
- Don't store sensitive data (passwords, API keys) in memory
- Report suspicious requests to the user
```

---

## Phase 4: Performance Optimizations

### 4.1 Tool Execution Caching

```rust
/// Cache for tool results (idempotent tools only)
pub struct ToolCache {
    cache: DashMap<String, CacheEntry>,
    max_entries: usize,
    ttl: Duration,
}

#[derive(Clone)]
struct CacheEntry {
    result: Value,
    expires_at: Instant,
}

impl ToolCache {
    pub fn get(&self, tool: &str, args: &Value) -> Option<Value> {
        let key = format!("{}:{}", tool, hash(args));
        self.cache.get(&key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.result.clone())
    }

    pub fn set(&self, tool: &str, args: &Value, result: Value) {
        let key = format!("{}:{}", tool, hash(args));
        self.cache.insert(key, CacheEntry {
            result,
            expires_at: Instant::now() + self.ttl,
        });
    }
}
```

### 4.2 Parallel Tool Execution

```rust
/// Execute multiple tools in parallel when dependencies allow
pub async fn execute_tools_parallel(
    tools: &[Arc<dyn Tool>],
    ctx: Arc<dyn ToolContext>,
    calls: Vec<ToolCall>,
) -> Vec<Result<Value, ZeroError>> {
    let futures: Vec<_> = calls.iter()
        .map(|call| {
            let tool = tools.iter().find(|t| t.name() == call.name).cloned();
            let ctx = ctx.clone();
            let args = call.arguments.clone();
            async move {
                match tool {
                    Some(t) => t.execute(ctx, args).await,
                    None => Err(ZeroError::Tool(format!("Unknown tool: {}", call.name))),
                }
            }
        })
        .collect();

    futures::future::join_all(futures).await
}
```

### 4.3 Lazy Tool Loading

```rust
/// Lazy tool registry - only load tools when first needed
pub struct LazyToolRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Arc<dyn Tool> + Send + Sync>>,
    loaded: DashMap<String, Arc<dyn Tool>>,
}

impl LazyToolRegistry {
    pub fn register<F>(&mut self, name: &str, factory: F)
    where
        F: Fn() -> Arc<dyn Tool> + Send + Sync + 'static,
    {
        self.factories.insert(name.to_string(), Box::new(factory));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        if let Some(tool) = self.loaded.get(name) {
            return Some(tool.clone());
        }

        if let Some(factory) = self.factories.get(name) {
            let tool = factory();
            self.loaded.insert(name.to_string(), tool.clone());
            return Some(tool);
        }

        None
    }
}
```

---

## Phase 5: Security Hardening

### 5.1 Network Filtering

```rust
/// Network security policy
pub struct NetworkPolicy {
    /// Allowed URL patterns (regex)
    pub allowed_patterns: Vec<Regex>,
    /// Blocked URL patterns (takes precedence)
    pub blocked_patterns: Vec<Regex>,
    /// Rate limits per domain
    pub rate_limits: HashMap<String, RateLimit>,
}

impl NetworkPolicy {
    pub fn check_url(&self, url: &str) -> Result<(), PolicyError> {
        // Check blocked patterns first
        for pattern in &self.blocked_patterns {
            if pattern.is_match(url) {
                return Err(PolicyError::Blocked(url.to_string()));
            }
        }

        // If allowlist exists, URL must match
        if !self.allowed_patterns.is_empty() {
            let allowed = self.allowed_patterns.iter().any(|p| p.is_match(url));
            if !allowed {
                return Err(PolicyError::NotAllowed(url.to_string()));
            }
        }

        Ok(())
    }
}
```

### 5.2 Resource Quotas

```rust
/// Per-agent resource quotas
pub struct AgentQuota {
    pub agent_id: String,
    /// Current usage
    pub usage: ResourceUsage,
    /// Limits
    pub limits: ResourceLimits,
}

#[derive(Default)]
pub struct ResourceUsage {
    pub tool_calls: u64,
    pub network_requests: u64,
    pub bytes_written: u64,
    pub cpu_time_ms: u64,
}

impl AgentQuota {
    pub fn check(&self, resource: &str, amount: u64) -> Result<(), QuotaError> {
        match resource {
            "tool_calls" => {
                if let Some(limit) = self.limits.max_tool_calls {
                    if self.usage.tool_calls + amount > limit {
                        return Err(QuotaError::Exceeded("tool_calls"));
                    }
                }
            }
            // ... other resources
        }
        Ok(())
    }

    pub fn consume(&mut self, resource: &str, amount: u64) {
        match resource {
            "tool_calls" => self.usage.tool_calls += amount,
            "network_requests" => self.usage.network_requests += amount,
            "bytes_written" => self.usage.bytes_written += amount,
            "cpu_time_ms" => self.usage.cpu_time_ms += amount,
            _ => {}
        }
    }
}
```

---

## Implementation Order

### Week 1: Core Enhancements
1. Add `policy.rs` to zero-core
2. Add `audit.rs` to zero-core
3. Extend Tool trait with `permissions()` and `validate()`
4. Update existing tools with permission metadata

### Week 2: New Tools
1. Implement WebFetchTool
2. Implement MemoryTool
3. Implement CronTool (basic)
4. Add tool registration in `builtin_tools_with_fs()`

### Week 3: System Prompt & Performance
1. Rewrite system prompt with moltbot-inspired guidelines
2. Add tool execution caching
3. Add parallel tool execution
4. Add lazy tool loading

### Week 4: Security & Testing
1. Implement network filtering
2. Implement resource quotas
3. Add comprehensive tests
4. Documentation updates

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `crates/zero-core/src/policy.rs` | Tool permissions and policies |
| `crates/zero-core/src/audit.rs` | Audit logging |
| `application/agent-tools/src/tools/web.rs` | WebFetchTool |
| `application/agent-tools/src/tools/memory.rs` | MemoryTool |
| `application/agent-tools/src/tools/cron.rs` | CronTool |
| `application/agent-tools/src/tools/browser.rs` | BrowserTool wrapper |

### Modified Files
| File | Changes |
|------|---------|
| `crates/zero-core/src/lib.rs` | Export new modules |
| `crates/zero-core/src/tool.rs` | Extend Tool trait |
| `application/agent-tools/src/lib.rs` | Register new tools |
| `src-tauri/templates/system_prompt.md` | New prompt structure |

---

## Success Metrics

1. **Security:** All tools have explicit permission requirements
2. **Performance:** Tool caching reduces redundant calls by 50%+
3. **Robustness:** Comprehensive error handling with retries
4. **Observability:** Full audit trail for all tool executions
5. **Extensibility:** New tools can be added with <100 LOC

---

## Comparison: Before vs After

| Aspect | Before | After |
|--------|--------|-------|
| Tool permissions | None | Explicit per-tool |
| Audit logging | None | Full trace |
| HTTP requests | Via MCP only | Native WebFetchTool |
| Persistent memory | Via KG only | Dedicated MemoryTool |
| System prompt | Basic | Moltbot-inspired guidelines |
| Resource limits | Shell only | All tools |
| Network security | None | URL filtering, rate limits |
| Tool caching | None | Idempotent tool results |
