# Mission Control — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the slide-out chat with a full-page Mission Control — visible intent reasoning, session titles, subagent output schemas, intelligence feed sidebar, rich message blocks, and file attachments.

**Architecture:** Phase 1 (Backend): remove intent analysis middleware, add set_session_title tool, add output_schema to delegations, add file upload endpoint, add first-turn instruction shard. Phase 2 (UI): build 12 focused React components replacing WebChatPanel.tsx, with real-time WebSocket event mapping.

**Tech Stack:** Rust (axum, rusqlite, serde), React 19 + TypeScript, existing WebSocket transport, SVG-free (no charts — pure HTML/CSS blocks)

**Spec:** `docs/superpowers/specs/2026-03-30-mission-control-design.md`

---

## File Structure

### New Files (Rust — Phase 1)
| File | Responsibility |
|---|---|
| `runtime/agent-tools/src/tools/session_title.rs` | `set_session_title` tool |
| `gateway/src/http/upload.rs` | `POST /api/upload` file upload endpoint |
| `gateway/templates/shards/first_turn_protocol.md` | Agent instruction shard for first-turn behavior |

### Modified Files (Rust — Phase 1)
| File | Change |
|---|---|
| `runtime/agent-tools/src/tools/delegate.rs` | Add `output_schema` parameter |
| `gateway/gateway-execution/src/delegation/spawn.rs` | Inject schema into child prompt |
| `gateway/gateway-execution/src/delegation/callback.rs` | Validate response against schema |
| `gateway/gateway-execution/src/runner.rs` | Remove intent analysis call, register session_title tool |
| `gateway/gateway-events/src/lib.rs` | Add `SessionTitleChanged` event |
| `gateway/src/http/mod.rs` | Register upload route |
| `gateway/src/state.rs` | Wire session_title tool |

### New Files (TypeScript — Phase 2)
| File | Responsibility |
|---|---|
| `features/chat/MissionControl.tsx` | Page layout: session bar + narrative + sidebar + input |
| `features/chat/SessionBar.tsx` | Title, agent, status, metrics, controls |
| `features/chat/ExecutionNarrative.tsx` | Scrollable block list, renders by type |
| `features/chat/UserMessage.tsx` | User avatar, text, attachments |
| `features/chat/RecallBlock.tsx` | Purple recall card from memory.recall result |
| `features/chat/ToolExecutionBlock.tsx` | Expandable dark tool card |
| `features/chat/DelegationBlock.tsx` | Green delegation card with live stats |
| `features/chat/PlanBlock.tsx` | Checklist from update_plan |
| `features/chat/AgentResponse.tsx` | Agent avatar, markdown response |
| `features/chat/IntelligenceFeed.tsx` | Right sidebar: ward + facts + subagents + plan |
| `features/chat/ChatInput.tsx` | Text input + attachment buttons + upload |
| `features/chat/mission-hooks.ts` | WebSocket events, message parsing, state |

### Modified Files (TypeScript — Phase 2)
| File | Change |
|---|---|
| `features/agent/WebChatPanel.tsx` | Redirect to MissionControl |
| `styles/components.css` | Add mission control CSS classes |

---

## Phase 1: Backend

### Task 1: SessionTitleChanged Event

**Files:**
- Modify: `gateway/gateway-events/src/lib.rs`

- [ ] **Step 1: Read the GatewayEvent enum**

Find the `GatewayEvent` enum in `gateway/gateway-events/src/lib.rs`. Understand the existing variants and their structure.

- [ ] **Step 2: Add SessionTitleChanged variant**

```rust
SessionTitleChanged {
    session_id: String,
    title: String,
},
```

Follow the pattern of existing variants for serialization.

- [ ] **Step 3: Verify**

Run: `cargo check --workspace`

- [ ] **Step 4: Commit**

```bash
git add gateway/gateway-events/src/lib.rs
git commit -m "feat(events): add SessionTitleChanged WebSocket event"
```

---

### Task 2: set_session_title Tool

**Files:**
- Create: `runtime/agent-tools/src/tools/session_title.rs`
- Modify: `runtime/agent-tools/src/tools/mod.rs` or wherever tools are registered
- Modify: `gateway/gateway-execution/src/runner.rs` — register the tool

- [ ] **Step 1: Read existing tool implementations**

Read `runtime/agent-tools/src/tools/` — understand the Tool trait, how tools are registered, how they access session state and event bus. Look at a simple tool (like `update_plan` or `ward`) for the pattern.

- [ ] **Step 2: Implement set_session_title tool**

The tool:
- Input: `{ "title": "string" }` — concise title (2-8 words)
- Execution: update `sessions.title` via the state service, emit `SessionTitleChanged` event
- Output: `"Session title set to: {title}"`

Find how to access `session_id` from the tool execution context. Check how other tools (like `update_plan`) access session state.

- [ ] **Step 3: Register the tool**

Add to the tool registry in the runner/executor setup. It should be an always-on tool (available in every session).

- [ ] **Step 4: Add update_session_title to state service**

In `services/execution-state/` or wherever session CRUD lives, add:

```rust
pub fn update_session_title(&self, session_id: &str, title: &str) -> Result<(), String> {
    // UPDATE sessions SET title = ?1 WHERE id = ?2
}
```

- [ ] **Step 5: Verify**

Run: `cargo check --workspace`
Run: `cargo test --workspace --lib --bins --tests 2>&1 | grep FAILED`

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-tools/src/tools/session_title.rs runtime/agent-tools/src/tools/mod.rs gateway/gateway-execution/src/runner.rs services/execution-state/
git commit -m "feat(tools): add set_session_title — agent names its own sessions"
```

---

### Task 3: Subagent Output Schemas

**Files:**
- Modify: `runtime/agent-tools/src/tools/delegate.rs`
- Modify: `gateway/gateway-execution/src/delegation/spawn.rs`
- Modify: `gateway/gateway-execution/src/delegation/callback.rs`

- [ ] **Step 1: Read delegation code**

Read all three files to understand:
- How `delegate_to_agent` tool parameters are defined
- How the child agent is spawned in `spawn.rs`
- How the child's response comes back in `callback.rs`

- [ ] **Step 2: Add output_schema parameter to delegate tool**

In `delegate.rs`, add optional `output_schema` to the tool's parameter schema:

```json
"output_schema": {
  "type": "object",
  "description": "Optional JSON Schema the child agent must follow in its response"
}
```

Parse it as `Option<serde_json::Value>` from the tool input.

- [ ] **Step 3: Inject schema into child agent prompt**

In `spawn.rs`, when creating the child agent's system prompt, if `output_schema` is provided:

```rust
if let Some(schema) = &output_schema {
    let schema_str = serde_json::to_string_pretty(schema).unwrap_or_default();
    child_instructions.push_str(&format!(
        "\n\n## Output Contract\nYour response MUST be a JSON object matching this schema:\n```json\n{}\n```\nRespond with ONLY the JSON object. No explanation before or after.",
        schema_str
    ));
}
```

Pass `output_schema` through the delegation request struct.

- [ ] **Step 4: Validate response in callback**

In `callback.rs`, when the child's response arrives, if `output_schema` was provided:

```rust
fn validate_delegation_response(response: &str, schema: &Option<serde_json::Value>) -> String {
    if schema.is_none() {
        return response.to_string(); // No schema — pass through
    }
    // Try parsing as JSON
    match serde_json::from_str::<serde_json::Value>(response) {
        Ok(json) => {
            // Valid JSON — pass through (full JSON Schema validation is future work)
            serde_json::to_string(&json).unwrap_or_else(|_| response.to_string())
        }
        Err(_) => {
            // Not JSON — wrap as summary
            let wrapped = serde_json::json!({
                "summary": response,
                "_schema_valid": false
            });
            serde_json::to_string(&wrapped).unwrap_or_else(|_| response.to_string())
        }
    }
}
```

- [ ] **Step 5: Verify**

Run: `cargo check --workspace`

- [ ] **Step 6: Commit**

```bash
git add runtime/agent-tools/src/tools/delegate.rs gateway/gateway-execution/src/delegation/spawn.rs gateway/gateway-execution/src/delegation/callback.rs
git commit -m "feat(delegation): add output_schema — structured responses from subagents"
```

---

### Task 4: Remove Intent Analysis Middleware + First-Turn Shard

**Files:**
- Modify: `gateway/gateway-execution/src/runner.rs`
- Modify: `gateway/gateway-execution/src/middleware/intent_analysis.rs`
- Create: `gateway/templates/shards/first_turn_protocol.md`

- [ ] **Step 1: Read intent analysis integration**

In `runner.rs`, find where `analyze_intent` is called (in `create_executor()`). Understand what it does to the agent's instructions.

- [ ] **Step 2: Remove intent analysis LLM call**

In `runner.rs` `create_executor()`, remove the block that calls `analyze_intent()` and `inject_intent_context()`. Keep the `index_resources()` call if it exists separately, or move it to daemon startup.

The agent will now start without pre-analyzed intent. Its instructions (via the new shard) tell it to recall + plan on first turn.

- [ ] **Step 3: Keep resource indexing**

If `index_resources()` is called inside the intent analysis block, extract it and call it separately — either:
- At daemon startup in `state.rs` (preferred — runs once)
- Or at session start before executor creation (current location, just without the LLM call)

Resource indexing is fast (<100ms, no LLM) — it just upserts skill/agent/ward descriptions into memory_facts.

- [ ] **Step 4: Create first-turn instruction shard**

Create `gateway/templates/shards/first_turn_protocol.md`:

```markdown
## First Turn Protocol
On every new task from the user:
1. Call the memory tool to recall relevant knowledge — corrections, past strategies, domain context, available skills and agents
2. Call set_session_title with a concise title (2-8 words) describing the task
3. Switch to the appropriate ward if needed (based on recalled ward knowledge)
4. Call update_plan with your execution steps
5. Begin execution

When analyzing the user's request, consider:
- What they explicitly asked for
- What they would implicitly expect (save results, update wards, follow established patterns)
- Which subagents would be best suited for specialized work
- What corrections from past sessions apply
```

- [ ] **Step 5: Verify the shard is loaded**

Check `gateway/gateway-templates/` to understand how shards are assembled into the system prompt. Ensure the new shard is included. Read the shard loading code to verify new files are auto-discovered or need registration.

- [ ] **Step 6: Verify**

Run: `cargo check --workspace`
Run: `cargo test --package gateway-execution -- --nocapture`

- [ ] **Step 7: Commit**

```bash
git add gateway/gateway-execution/src/runner.rs gateway/gateway-execution/src/middleware/intent_analysis.rs gateway/templates/shards/first_turn_protocol.md
git commit -m "refactor(intent): replace middleware LLM call with first-turn protocol shard"
```

---

### Task 5: File Upload Endpoint

**Files:**
- Create: `gateway/src/http/upload.rs`
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Read existing HTTP handler patterns**

Read `gateway/src/http/sessions.rs` or `gateway/src/http/graph.rs` for the Axum handler pattern with `State(state): State<AppState>`.

- [ ] **Step 2: Implement upload endpoint**

```rust
// POST /api/upload
// Accepts multipart form data with a "file" field
// Stores in {vault_data}/uploads/{uuid}.{ext}
// Returns JSON: { id, filename, mime_type, size, path }
```

Use `axum::extract::Multipart` for file upload handling. Generate UUID for storage. Preserve original extension. Create uploads directory if needed.

Check if `axum` multipart feature is enabled in Cargo.toml — if not, add it.

- [ ] **Step 3: Register route**

In `mod.rs`:
```rust
.route("/api/upload", post(upload::upload_file))
```

- [ ] **Step 4: Verify**

Run: `cargo check --workspace`

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/upload.rs gateway/src/http/mod.rs
git commit -m "feat(api): add POST /api/upload — file upload for chat attachments"
```

---

## Phase 2: UI — Mission Control

### Task 6: CSS Classes + Simple Components

**Files:**
- Modify: `apps/ui/src/styles/components.css`
- Create: `apps/ui/src/features/chat/UserMessage.tsx`
- Create: `apps/ui/src/features/chat/AgentResponse.tsx`
- Create: `apps/ui/src/features/chat/PlanBlock.tsx`

- [ ] **Step 1: Add Mission Control CSS to components.css**

Read `apps/ui/ARCHITECTURE.md` first. Add BEM classes:

```css
/* Mission Control */
.mission-control { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
.mission-control__session-bar { display: flex; align-items: center; padding: var(--spacing-2) var(--spacing-4); border-bottom: 1px solid var(--border); background: var(--card); gap: var(--spacing-3); flex-shrink: 0; }
.mission-control__body { display: flex; flex: 1; overflow: hidden; }
.mission-control__narrative { flex: 1; overflow-y: auto; padding: var(--spacing-4) var(--spacing-5); display: flex; flex-direction: column; gap: var(--spacing-4); }
.mission-control__sidebar { width: 260px; border-left: 1px solid var(--border); background: var(--card); overflow-y: auto; flex-shrink: 0; }
.mission-control__input { border-top: 1px solid var(--border); padding: var(--spacing-3) var(--spacing-4); display: flex; align-items: center; gap: var(--spacing-3); background: var(--card); flex-shrink: 0; }

/* Session Bar */
.session-bar__title { font-weight: 600; font-size: var(--text-sm); }
.session-bar__badge { font-size: var(--text-xs); color: var(--muted-foreground); background: var(--muted); padding: 1px 8px; border-radius: var(--radius-full); }
.session-bar__metric { font-size: var(--text-xs); color: var(--muted-foreground); font-family: var(--font-mono); }

/* Message blocks */
.msg-block { display: flex; gap: var(--spacing-3); }
.msg-block__avatar { width: 28px; height: 28px; border-radius: var(--radius-full); display: flex; align-items: center; justify-content: center; flex-shrink: 0; font-size: var(--text-xs); font-weight: 600; }
.msg-block__avatar--user { background: var(--primary); color: var(--primary-foreground); }
.msg-block__avatar--agent { background: var(--muted); color: var(--muted-foreground); }
.msg-block__time { font-size: 9px; color: var(--muted-foreground); margin-bottom: var(--spacing-1); }
.msg-block__content { font-size: var(--text-sm); line-height: 1.6; }

/* Recall block */
.recall-block { margin-left: 40px; background: rgba(139, 92, 246, 0.05); border: 1px solid rgba(139, 92, 246, 0.12); border-radius: var(--radius-md); padding: var(--spacing-2-5) var(--spacing-3); font-size: var(--text-xs); }
.recall-block__header { display: flex; align-items: center; gap: var(--spacing-2); margin-bottom: var(--spacing-2); color: #8b5cf6; font-weight: 600; }
.recall-block__correction { color: var(--destructive); }
.recall-block__episode { color: var(--muted-foreground); }

/* Tool execution block */
.tool-block { margin-left: 40px; background: var(--muted); border: 1px solid var(--border); border-radius: var(--radius-md); overflow: hidden; }
.tool-block__header { display: flex; align-items: center; padding: var(--spacing-2) var(--spacing-3); gap: var(--spacing-2); cursor: pointer; font-size: var(--text-xs); }
.tool-block__header:hover { background: rgba(255,255,255,0.03); }
.tool-block__name { font-weight: 600; color: var(--warning); }
.tool-block__summary { color: var(--muted-foreground); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tool-block__duration { color: var(--muted-foreground); font-family: var(--font-mono); }
.tool-block__status--success { color: var(--success); }
.tool-block__status--error { color: var(--destructive); }
.tool-block__body { padding: 0 var(--spacing-3) var(--spacing-3) calc(var(--spacing-3) + 18px); font-family: var(--font-mono); font-size: 10px; color: var(--muted-foreground); line-height: 1.6; white-space: pre-wrap; }
.tool-block--error { border-color: var(--destructive); }

/* Delegation block */
.delegation-block { margin-left: 40px; background: rgba(16, 185, 129, 0.04); border: 1px solid rgba(16, 185, 129, 0.12); border-radius: var(--radius-md); padding: var(--spacing-2-5) var(--spacing-3); }
.delegation-block__header { display: flex; align-items: center; gap: var(--spacing-2); margin-bottom: var(--spacing-1-5); color: var(--success); font-weight: 600; font-size: var(--text-xs); }
.delegation-block__task { font-size: var(--text-xs); color: var(--muted-foreground); line-height: 1.5; }
.delegation-block__stats { display: flex; gap: var(--spacing-3); font-size: 10px; color: var(--muted-foreground); margin-top: var(--spacing-2); }

/* Plan block */
.plan-block { margin-left: 40px; border: 1px solid var(--border); border-radius: var(--radius-md); padding: var(--spacing-2-5) var(--spacing-3); }
.plan-block__step { display: flex; gap: var(--spacing-2); align-items: center; padding: var(--spacing-1) 0; font-size: var(--text-xs); }
.plan-block__step--done { text-decoration: line-through; color: var(--muted-foreground); }

/* Intelligence Feed sections */
.intel-section { padding: var(--spacing-3) var(--spacing-3-5); border-bottom: 1px solid var(--border); }
.intel-section__title { font-size: 9px; text-transform: uppercase; letter-spacing: 0.5px; color: var(--muted-foreground); margin-bottom: var(--spacing-2); font-weight: 600; }

/* Chat input */
.chat-input__field { flex: 1; background: var(--muted); border: 1px solid var(--border); border-radius: var(--radius-md); padding: var(--spacing-2) var(--spacing-3); font-size: var(--text-sm); color: var(--foreground); outline: none; resize: none; min-height: 36px; max-height: 120px; }
.chat-input__field:focus { border-color: var(--primary); }
.chat-input__send { background: var(--primary); color: var(--primary-foreground); border: none; border-radius: var(--radius-md); padding: var(--spacing-2) var(--spacing-4); font-size: var(--text-xs); font-weight: 500; cursor: pointer; }
.chat-input__attach { background: none; border: none; color: var(--muted-foreground); cursor: pointer; font-size: 16px; padding: var(--spacing-1); }
.chat-input__chips { display: flex; gap: var(--spacing-2); padding-bottom: var(--spacing-2); }
.chat-input__chip { background: var(--muted); padding: 2px 8px; border-radius: var(--radius-sm); font-size: var(--text-xs); display: flex; align-items: center; gap: var(--spacing-1); }
```

- [ ] **Step 2: Create UserMessage, AgentResponse, PlanBlock**

Three simple components following the CSS classes above. Read the transport types for message structure.

- [ ] **Step 3: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add -f apps/ui/src/styles/components.css apps/ui/src/features/chat/
git commit -m "feat(ui): add Mission Control CSS + UserMessage, AgentResponse, PlanBlock"
```

---

### Task 7: RecallBlock + ToolExecutionBlock + DelegationBlock

**Files:**
- Create: `apps/ui/src/features/chat/RecallBlock.tsx`
- Create: `apps/ui/src/features/chat/ToolExecutionBlock.tsx`
- Create: `apps/ui/src/features/chat/DelegationBlock.tsx`

- [ ] **Step 1: RecallBlock**

Parses the `memory.recall` tool result JSON. Renders:
- Header: "🧠 Memory Recall — N facts, M episodes"
- Corrections (category=correction) as red warning text
- Episodes with outcome badges
- Domain facts below
- Collapsible if > 5 items

- [ ] **Step 2: ToolExecutionBlock**

Expandable block for tool calls (shell, read, write, edit, etc.):
- Header (always visible): tool name (amber), command summary, duration, status ✓/✗
- Body (collapsed by default): full input + output, monospace
- Click header to expand/collapse
- Error state: red border, error visible in header

- [ ] **Step 3: DelegationBlock**

Green-bordered block for delegations:
- Header: agent name + "delegating" / "completed"
- Task description
- Live stats: tool call count, token count, duration (update via WebSocket events)
- Completed: show result summary (or structured JSON if output_schema)

- [ ] **Step 4: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 5: Commit**

```bash
git add -f apps/ui/src/features/chat/RecallBlock.tsx apps/ui/src/features/chat/ToolExecutionBlock.tsx apps/ui/src/features/chat/DelegationBlock.tsx
git commit -m "feat(ui): add RecallBlock, ToolExecutionBlock, DelegationBlock"
```

---

### Task 8: IntelligenceFeed + SessionBar + ChatInput

**Files:**
- Create: `apps/ui/src/features/chat/IntelligenceFeed.tsx`
- Create: `apps/ui/src/features/chat/SessionBar.tsx`
- Create: `apps/ui/src/features/chat/ChatInput.tsx`

- [ ] **Step 1: IntelligenceFeed**

Right sidebar with 4 sections:
- **Active Ward:** ward name + content excerpt. Updated when ward tool fires.
- **Recalled Facts:** mirrors RecallBlock content. Corrections pinned at top.
- **Subagents:** agent name + status dot + task. From delegation events.
- **Execution Plan:** checklist from update_plan. Synced with PlanBlock.

All sections use `.intel-section` CSS class.

State comes from props — parent component manages the data.

- [ ] **Step 2: SessionBar**

Top bar showing: status dot, title, agent badge, session ID, token count, duration, model, stop button.

Title updates from `SessionTitleChanged` WebSocket event. Token/duration update from `Token` events.

- [ ] **Step 3: ChatInput**

Bottom bar: attachment buttons (📎 file, 🖼️ image), textarea, send button.

File upload: calls `POST /api/upload`, shows chip with filename while uploading, includes attachment refs in message. Image upload: same flow.

Read how the current `WebChatPanel.tsx` sends messages (via `transport.invoke()` or WebSocket). Follow the same pattern.

- [ ] **Step 4: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 5: Commit**

```bash
git add -f apps/ui/src/features/chat/IntelligenceFeed.tsx apps/ui/src/features/chat/SessionBar.tsx apps/ui/src/features/chat/ChatInput.tsx
git commit -m "feat(ui): add IntelligenceFeed sidebar, SessionBar, ChatInput with attachments"
```

---

### Task 9: mission-hooks + ExecutionNarrative

**Files:**
- Create: `apps/ui/src/features/chat/mission-hooks.ts`
- Create: `apps/ui/src/features/chat/ExecutionNarrative.tsx`

- [ ] **Step 1: mission-hooks.ts**

Read `apps/ui/src/features/agent/WebChatPanel.tsx` thoroughly — understand how it:
- Connects to WebSocket
- Receives streaming events (Token, ToolCall, ToolResult, etc.)
- Builds message list from events
- Handles delegation lifecycle

Create hooks that provide the same functionality but with richer message typing:

```typescript
interface NarrativeBlock {
  id: string;
  type: 'user' | 'recall' | 'tool' | 'delegation' | 'plan' | 'response';
  timestamp: string;
  data: any; // typed per block type
  isStreaming?: boolean;
}

export function useMissionEvents(conversationId: string) {
  // Subscribe to WebSocket events
  // Build NarrativeBlock[] from events
  // Return { blocks, sessionTitle, status, tokenCount, duration, subagents, plan, recalledFacts, activeWard }
}
```

Tool name determines block type:
- `memory` (action: recall) → 'recall'
- `delegate_to_agent` → 'delegation'
- `update_plan` → 'plan'
- `set_session_title` → updates sessionTitle (no block)
- `respond` → 'response'
- Everything else → 'tool'

- [ ] **Step 2: ExecutionNarrative**

Scrollable list that renders NarrativeBlock[] by type:

```typescript
function renderBlock(block: NarrativeBlock) {
  switch (block.type) {
    case 'user': return <UserMessage {...} />;
    case 'recall': return <RecallBlock {...} />;
    case 'tool': return <ToolExecutionBlock {...} />;
    case 'delegation': return <DelegationBlock {...} />;
    case 'plan': return <PlanBlock {...} />;
    case 'response': return <AgentResponse {...} />;
  }
}
```

Auto-scrolls to bottom on new blocks. Preserves scroll position when user has scrolled up.

- [ ] **Step 3: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add -f apps/ui/src/features/chat/mission-hooks.ts apps/ui/src/features/chat/ExecutionNarrative.tsx
git commit -m "feat(ui): add mission-hooks (WebSocket event mapping) + ExecutionNarrative"
```

---

### Task 10: MissionControl Main Page + Wire Up

**Files:**
- Create: `apps/ui/src/features/chat/MissionControl.tsx`
- Modify: `apps/ui/src/features/agent/WebChatPanel.tsx`

- [ ] **Step 1: MissionControl.tsx**

The main page component that composes everything:

```typescript
export function MissionControl() {
  const { conversationId, agentId } = useSessionParams(); // from URL or state

  const {
    blocks, sessionTitle, status, tokenCount, duration,
    subagents, plan, recalledFacts, activeWard
  } = useMissionEvents(conversationId);

  return (
    <div className="mission-control">
      <SessionBar
        title={sessionTitle}
        agentId={agentId}
        status={status}
        tokenCount={tokenCount}
        duration={duration}
      />
      <div className="mission-control__body">
        <ExecutionNarrative blocks={blocks} />
        <IntelligenceFeed
          ward={activeWard}
          facts={recalledFacts}
          subagents={subagents}
          plan={plan}
        />
      </div>
      <ChatInput
        onSend={(message, attachments) => { /* invoke agent */ }}
        disabled={status === 'running'}
      />
    </div>
  );
}
```

- [ ] **Step 2: Replace WebChatPanel**

In `WebChatPanel.tsx`, replace the component body:

```typescript
import { MissionControl } from '../chat/MissionControl';

export function WebChatPanel(props) {
  return <MissionControl />;
}
```

Preserve the export name so existing imports work.

- [ ] **Step 3: Build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add -f apps/ui/src/features/chat/MissionControl.tsx apps/ui/src/features/agent/WebChatPanel.tsx
git commit -m "feat(ui): Mission Control — full-page execution theater replaces chat slide-out"
```

---

### Task 11: End-to-End Verification

- [ ] **Step 1: Full Rust build + tests**

Run: `cargo test --workspace --lib --bins --tests`

- [ ] **Step 2: Full UI build**

Run: `cd apps/ui && npm run build`

- [ ] **Step 3: Manual smoke test — session title**

Start daemon. Send a message. Verify the agent calls `set_session_title` and the session bar updates.

- [ ] **Step 4: Manual smoke test — first-turn protocol**

Verify the agent calls `memory.recall` → `set_session_title` → `ward` → `update_plan` on its first turn. All visible as blocks.

- [ ] **Step 5: Manual smoke test — delegation with schema**

Send a task that triggers delegation. Verify the delegation block shows live stats and structured result (if schema provided).

- [ ] **Step 6: Manual smoke test — intelligence feed**

Verify right sidebar shows: recalled facts, active subagents, execution plan. All updating in real-time.

- [ ] **Step 7: Commit + Push**

```bash
git add -A
git commit -m "feat: Mission Control complete — execution theater with full observability"
git push origin feat/model-capabilities-registry
```
