# Logging Guidelines

This document covers logging practices for AgentZero, focusing on minimal console logs on the UI and proper logging in Rust.

## Table of Contents

1. [Principles](#principles)
2. [Frontend Logging](#frontend-logging)
3. [Backend Logging](#backend-logging)
4. [Middleware Logging](#middleware-logging)
5. [Error Logging](#error-logging)
6. [Debug Logging](#debug-logging)

---

## Principles

### 1. Be Minimal

**Rule**: Log only what's necessary for users and developers.

**Examples**:
- ✅ Log: Agent created, Error saving file, Middleware ran
- ❌ Don't log: Component mounted, State updated, Hook called

**Rationale**: Too much log noise makes it hard to find important information.

### 2. Be Structured

**Rule**: Use consistent log formats and levels.

**Frontend**:
```typescript
// ✅ Good: Structured log with context
console.log('[AgentService] Created agent:', agent.name);
console.error('[AgentService] Failed to save:', error.message);

// ❌ Bad: Unstructured log
console.log('Agent created!');
console.log('Error:', error);
```

**Backend (Rust)**:
```rust
// ✅ Good: Structured log with module prefix
eprintln!("[agents] Created agent: {}", name);
eprintln!("[agents] Failed to save: {}", error);

// ❌ Bad: Unstructured log
eprintln!("Created agent!");
eprintln!("Error: {:?}", error);
```

### 3. Be Actionable

**Rule**: Logs should indicate what happened and what to do.

**Examples**:
- ✅ "Failed to connect to MCP server: Connection refused. Check if server is running."
- ❌ "Error connecting to MCP"

### 4. Use Appropriate Levels

| Level | When to Use | Example |
|-------|-------------|---------|
| `Error` | Operation failed, needs attention | "Failed to create agent: Permission denied" |
| `Warn` | Unexpected but recoverable | "MCP server not responding, using cache" |
| `Info` | Important state changes | "Agent created successfully" |
| `Debug` | Diagnostic info (development only) | "Middleware triggered: 60000 tokens" |

---

## Frontend Logging

### Console Log Guidelines

**Principle**: Minimize console logs in production. Only log errors and important state changes.

### When to Log

#### ✅ DO Log These Events

1. **User Actions** (important ones):
```typescript
console.log('[AgentService] Creating agent:', displayName);
console.log('[Conversation] Started new conversation with agent:', agentId);
```

2. **API Errors**:
```typescript
console.error('[AgentService] Failed to load agents:', error);
console.error('[MCP] Server connection failed:', serverId, error);
```

3. **Middleware Activity** (single summary event):
```typescript
console.log('[Middleware] Summarization: Compressed 24 messages into 456 characters');
console.log('[Middleware] Context Editing: Cleared 15 tool results (reclaimed ~18234 tokens)');
```

4. **File Operations** (results, not every step):
```typescript
console.log('[AgentFile] Uploaded 3 files to agent:', agentId);
console.error('[AgentFile] Failed to save file:', filename, error);
```

#### ❌ DON'T Log These Events

1. **Component Lifecycle**:
```typescript
// ❌ Don't log
console.log('[AgentList] Component mounted');
console.log('[AgentList] Rendering agents');
useEffect(() => {
  console.log('[AgentList] Agents updated');  // Too noisy
}, [agents]);
```

2. **State Updates**:
```typescript
// ❌ Don't log
console.log('[AgentForm] Name updated:', name);
console.log('[AgentForm] Temperature changed:', temperature);
```

3. **Every Function Call**:
```typescript
// ❌ Don't log
const handleSubmit = () => {
  console.log('[AgentForm] handleSubmit called');  // Redundant
  // ...
};
```

4. **Hook Triggers**:
```typescript
// ❌ Don't log
useEffect(() => {
  console.log('[Agent] Loading agent data...');  // Use loading indicator instead
}, []);
```

### Error Handling Pattern

**Principle**: Always log errors with context and actionable messages.

```typescript
// ✅ Good: Error with context
try {
  await agentService.createAgent(agent);
  console.log('[AgentService] Agent created successfully:', agent.displayName);
} catch (error) {
  console.error('[AgentService] Failed to create agent:', {
    name: agent.displayName,
    error: error instanceof Error ? error.message : String(error),
    action: 'Check if agent name already exists'
  });
  // Show user-friendly error toast
  showToast(`Failed to create agent: ${error.message}`, 'error');
}

// ❌ Bad: Error without context
try {
  await agentService.createAgent(agent);
} catch (error) {
  console.error('Error:', error);  // What operation? What agent?
}
```

### Service Layer Logging

**Principle**: Services should log their operations at entry points.

```typescript
// services/agent.ts
export async function createAgent(agent: Omit<Agent, "id" | "createdAt">): Promise<Agent> {
  console.log('[AgentService] Creating agent:', agent.displayName);

  try {
    const result = await invoke('create_agent', { agent });
    console.log('[AgentService] Agent created:', result.name);
    return result;
  } catch (error) {
    console.error('[AgentService] Create agent failed:', {
      name: agent.displayName,
      error: error instanceof Error ? error.message : String(error)
    });
    throw error;
  }
}
```

**Note**: Don't log inside helper functions, only at service boundaries.

---

## Backend Logging

### Rust Logging Guidelines

**Principle**: Use `eprintln!` for all logging (stdout is used for Tauri IPC).

### When to Log

#### ✅ DO Log These Events

1. **Command Entry/Exit** (for important commands):
```rust
#[tauri::command]
pub async fn create_agent(agent: Agent) -> Result<Agent, String> {
    eprintln!("[agents] Creating agent: {}", agent.display_name);

    // ... implementation ...

    eprintln!("[agents] Agent created successfully: {}", agent.name);
    Ok(created_agent)
}
```

2. **Errors** (always with context):
```rust
let config_path = agent_dir.join("config.yaml");
if !config_path.exists() {
    return Err(format!(
        "[agents] config.yaml not found for agent: {}. Expected at: {:?}",
        agent_id, agent_dir
    ));
}
```

3. **File Operations** (results):
```rust
fs::write(&config_path, config_yaml)
    .map_err(|e| format!("[agents] Failed to write config.yaml to {:?}: {}", config_path, e))?;
eprintln!("[agents] Wrote config.yaml for agent: {}", agent_id);
```

4. **MCP Operations** (connection state):
```rust
eprintln!("[mcp] Connecting to server: {} ({})", server.name, server.transport);
eprintln!("[mcp] Connected to {}, {} tools available", server.name, tool_count);
```

#### ❌ DON'T Log These Events

1. **Every Function Call**:
```rust
// ❌ Don't log
fn get_agent_id(agent: &Agent) -> &str {
    eprintln!("Getting agent ID");  // Redundant
    &agent.id
}
```

2. **Loop Iterations**:
```rust
// ❌ Don't log
for entry in entries {
    eprintln!("Processing entry: {:?}", entry.path());  // Too noisy
}
```

3. **Successful Reads** (unless diagnostic):
```rust
// ❌ Don't log (expected operation)
let content = fs::read_to_string(&path)?;
eprintln!("Read file: {:?}", path);  // Not needed

// ✅ OK for diagnostics in development
#[cfg(debug_assertions)]
eprintln!("[agents] Read AGENTS.md: {} bytes", content.len());
```

### Error Pattern

**Principle**: Include module prefix, operation, error, and context.

```rust
// ✅ Good: Structured error
pub async fn read_agent_folder(agent_dir: &PathBuf) -> Result<Agent, String> {
    let config_path = agent_dir.join("config.yaml");

    if !config_path.exists() {
        return Err(format!(
            "[agents] Agent config not found at {:?}",
            config_path
        ));
    }

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!(
            "[agents] Failed to read config.yaml from {:?}: {}",
            config_path, e
        ))?;

    // ...
}

// ❌ Bad: Unstructured error
pub async fn read_agent_folder(agent_dir: &PathBuf) -> Result<Agent, String> {
    let config_path = agent_dir.join("config.yaml");

    if !config_path.exists() {
        return Err("Config not found".to_string());  // Where?
    }

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Error: {}", e))?;  // What file?
}
```

### Module Prefix Convention

Use consistent module prefixes for easy filtering:

| Module | Prefix | Examples |
|--------|--------|----------|
| Agents | `[agents]` | Creating agent, Updating agent, Deleting agent |
| Providers | `[providers]` | Adding provider, Testing connection |
| MCP | `[mcp]` | Connecting to server, Tool execution |
| Skills | `[skills]` | Loading skill, Parsing frontmatter |
| Conversations | `[conversations]` | Creating conversation, Saving message |
| Executor | `[executor]` | Starting execution, Tool call |
| Middleware | `[middleware]` | Summarization, Context editing |
| LLM | `[llm]` | API request, Stream response |

---

## Middleware Logging

### Principle

**Rule**: Middleware should emit a single summary event, not verbose logs.

### ✅ Good Middleware Logging

```rust
// Summarization middleware
async fn process(&self, messages: Vec<ChatMessage>, _context: &MiddlewareContext) -> Result<MiddlewareEffect, String> {
    // ... check trigger ...

    // ... generate summary ...

    // Single event with summary
    let event = StreamEvent::Token {
        timestamp: now,
        content: format!(
            "[Previous conversation summary:]\n[Summarized {} messages into {} characters]",
            to_summarize.len(),
            summary.len()
        ),
    };

    Ok(MiddlewareEffect::EmitAndModify {
        event,
        messages: new_messages,
    })
}

// Context editing middleware
async fn process(&self, messages: Vec<ChatMessage>, _context: &MiddlewareContext) -> Result<MiddlewareEffect, String> {
    // ... find and clear tool results ...

    // Single event with summary
    let event = StreamEvent::Token {
        timestamp: now,
        content: format!(
            "[Cleared {} tool results (reclaimed ~{} tokens)]",
            indices_to_clear.len(),
            tokens_to_reclaim
        ),
    };

    Ok(MiddlewareEffect::EmitAndModify {
        event,
        messages: modified_messages,
    })
}
```

### ❌ Bad Middleware Logging

```rust
// ❌ Verbose logging (don't do this)
async fn process(&self, messages: Vec<ChatMessage>, context: &MiddlewareContext) -> Result<MiddlewareEffect, String> {
    eprintln!("[middleware] Summarization process called");
    eprintln!("[middleware] Token count: {}", estimate_tokens(&messages));

    if should_trigger {
        eprintln!("[middleware] Trigger condition met!");
        eprintln!("[middleware] Splitting messages...");
        eprintln!("[middleware] Keeping {} messages", keep_count);
        eprintln!("[middleware] Summarizing {} messages", to_summarize.len());
        eprintln!("[middleware] Calling LLM for summary...");
        // ...
    }

    eprintln!("[middleware] Process complete");
}
```

### Frontend Display

Middleware events appear in the UI as token events:

```
User: Can you help me with Rust?

Assistant: [Previous conversation summary:]

Summary of previous conversation:
User asked about Python memory management...
[Summarized 24 messages into 456 characters]

User: Can you help me with Rust?
```

**Note**: The event content is visible to users, so keep it concise and informative.

---

## Error Logging

### Frontend Error Handling

**Pattern**: Try-catch with context logging and user notification.

```typescript
// ✅ Good: Complete error handling
async function handleSaveAgent(agent: Agent) {
  console.log('[AgentService] Saving agent:', agent.displayName);

  try {
    await agentService.updateAgent(agent.id, agent);
    console.log('[AgentService] Agent saved successfully');
    showToast('Agent saved successfully', 'success');
  } catch (error) {
    console.error('[AgentService] Failed to save agent:', {
      id: agent.id,
      name: agent.displayName,
      error: error instanceof Error ? error.message : String(error)
    });
    showToast(`Failed to save: ${error.message}`, 'error');
  }
}

// ❌ Bad: Incomplete error handling
async function handleSaveAgent(agent: Agent) {
  try {
    await agentService.updateAgent(agent.id, agent);
  } catch (error) {
    console.error(error);  // No context, no user notification
  }
}
```

### Backend Error Handling

**Pattern**: Use `?` operator with `map_err` for descriptive errors.

```rust
// ✅ Good: Descriptive errors
pub async fn write_agent_file(
    agent_id: String,
    file_path: String,
    content: String,
) -> Result<(), String> {
    let agent_dir = get_agents_dir()?.join(&agent_id);
    let full_path = agent_dir.join(&file_path);

    // Ensure parent directory exists
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!(
                "[agents] Failed to create directory {:?}: {}",
                parent, e
            ))?;
    }

    // Write file
    fs::write(&full_path, content)
        .map_err(|e| format!(
            "[agents] Failed to write file {:?}: {}",
            full_path, e
        ))?;

    eprintln!("[agents] Wrote file: agent={}, path={}", agent_id, file_path);
    Ok(())
}

// ❌ Bad: Generic errors
pub async fn write_agent_file(
    agent_id: String,
    file_path: String,
    content: String,
) -> Result<(), String> {
    let full_path = get_agents_dir()?.join(&agent_id).join(&file_path);

    fs::create_dir_all(full_path.parent().unwrap())?;  // No error context
    fs::write(&full_path, content)?;  // No error context
    Ok(())
}
```

---

## Debug Logging

### Development-Only Logging

**Principle**: Use conditional compilation for debug-only logs.

#### Frontend (TypeScript)

```typescript
// ✅ Good: Debug-only logging
const DEBUG = import.meta.env.DEV;

function debugLog(category: string, message: string, data?: any) {
  if (DEBUG) {
    console.log(`[${category}] ${message}`, data ?? '');
  }
}

// Usage
debugLog('AgentService', 'Loading agents...');
debugLog('AgentService', 'Agents loaded:', agents);

// ❌ Bad: Always logs in production
console.log('[DEBUG] Loading agents...');  // Pollutes production console
```

#### Backend (Rust)

```rust
// ✅ Good: Debug-only logging
#[cfg(debug_assertions)]
eprintln!("[agents] Processing agent file: {:?}", file_path);

// or

if cfg!(debug_assertions) {
    eprintln!("[agents] Token estimate: {}", token_count);
}

// ❌ Bad: Always logs
eprintln!("[DEBUG] Processing file: {:?}", file_path);  // Pollutes logs
```

### Diagnostic Commands

**For debugging specific issues**, add verbose logging to specific commands:

```rust
#[tauri::command)]
pub async fn debug_agent_tokens(agent_id: String) -> Result<usize, String> {
    eprintln!("[debug] Calculating tokens for agent: {}", agent_id);

    let agent = get_agent(&agent_id)?;
    let messages = load_conversation_messages(&agent_id)?;

    let token_count = estimate_total_tokens(&messages);

    eprintln!("[debug] Agent: {}, Messages: {}, Tokens: {}",
        agent.display_name, messages.len(), token_count
    );

    Ok(token_count)
}
```

---

## Log Levels Reference

### When to Use Each Level

| Level | Frontend | Backend | Use Case |
|-------|----------|---------|----------|
| **Error** | `console.error()` | `eprintln!("[module] Error: ...")` | Operation failed, user action needed |
| **Warn** | `console.warn()` | `eprintln!("[module] Warning: ...")` | Unexpected but recovered |
| **Info** | `console.log()` | `eprintln!("[module] ...")` | Important state changes |
| **Debug** | Conditional | `#[cfg(debug_assertions)]` | Diagnostic info only |

### Examples

```typescript
// Error: Operation failed
console.error('[AgentService] Failed to create agent:', {
  name: displayName,
  error: error.message,
  suggestion: 'Check if agent name already exists'
});

// Warning: Unexpected but OK
console.warn('[MCP] Server slow to respond, using cached tools:', serverId);

// Info: Important state change
console.log('[AgentService] Agent created:', agent.name);

// Debug: Diagnostic only
if (DEBUG) {
  console.log('[AgentService] Fetched providers:', providers.length);
}
```

```rust
// Error: Operation failed
eprintln!("[agents] Failed to write config.yaml: {}", error);

// Warning: Unexpected but OK
eprintln!("[llm] API rate limit approaching, slowing requests");

// Info: Important state change
eprintln!("[agents] Agent created: {}", agent_name);

// Debug: Diagnostic only
#[cfg(debug_assertions)]
eprintln!("[agents] Loaded {} agents from disk", agents.len());
```

---

## Testing Logs

### Verifying Logs in Development

1. **Frontend**: Open browser DevTools Console
   - Should see minimal logs (errors, important events)
   - No component lifecycle noise
   - No state update spam

2. **Backend**: Run Tauri dev mode
   ```bash
   cargo tauri dev
   ```
   - Check terminal for Rust logs
   - Should see structured logs with module prefixes
   - No verbose debug spam

### Log Filtering

**For filtering logs by module**:

```bash
# Frontend: Filter by module in console
console.log('[AgentService]', ...);  // Filter: "[AgentService]"

# Backend: Filter by module in terminal
cargo tauri dev 2>&1 | grep "\[agents\]"    # Only agent logs
cargo tauri dev 2>&1 | grep "\[mcp\]"      # Only MCP logs
cargo tauri dev 2>&1 | grep -v "\[debug\]" # Exclude debug logs
```

---

## Best Practices Summary

### DO ✅

1. **Log important events**: Agent created, file saved, error occurred
2. **Use structured logs**: Module prefix, operation, context
3. **Emit single summary events**: For middleware operations
4. **Log errors with context**: What, where, why, how to fix
5. **Use appropriate levels**: Error, Warn, Info, Debug
6. **Keep it minimal**: Only log what's necessary
7. **Make logs actionable**: Include what to do next

### DON'T ❌

1. **Log component lifecycle**: Mounted, updated, unmounted
2. **Log every function call**: Entry/exit logs
3. **Log state updates**: Every useState change
4. **Log loop iterations**: Every item processed
5. **Use unstructured logs**: "Error:", "Done:", etc.
6. **Log expected operations**: Successful file reads without context
7. **Duplicate logs**: Log same event at multiple layers

---

## Related Documentation

| Document | Description |
|----------|-------------|
| `ARCHITECTURE.md` | System architecture and module structure |
| `WORKING_SCENARIOS.md` | User-facing scenarios and troubleshooting |
| `src/domains/agent_runtime/middleware/AGENTS.md` | Middleware logging patterns |
| `src/commands/AGENTS.md` | Agent command logging examples |
