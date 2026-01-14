# Agent Tools Implementation

## Overview

Core tools for AI agent execution via LangChain.js. Each tool extends `BaseTool` and converts to LangChain's `DynamicStructuredTool` format. Tools call Tauri commands → Rust backend for system access.

## Architecture

```
src/domains/agent-runtime/tools/
├── base/
│   └── BaseTool.ts          # Abstract base class
├── registry/
│   └── ToolRegistry.ts      # Singleton tool manager
├── impl/
│   ├── ReadTool.ts          # File reading
│   ├── WriteTool.ts         # File writing
│   ├── GrepTool.ts          # Regex search
│   ├── GlobTool.ts          # Pattern matching
│   ├── BashTool.ts          # Shell execution
│   └── PythonTool.ts        # Python code execution
└── index.ts                 # Exports all tools
```

## BaseTool Pattern

```typescript
export abstract class BaseTool {
  abstract readonly name: string;
  abstract readonly description: string;
  abstract readonly schema: z.ZodType<any>;

  abstract execute(inputs: any): Promise<string>;

  toLangChainTool(): DynamicStructuredTool {
    return new DynamicStructuredTool({
      name: this.name,
      description: this.description,
      schema: this.schema,
      func: async (inputs) => this.execute(inputs),
    });
  }
}
```

## Tool Registry

Singleton pattern for tool management:

```typescript
class ToolRegistry {
  private static instance: ToolRegistry;
  private tools: Map<string, BaseTool> = new Map();

  register(tool: BaseTool): void;
  getTool(name: string): BaseTool | undefined;
  getAllTools(): BaseTool[];
  getLangChainTools(): DynamicStructuredTool[];
}
```

## Cross-Platform Shell

| Platform | Shell | Arg | Fallback |
|----------|-------|-----|----------|
| Linux/macOS | bash | -c | sh |
| Windows | PowerShell | -Command | WSL bash |

Python tool uses venv at `~/.config/zeroagent/venv/`.

## Tools Specification

### ReadTool
- Reads file contents with optional offset/limit
- Returns file content as string
- Supports both text and binary (base64)

### WriteTool
- Writes content to files
- Creates parent directories if needed
- Returns success message

### GrepTool
- Regex search with context lines
- Returns matches with line numbers
- Supports case-insensitive flag

### GlobTool
- Finds files by pattern (*, **, etc.)
- Returns list of matching paths
- Sorted alphabetically

### BashTool
- Executes shell commands cross-platform
- Auto-detects available shell
- Returns stdout/stderr combined

### PythonTool
- Executes Python code in isolated venv
- Venv location: `~/.config/zeroagent/venv/`
- Captures stdout/stderr
- Returns execution result

## Tauri Commands (Rust)

```rust
#[tauri::command]
async fn execute_shell_command(command: String) -> Result<String, String>;

#[tauri::command]
async fn execute_python_code(code: String) -> Result<String, String>;
```

## Key Learnings

1. **LangChain runs in renderer**: No Node.js worker needed
2. **Tools are stateless**: Each call is independent
3. **Error handling**: Wrap all errors in user-friendly messages
4. **Shell detection**: Probe availability before using
5. **Python isolation**: Always use venv, never system Python
6. **Path handling**: Use Tauri's path API for cross-platform

## Usage Example

```typescript
import { ToolRegistry } from '@/domains/agent-runtime/tools';

// Register tools
const registry = ToolRegistry.getInstance();
registry.register(new ReadTool(tauriInvoke));
registry.register(new BashTool(tauriInvoke));

// Get LangChain tools for agent
const tools = registry.getLangChainTools();

// Create LangChain agent
const agent = await createAgent({
  llm: model,
  tools,
  prompt,
});
```
