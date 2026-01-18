# Working Scenarios

This document covers all working scenarios in AgentZero, providing step-by-step guidance for common user tasks.

## Table of Contents

1. [Agent Management](#agent-management)
2. [Provider Configuration](#provider-configuration)
3. [MCP Server Integration](#mcp-server-integration)
4. [Skills Management](#skills-management)
5. [Conversation Management](#conversation-management)
6. [Middleware Configuration](#middleware-configuration)

---

## Agent Management

### Scenario 1: Create a New Agent (Quick)

**Use case**: Quickly create a simple agent with basic configuration.

**Steps**:
1. Click **"+ New Agent"** button in agents list
2. Fill in the form:
   - **Display Name**: "My Research Agent"
   - **Description**: "Helps with research tasks"
   - **Provider**: Select from configured providers
   - **Model**: Select model (auto-populated from provider)
   - **Temperature**: Adjust slider (0.0 - 2.0)
   - **Max Tokens**: Set response limit
   - **Thinking Mode**: Enable for DeepSeek/GLM reasoning
3. Select **MCP Servers** (optional):
   - Toggle available servers on/off
4. Select **Skills** (optional):
   - Toggle available skills on/off
5. Click **"Create Agent"**

**Result**: New agent created and appears in agents list.

### Scenario 2: Create Agent with Advanced Configuration

**Use case**: Create agent with custom instructions and files.

**Steps**:
1. Click **"Open IDE"** button (or create new agent)
2. **Configure Metadata** (Configuration tab):
   - Fill in name, description, provider, model
   - Adjust temperature and max tokens
   - Select MCP servers and skills
3. **Write Instructions**:
   - Click **"AGENTS.md"** in file explorer
   - Write agent instructions in markdown editor
   - Changes auto-save every 500ms
4. **Add Files** (optional):
   - Click **"+ New Folder"** or **"Upload Files"**
   - Add reference materials, code snippets, etc.
5. Click **"Save Agent"** at top right

**Result**: Full-featured agent with custom instructions and files.

### Scenario 3: Edit Existing Agent

**Use case**: Update agent configuration or instructions.

**Steps**:
1. Find agent in agents list
2. Click **"Open IDE"** button
3. Make changes:
   - **config.yaml**: Edit metadata (Configuration tab)
   - **AGENTS.md**: Edit instructions
   - **Middleware tab**: Configure middleware
   - **Custom files**: Edit any file
4. Changes auto-save for existing agents
5. Close IDE when done

**Result**: Agent updated with changes saved.

### Scenario 4: Delete Agent

**Use case**: Remove an agent that's no longer needed.

**Steps**:
1. Find agent in agents list
2. Click **delete icon** (trash)
3. Confirm deletion

**Result**: Agent folder and all files removed from disk.

---

## Provider Configuration

### Scenario 5: Add OpenAI-Compatible Provider

**Use case**: Add a custom LLM provider (OpenAI, Azure, local model, etc.).

**Steps**:
1. Navigate to **Providers** page
2. Click **"+ Add Provider"**
3. Fill in the form:
   - **Name**: "My LLM Provider"
   - **Description**: "Custom OpenAI-compatible API"
   - **Base URL**: `https://api.openai.com/v1` (or custom)
   - **API Key**: Your API key
4. Click **"Add Provider"**

**Result**: Provider available for use in agents.

### Scenario 6: Add Local Model Provider

**Use case**: Use a local LLM (Ollama, LM Studio, etc.).

**Steps**:
1. Start local LLM server (e.g., `ollama serve`)
2. Navigate to **Providers** page
3. Click **"+ Add Provider"**
4. Fill in the form:
   - **Name**: "Local Models"
   - **Base URL**: `http://localhost:11434/v1` (Ollama)
   - **API Key**: `ollama` (placeholder, not used)
5. Click **"Add Provider"**
6. Edit provider to add models:
   - Click **edit icon** on provider
   - Add models: `llama3`, `mistral`, `codellama`, etc.

**Result**: Local models available for agent configuration.

---

## MCP Server Integration

### Scenario 7: Add Filesystem MCP Server

**Use case**: Allow agent to read/write files on your system.

**Steps**:
1. Navigate to **MCP Servers** page
2. Click **"+ Add Server"**
3. Fill in the form:
   - **Transport**: `stdio`
   - **Command**: `npx`
   - **Args**: `-y @modelcontextprotocol/server-filesystem /allowed/path`
4. Click **"Add Server"**
5. Enable for specific agent:
   - Open agent in IDE
   - Go to **Configuration** tab
   - Toggle **Filesystem** MCP on

**Result**: Agent can read/write files in allowed path.

### Scenario 8: Add Search MCP Server

**Use case**: Allow agent to search the web.

**Steps**:
1. Navigate to **MCP Servers** page
2. Click **"+ Add Server"**
3. Fill in the form:
   - **Transport**: `stdio`
   - **Command**: `npx`
   - **Args**: `-y @modelcontextprotocol/server-brave-search`
   - **Env**: `BRAVE_API_KEY=your_api_key`
4. Click **"Add Server"**
5. Enable for agent in agent configuration

**Result**: Agent can search the web using Brave Search API.

### Scenario 9: Add HTTP MCP Server

**Use case**: Connect to hosted MCP server.

**Steps**:
1. Navigate to **MCP Servers** page
2. Click **"+ Add Server"**
3. Fill in the form:
   - **Transport**: `http` (or `sse`)
   - **URL**: `https://mcp-server.example.com`
4. Click **"Add Server"**

**Result**: Connected to remote MCP server.

---

## Skills Management

### Scenario 10: Create Custom Skill

**Use case**: Create a reusable skill with parameters.

**Steps**:
1. Navigate to **Skills** page
2. Click **"+ New Skill"**
3. Fill in metadata:
   - **Name**: `weather` (URL-friendly)
   - **Display Name**: "Weather Lookup"
   - **Description**: "Get current weather for a location"
4. Define parameters (YAML frontmatter):
   ```yaml
   ---
   name: weather
   description: Get current weather for a location
   parameters:
     - name: location
       type: string
       description: City name or ZIP code
       required: true
   ---
   ```
5. Write skill instructions:
   ```markdown
   # Weather Lookup Skill

   Call the weather API to get current weather for the specified location.

   Example:
   - location: "San Francisco, CA"
   - location: "90210"

   Return temperature, conditions, and forecast in a concise format.
   ```
6. Click **"Save Skill"**

**Result**: Skill available for agents to use.

### Scenario 11: Add Skill to Agent

**Use case**: Enable a skill for a specific agent.

**Steps**:
1. Open agent in IDE
2. Go to **Configuration** tab
3. Find **Skills** section
4. Toggle desired skills on/off
5. Changes auto-save

**Result**: Agent can use selected skills in conversations.

---

## Conversation Management

### Scenario 12: Start New Conversation

**Use case**: Start chatting with an agent.

**Steps**:
1. Select agent from agents list
2. Type message in chat input
3. Press **Enter** or click send button
4. Watch response stream in real-time

**Result**: New conversation created with agent response.

### Scenario 13: Continue Existing Conversation

**Use case**: Resume a previous conversation.

**Steps**:
1. Click **Conversations** in sidebar
2. Find and select previous conversation
3. Type new message
4. Continue conversation with full history

**Result**: Conversation history preserved and continued.

### Scenario 14: Delete Conversation

**Use case**: Remove a conversation.

**Steps**:
1. Click **Conversations** in sidebar
2. Find conversation to delete
3. Click delete icon
4. Confirm deletion

**Result**: Conversation and all messages removed.

---

## Middleware Configuration

### Scenario 15: Enable Summarization Middleware

**Use case**: Automatically compress long conversations to fit in context window.

**Steps**:
1. Open agent in IDE
2. Click on **config.yaml** in file explorer
3. Click **Middleware** tab
4. Edit middleware YAML:
   ```yaml
   middleware:
     summarization:
       enabled: true
       trigger:
         tokens: 60000        # Trigger at 60k tokens
       keep:
         messages: 6          # Keep 6 recent messages
   ```
5. Changes auto-save

**Result**: Long conversations automatically summarized when approaching limit.

**What happens**:
- When conversation reaches 60,000 tokens
- Middleware compresses all but 6 most recent messages
- Summary injected as system message
- Recent messages kept intact
- Event emitted: `[Summarized 24 messages into 456 characters]`

### Scenario 16: Enable Context Editing Middleware

**Use case**: Clear old tool results to free up tokens.

**Steps**:
1. Open agent in IDE
2. Click on **config.yaml** in file explorer
3. Click **Middleware** tab
4. Edit middleware YAML:
   ```yaml
   middleware:
     context_editing:
       enabled: true
       trigger_tokens: 60000
       keep_tool_results: 10
       min_reclaim: 1000
   ```
5. Changes auto-save

**Result**: Old tool results automatically cleared when token limit approached.

**What happens**:
- When conversation reaches 60,000 tokens
- Keeps 10 most recent tool results
- Clears older results with placeholder text
- Event emitted: `[Cleared 15 tool results (reclaimed ~18234 tokens)]`

### Scenario 17: Use Custom Model for Summarization

**Use case**: Use cheaper/faster model for summarization while main agent uses premium model.

**Steps**:
1. Add two providers:
   - Main provider: `gpt-4o` (premium)
   - Summary provider: `gpt-3.5-turbo` (cheaper)
2. Configure agent with main provider
3. Edit middleware YAML:
   ```yaml
   middleware:
     summarization:
       enabled: true
       provider: gpt-3.5-turbo    # Override provider
       model: gpt-3.5-turbo       # Override model
       trigger:
         tokens: 60000
       keep:
         messages: 6
   ```

**Result**: Agent uses GPT-4o for responses, but GPT-3.5-turbo for summarization (cheaper).

### Scenario 18: Exclude Specific Tools from Context Editing

**Use case**: Always preserve search results, but clear other tool outputs.

**Steps**:
1. Open agent in IDE
2. Click on **config.yaml** in file explorer
3. Click **Middleware** tab
4. Edit middleware YAML:
   ```yaml
   middleware:
     context_editing:
       enabled: true
       trigger_tokens: 60000
       keep_tool_results: 5
       exclude_tools:          # Never clear these
         - search
         - database_lookup
   ```

**Result**: Search and database results always preserved, other tools cleared.

### Scenario 19: Combined Middleware Setup

**Use case**: Use both summarization and context editing together.

**Steps**:
1. Open agent in IDE
2. Click on **config.yaml** in file explorer
3. Click **Middleware** tab
4. Edit middleware YAML:
   ```yaml
   middleware:
     # Compress conversation history
     summarization:
       enabled: true
       trigger:
         tokens: 50000
       keep:
         messages: 6

     # Clear old tool results
     context_editing:
       enabled: true
       trigger_tokens: 50000
       keep_tool_results: 10
       exclude_tools:
         - search
   ```

**Result**: Both middlewares work together:
- Summarization runs first (compresses chat history)
- Context editing runs second (clears old tool results)
- Search results always preserved

---

## Advanced Scenarios

### Scenario 20: Agent with File Upload

**Use case**: Agent needs reference documents.

**Steps**:
1. Open agent in IDE
2. In file explorer, click **"Upload Files"**
3. Select documents (PDF, images, text files)
4. Files appear in file explorer
5. Reference in instructions:
   ```markdown
   You have access to the following reference files:
   - assets/specs.pdf (product specifications)
   - assets/pricing.csv (price list)
   ```

**Result**: Agent can access uploaded files through file explorer.

### Scenario 21: Debug Agent with Thinking Mode

**Use case**: See agent's reasoning process (DeepSeek, GLM models).

**Steps**:
1. Configure agent with:
   - **Provider**: DeepSeek or GLM
   - **Thinking Mode**: Enabled
2. Start conversation
3. Watch for `<think>` tags in response
4. Reasoning process visible in real-time

**Result**: Agent's thinking process displayed in chat.

### Scenario 22: Test Agent Different Configuration

**Use case**: Compare agent behavior with different temperatures.

**Steps**:
1. Create first agent:
   - Name: "Creative Agent"
   - Temperature: 1.5 (high creativity)
2. Clone to second agent:
   - Name: "Precise Agent"
   - Temperature: 0.2 (more focused)
3. Test both with same prompt
4. Compare responses

**Result**: See how temperature affects agent behavior.

---

## Troubleshooting

### Issue: Agent Not Responding

**Symptoms**: No response after sending message.

**Solutions**:
1. Check provider configuration:
   - Verify API key is correct
   - Test base URL in browser
   - Check model name spelling
2. Check agent config:
   - Verify provider is selected
   - Verify model is selected
3. Check console/logs for errors

### Issue: MCP Tools Not Available

**Symptoms**: Agent doesn't use MCP tools.

**Solutions**:
1. Verify MCP server is added
2. Verify MCP server is enabled for agent
3. Check MCP server is running (stdio/sse)
4. Check MCP server URL (http)
5. Look for MCP errors in logs

### Issue: Middleware Not Running

**Symptoms**: No middleware events in conversation.

**Solutions**:
1. Verify middleware YAML is valid
2. Check `enabled: true` is set
3. Verify trigger conditions are met (token count)
4. Check for YAML syntax errors
5. Look for middleware errors in logs

### Issue: File Upload Fails

**Symptoms**: Cannot upload files to agent.

**Solutions**:
1. Check file size (may be too large)
2. Check file type (binary files supported)
3. Verify agent folder exists
4. Check disk space
5. Look for file system errors in logs

---

## Best Practices

### Agent Design

1. **Clear Instructions**: Be specific about agent's role and capabilities
2. **Structured Prompts**: Use markdown formatting for clarity
3. **Temperature Selection**:
   - 0.0 - 0.3: Factual, precise tasks
   - 0.4 - 0.7: Balanced responses (default)
   - 0.8 - 2.0: Creative tasks

### Middleware Configuration

1. **Summarization**:
   - Set trigger at 70-80% of context window
   - Keep 5-10 recent messages for continuity
   - Use custom summary prompt for domain-specific needs

2. **Context Editing**:
   - Always preserve critical tools (search, database)
   - Set min_reclaim to avoid unnecessary clearing
   - Adjust keep_tool_results based on usage patterns

### Performance

1. **Model Selection**:
   - Use cheaper models for summarization
   - Use local models for privacy/cost savings
   - Reserve premium models for complex tasks

2. **Token Management**:
   - Enable middleware for long conversations
   - Monitor token usage in logs
   - Adjust triggers based on typical usage

---

## Related Documentation

| Document | Description |
|----------|-------------|
| `ARCHITECTURE.md` | System architecture and technical details |
| `src/commands/AGENTS.md` | Agent commands implementation |
| `src/domains/agent_runtime/middleware/AGENTS.md` | Middleware system documentation |
| `LOGGING.md` | Logging guidelines and best practices |
