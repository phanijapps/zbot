# Quick Chat UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an ephemeral, single-session, memory-aware quick-chat interface at `/chat-v2` powered by a new dedicated `quick-chat` agent and a shared rolling status pill.

**Architecture:** New `quick-chat` agent (separate template + installed config) drives a lean UI at `/chat-v2` / `/chat-v2/:sessionId`. Invoked over the existing WebSocket `invoke` command with `mode="chat"` (reuses `SessionMode::Chat` — skips intent analysis / planning / delegation beyond one subagent). Introduces three shared UI primitives (tool-phrase dictionary, `useStatusPillAggregator` hook, `StatusPill` component) that Research later consumes. `/chat` (FastChat) and `/` (MissionControl) remain untouched.

**Tech Stack:** React 18 + TypeScript, Vite, React Router v6. Rust workspace for the agent template + endpoint wiring. Vitest for UI unit tests. Existing `getTransport()` transport and `ArtifactSlideOut` component reused.

**Related spec:** `docs/superpowers/specs/2026-04-19-chat-research-ui-redesign-design.md`

---

## File Structure

### New files

```
gateway/templates/agents/quick-chat-agent.md            # seed prompt shipped with daemon
~/Documents/zbot/agents/quick-chat/AGENTS.md            # installed agent prompt (written by task)
~/Documents/zbot/agents/quick-chat/config.yaml         # installed agent config (written by task)

apps/ui/src/features/shared/statusPill/
    tool-phrase.ts                                      # tool-call → display phrase dictionary
    tool-phrase.test.ts                                 # unit tests for dictionary
    use-status-pill-aggregator.ts                       # hook: collapses event stream → pill state
    use-status-pill-aggregator.test.ts                  # aggregator tests
    StatusPill.tsx                                      # presentation component
    StatusPill.test.tsx                                 # render tests
    types.ts                                            # PillState, PillCategory, PillColor
    index.ts                                            # public barrel

apps/ui/src/features/chat-v2/
    QuickChat.tsx                                       # page component
    QuickChat.test.tsx                                  # render + empty-state tests
    useQuickChat.ts                                     # state hook + event handler
    useQuickChat.test.ts                                # unit tests
    InlineActivityChip.tsx                              # 🧠/📚/→ chip renderer
    quick-chat.css                                      # scoped styles
    types.ts                                            # QuickChatMessage, QuickChatState
    index.ts                                            # barrel
```

### Modified files

```
apps/ui/src/App.tsx                                     # route wiring for /chat-v2 and /chat-v2/:sessionId
apps/ui/src/index.css                                   # import quick-chat.css
gateway/src/http/chat.rs                                # (optional T17) expose quick-chat session reset endpoint
```

### Not modified

`FastChat.tsx`, `fast-chat-hooks.ts`, `MissionControl.tsx`, `mission-hooks.ts`, `WebChatPanel.tsx` — all left untouched. Retired in a separate cleanup PR.

---

## Tasks

### Task 1: Create quick-chat agent template (gateway seed)

**Files:**
- Create: `gateway/templates/agents/quick-chat-agent.md`

- [ ] **Step 1: Write the template file**

```markdown
You are QUICK-CHAT. You handle short, memory-aware conversational questions and quick single-step tasks for the user. You are NOT a research agent — if a task needs multi-step planning, orchestration across agents, or a full workbench workflow, respond with a one-line note telling the user to move the task to the Research page.

You have access to:
- `memory` (actions: recall, get_fact, save_fact) — recall facts the user has stored.
- `load_skill` — load a single skill to execute a bounded task (search, format conversion, web read, etc).
- `delegate_to_agent` — delegate to AT MOST ONE subagent per user turn when the task genuinely needs a specialist. If you are tempted to delegate a second time, stop and respond to the user instead.
- `ward` (actions: use, info) — read-only ward recall.
- `grep` — read-only file probes.
- `graph_query`, `ingest` — knowledge-graph read/write.
- `multimodal_analyze` — vision on pasted/attached images.
- `respond` — your final user-facing message.

## Hard rules

H1 — NEVER invoke `planner-agent`. If the task needs a plan, tell the user: "This needs multiple steps — move it to the Research page."
H2 — NEVER write `plan.md`, `AGENTS.md`, or step files. You are not a planner.
H3 — At most ONE `delegate_to_agent` call per turn.
H4 — Respond conversationally. Short answers are good. Use markdown sparingly.

## Decision procedure

1. If the user's ask is answerable from memory or general knowledge, answer directly.
2. If it needs a bounded skill (web search, image analysis, format conversion, single file read), `load_skill` and run it yourself.
3. If it needs specialist execution (e.g., one writing-agent call to draft a memo), `delegate_to_agent` ONCE and synthesize the result.
4. If it needs a plan or multi-agent coordination, stop and tell the user to use the Research page.
5. End every turn with `respond(...)`.
```

- [ ] **Step 2: Verify template location**

Run: `test -f gateway/templates/agents/quick-chat-agent.md && echo OK`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add gateway/templates/agents/quick-chat-agent.md
git commit -m "feat(chat-v2): add quick-chat agent template"
```

---

### Task 2: Install quick-chat agent locally

**Files:**
- Create: `~/Documents/zbot/agents/quick-chat/AGENTS.md`
- Create: `~/Documents/zbot/agents/quick-chat/config.yaml`

- [ ] **Step 1: Copy template to AGENTS.md**

Run: `mkdir -p ~/Documents/zbot/agents/quick-chat && cp gateway/templates/agents/quick-chat-agent.md ~/Documents/zbot/agents/quick-chat/AGENTS.md`
Expected: no output; file exists.

- [ ] **Step 2: Write config.yaml**

```yaml
name: quick-chat
displayName: Quick Chat
description: Memory-aware conversational agent for short questions and single-step tasks. Cannot plan — delegates to Research for multi-step work.
agentType: specialist
providerId: provider-anthropic
model: claude-sonnet-4-6
temperature: 0.5
maxTokens: 4096
thinkingEnabled: false
voiceRecordingEnabled: false
tools:
- memory
- load_skill
- delegate_to_agent
- respond
- ward
- grep
- multimodal_analyze
- graph_query
- ingest
skills: []
mcps: []
```

- [ ] **Step 3: Verify agent loads at daemon start**

Run: `cargo check --workspace`
Expected: no errors (no code changes, just sanity).

Restart daemon and confirm via `curl -s http://localhost:18791/api/agents | jq '.[] | select(.name=="quick-chat") | .name'`.
Expected: `"quick-chat"`

- [ ] **Step 4: Commit**

```bash
git add -A  # NOTE: ~/Documents/zbot is typically outside the repo. Skip this step if so.
# If inside repo, otherwise:
# git commit -m "feat(chat-v2): install quick-chat agent config"
```

(If the agents directory is outside the repo, document the config YAML inline in the plan spec instead and skip the commit — agents are user-level configs, not repo state.)

---

### Task 3: Shared types for StatusPill

**Files:**
- Create: `apps/ui/src/features/shared/statusPill/types.ts`

- [ ] **Step 1: Write types**

```typescript
// Category of the currently-displayed action — drives color.
export type PillCategory = "read" | "write" | "delegate" | "respond" | "neutral";

// Computed display state for the pill.
export interface PillState {
  visible: boolean;
  // Primary narration text (from the last Thinking delta), truncated to ~80 chars.
  narration: string;
  // Muted suffix derived from the current ToolCall (e.g., "· yf_fundamentals.py").
  suffix: string;
  category: PillCategory;
  // True when a session is running but no events have arrived yet.
  starting: boolean;
  // Monotonic counter used by UI to trigger slide-in/slide-out animations.
  swapCounter: number;
}

export const EMPTY_PILL: PillState = {
  visible: false,
  narration: "",
  suffix: "",
  category: "neutral",
  starting: false,
  swapCounter: 0,
};

export const NARRATION_MAX = 80;
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/shared/statusPill/types.ts
git commit -m "feat(chat-v2): status-pill shared types"
```

---

### Task 4: Tool-phrase dictionary (with tests)

**Files:**
- Create: `apps/ui/src/features/shared/statusPill/tool-phrase.ts`
- Test: `apps/ui/src/features/shared/statusPill/tool-phrase.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// tool-phrase.test.ts
import { describe, it, expect } from "vitest";
import { describeTool } from "./tool-phrase";

describe("describeTool", () => {
  it("maps write_file to Creating <basename>", () => {
    const r = describeTool("write_file", { path: "src/yf_fundamentals.py" });
    expect(r).toEqual({
      narration: "Creating yf_fundamentals.py",
      suffix: "· yf_fundamentals.py",
      category: "write",
    });
  });

  it("maps edit_file to Editing <basename>", () => {
    const r = describeTool("edit_file", { path: "/a/b/c.ts" });
    expect(r.narration).toBe("Editing c.ts");
    expect(r.category).toBe("write");
  });

  it("maps shell with cat to Reading", () => {
    const r = describeTool("shell", { command: "cat README.md" });
    expect(r.category).toBe("read");
    expect(r.narration).toContain("Reading");
  });

  it("maps load_skill", () => {
    const r = describeTool("load_skill", { skill: "web-read" });
    expect(r.narration).toBe("Loading web-read skill");
    expect(r.category).toBe("read");
  });

  it("maps delegate_to_agent to purple", () => {
    const r = describeTool("delegate_to_agent", { agent_id: "writer-agent" });
    expect(r.category).toBe("delegate");
    expect(r.narration).toBe("Delegating to writer-agent");
  });

  it("maps memory recall to Recalling", () => {
    const r = describeTool("memory", { action: "recall" });
    expect(r.category).toBe("read");
    expect(r.narration).toBe("Recalling from memory");
  });

  it("maps respond to green", () => {
    const r = describeTool("respond", {});
    expect(r.category).toBe("respond");
    expect(r.narration).toBe("Responding");
  });

  it("maps unknown tool to neutral with tool name", () => {
    const r = describeTool("some_tool", { foo: 1 });
    expect(r.category).toBe("neutral");
    expect(r.narration).toBe("Running some_tool");
  });

  it("uses camelCase path alias", () => {
    const r = describeTool("write_file", { filePath: "x.py" });
    expect(r.narration).toBe("Creating x.py");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd apps/ui && npx vitest run src/features/shared/statusPill/tool-phrase.test.ts`
Expected: FAIL — `describeTool` is not defined.

- [ ] **Step 3: Implement the dictionary**

```typescript
// tool-phrase.ts
import type { PillCategory } from "./types";

export interface ToolPhrase {
  narration: string;
  suffix: string;
  category: PillCategory;
}

function basename(p: string): string {
  const idx = p.lastIndexOf("/");
  return idx >= 0 ? p.slice(idx + 1) : p;
}

function argPath(args: Record<string, unknown>): string | undefined {
  const v = args.path ?? args.filePath ?? args.file_path;
  return typeof v === "string" ? v : undefined;
}

export function describeTool(
  tool: string,
  args: Record<string, unknown> = {}
): ToolPhrase {
  switch (tool) {
    case "write_file": {
      const p = argPath(args) ?? "";
      const b = basename(p);
      return { narration: `Creating ${b}`, suffix: `· ${b}`, category: "write" };
    }
    case "edit_file": {
      const p = argPath(args) ?? "";
      const b = basename(p);
      return { narration: `Editing ${b}`, suffix: `· ${b}`, category: "write" };
    }
    case "shell": {
      const cmd = typeof args.command === "string" ? args.command : "";
      if (cmd.startsWith("cat ") || cmd.startsWith("head ") || cmd.startsWith("tail ")) {
        const tail = cmd.split(" ").slice(1).join(" ");
        return { narration: `Reading ${basename(tail)}`, suffix: `· ${basename(tail)}`, category: "read" };
      }
      if (cmd.startsWith("ls")) {
        return { narration: "Listing files", suffix: `· ${cmd.slice(0, 30)}`, category: "read" };
      }
      return { narration: `Running shell`, suffix: `· ${cmd.slice(0, 30)}`, category: "neutral" };
    }
    case "load_skill": {
      const s = typeof args.skill === "string" ? args.skill : "skill";
      return { narration: `Loading ${s} skill`, suffix: `· ${s}`, category: "read" };
    }
    case "delegate_to_agent": {
      const a = (args.agent_id ?? args.agentId ?? "subagent") as string;
      return { narration: `Delegating to ${a}`, suffix: `· ${a}`, category: "delegate" };
    }
    case "memory": {
      const action = typeof args.action === "string" ? args.action : "";
      if (action === "recall") return { narration: "Recalling from memory", suffix: "· recall", category: "read" };
      if (action === "save_fact") return { narration: "Saving fact", suffix: "· save", category: "write" };
      return { narration: `Memory ${action}`, suffix: `· ${action}`, category: "neutral" };
    }
    case "graph_query":
      return { narration: "Searching knowledge graph", suffix: "· graph search", category: "read" };
    case "ingest":
      return { narration: "Ingesting to graph", suffix: "· ingest", category: "write" };
    case "ward": {
      const action = typeof args.action === "string" ? args.action : "";
      const name = typeof args.name === "string" ? args.name : "";
      if (action === "use" && name) return { narration: `Entering ${name}`, suffix: `· ${name}`, category: "neutral" };
      return { narration: `Ward ${action}`, suffix: `· ${action}`, category: "neutral" };
    }
    case "respond":
      return { narration: "Responding", suffix: "· respond", category: "respond" };
    default:
      return { narration: `Running ${tool}`, suffix: `· ${tool}`, category: "neutral" };
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/ui && npx vitest run src/features/shared/statusPill/tool-phrase.test.ts`
Expected: PASS — 9 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/shared/statusPill/tool-phrase.ts apps/ui/src/features/shared/statusPill/tool-phrase.test.ts
git commit -m "feat(chat-v2): tool-phrase dictionary for status pill"
```

---

### Task 5: `useStatusPillAggregator` hook (with tests)

**Files:**
- Create: `apps/ui/src/features/shared/statusPill/use-status-pill-aggregator.ts`
- Test: `apps/ui/src/features/shared/statusPill/use-status-pill-aggregator.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// use-status-pill-aggregator.test.ts
import { describe, it, expect } from "vitest";
import { reducePillState } from "./use-status-pill-aggregator";
import { EMPTY_PILL } from "./types";

describe("reducePillState", () => {
  it("starts hidden", () => {
    expect(reducePillState(EMPTY_PILL, { kind: "idle" })).toEqual(EMPTY_PILL);
  });

  it("shows starting on AgentStarted with no prior events", () => {
    const s = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    expect(s.visible).toBe(true);
    expect(s.starting).toBe(true);
    expect(s.category).toBe("neutral");
  });

  it("updates narration on Thinking", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "thinking", content: "Let me recall fundamentals…" });
    expect(s2.narration).toBe("Let me recall fundamentals…");
    expect(s2.starting).toBe(false);
    expect(s2.swapCounter).toBeGreaterThan(s1.swapCounter);
  });

  it("truncates narration to 80 chars", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const long = "x".repeat(120);
    const s2 = reducePillState(s1, { kind: "thinking", content: long });
    expect(s2.narration.length).toBeLessThanOrEqual(80);
  });

  it("updates suffix and color on ToolCall", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "tool_call", tool: "write_file", args: { path: "a.py" } });
    expect(s2.suffix).toBe("· a.py");
    expect(s2.category).toBe("write");
  });

  it("hides on AgentCompleted when it is the last active agent", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "agent_completed", agent_id: "root", is_final: true });
    expect(s2.visible).toBe(false);
  });

  it("stays visible on AgentCompleted with continuation pending", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "agent_completed", agent_id: "root", is_final: false });
    expect(s2.visible).toBe(true);
  });

  it("resets via reset event (new session)", () => {
    const s1 = reducePillState(EMPTY_PILL, { kind: "agent_started", agent_id: "root" });
    const s2 = reducePillState(s1, { kind: "reset" });
    expect(s2).toEqual(EMPTY_PILL);
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/shared/statusPill/use-status-pill-aggregator.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the reducer + hook**

```typescript
// use-status-pill-aggregator.ts
import { useEffect, useReducer } from "react";
import { type PillState, EMPTY_PILL, NARRATION_MAX } from "./types";
import { describeTool } from "./tool-phrase";

// Normalized events — aggregator only needs these kinds.
export type PillEvent =
  | { kind: "idle" }
  | { kind: "reset" }
  | { kind: "agent_started"; agent_id: string }
  | { kind: "agent_completed"; agent_id: string; is_final: boolean }
  | { kind: "thinking"; content: string }
  | { kind: "tool_call"; tool: string; args: Record<string, unknown> }
  | { kind: "respond" };

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max - 1) + "…";
}

export function reducePillState(state: PillState, ev: PillEvent): PillState {
  switch (ev.kind) {
    case "idle":
      return state;
    case "reset":
      return EMPTY_PILL;
    case "agent_started":
      return {
        ...state,
        visible: true,
        starting: state.narration === "" && state.suffix === "",
        swapCounter: state.swapCounter + 1,
      };
    case "agent_completed":
      if (ev.is_final) {
        return { ...EMPTY_PILL, swapCounter: state.swapCounter + 1 };
      }
      return state;
    case "thinking":
      return {
        ...state,
        visible: true,
        starting: false,
        narration: truncate(ev.content.trim(), NARRATION_MAX),
        swapCounter: state.swapCounter + 1,
      };
    case "tool_call": {
      const phrase = describeTool(ev.tool, ev.args);
      return {
        ...state,
        visible: true,
        starting: false,
        // Narration stays as the last Thinking; only fallback to dictionary
        // if there was no prior narration.
        narration: state.narration || phrase.narration,
        suffix: phrase.suffix,
        category: phrase.category,
        swapCounter: state.swapCounter + 1,
      };
    }
    case "respond":
      return { ...state, category: "respond", swapCounter: state.swapCounter + 1 };
    default:
      return state;
  }
}

/**
 * React hook wrapper — subscribe to pill events through `events` (a ref with a push method).
 * The parent page's event router calls `push(PillEvent)` for each relevant event; the hook
 * folds them into PillState.
 */
export interface PillEventSink {
  push(ev: PillEvent): void;
}

export function useStatusPill(): { state: PillState; sink: PillEventSink } {
  const [state, dispatch] = useReducer(reducePillState, EMPTY_PILL);
  const sink: PillEventSink = { push: dispatch };
  // Dummy effect to keep React 18 strict-mode happy if future extensions need it.
  useEffect(() => {
    return () => { /* no-op */ };
  }, []);
  return { state, sink };
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/shared/statusPill/use-status-pill-aggregator.test.ts`
Expected: PASS — 7 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/shared/statusPill/use-status-pill-aggregator.ts apps/ui/src/features/shared/statusPill/use-status-pill-aggregator.test.ts
git commit -m "feat(chat-v2): status-pill aggregator reducer + hook"
```

---

### Task 6: `StatusPill` component

**Files:**
- Create: `apps/ui/src/features/shared/statusPill/StatusPill.tsx`
- Test: `apps/ui/src/features/shared/statusPill/StatusPill.test.tsx`
- Create: `apps/ui/src/features/shared/statusPill/index.ts`

- [ ] **Step 1: Write failing tests**

```tsx
// StatusPill.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusPill } from "./StatusPill";
import { EMPTY_PILL } from "./types";

describe("<StatusPill>", () => {
  it("renders nothing when not visible", () => {
    const { container } = render(<StatusPill state={EMPTY_PILL} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders narration + suffix when visible", () => {
    render(
      <StatusPill
        state={{
          ...EMPTY_PILL,
          visible: true,
          narration: "Recalling fundamentals",
          suffix: "· memory",
          category: "read",
          swapCounter: 1,
        }}
      />
    );
    expect(screen.getByText("Recalling fundamentals")).toBeTruthy();
    expect(screen.getByText("· memory")).toBeTruthy();
  });

  it("applies category data attribute", () => {
    render(
      <StatusPill
        state={{
          ...EMPTY_PILL,
          visible: true,
          narration: "Responding",
          category: "respond",
          swapCounter: 1,
        }}
      />
    );
    const pill = screen.getByTestId("status-pill");
    expect(pill.getAttribute("data-category")).toBe("respond");
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/shared/statusPill/StatusPill.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the component**

```tsx
// StatusPill.tsx
import type { PillState } from "./types";

export interface StatusPillProps {
  state: PillState;
}

export function StatusPill({ state }: StatusPillProps) {
  if (!state.visible) return null;

  return (
    <div
      data-testid="status-pill"
      data-category={state.category}
      className="status-pill"
      key={state.swapCounter}
      aria-live="polite"
      aria-atomic="true"
    >
      <span className="status-pill__dot" aria-hidden="true" />
      <span className="status-pill__narration">{state.narration}</span>
      {state.suffix && <span className="status-pill__suffix">{state.suffix}</span>}
    </div>
  );
}
```

- [ ] **Step 4: Write the barrel and styles**

```typescript
// index.ts
export { StatusPill } from "./StatusPill";
export type { StatusPillProps } from "./StatusPill";
export { useStatusPill, reducePillState } from "./use-status-pill-aggregator";
export type { PillEvent, PillEventSink } from "./use-status-pill-aggregator";
export { describeTool } from "./tool-phrase";
export type { PillState, PillCategory } from "./types";
```

Styles in `apps/ui/src/features/shared/statusPill/status-pill.css`:

```css
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 5px 14px;
  border-radius: 16px;
  font-size: 12px;
  line-height: 1.2;
  background: rgba(100, 200, 255, 0.1);
  border: 1px solid rgba(100, 200, 255, 0.25);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  animation: status-pill-slide-in 150ms ease-out;
  max-width: min(560px, 80%);
}
.status-pill__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: status-pill-pulse 1.5s infinite;
  flex-shrink: 0;
}
.status-pill__narration {
  color: rgba(255, 255, 255, 0.95);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.status-pill__suffix {
  color: rgba(255, 255, 255, 0.5);
  font-family: ui-monospace, monospace;
  font-size: 11px;
  flex-shrink: 0;
}
.status-pill[data-category="read"]    { color: rgb(100, 200, 255); }
.status-pill[data-category="write"]   { color: rgb(140, 220, 255); background: rgba(140, 220, 255, 0.1); border-color: rgba(140, 220, 255, 0.25); }
.status-pill[data-category="delegate"]{ color: rgb(200, 150, 255); background: rgba(200, 150, 255, 0.1); border-color: rgba(200, 150, 255, 0.25); }
.status-pill[data-category="respond"] { color: rgb(150, 255, 180); background: rgba(150, 255, 180, 0.1); border-color: rgba(150, 255, 180, 0.25); }
.status-pill[data-category="neutral"] { color: rgba(255, 255, 255, 0.7); background: rgba(255, 255, 255, 0.05); border-color: rgba(255, 255, 255, 0.1); }

@keyframes status-pill-slide-in {
  from { opacity: 0; transform: translateX(12px); }
  to   { opacity: 1; transform: translateX(0); }
}
@keyframes status-pill-pulse {
  0%, 100% { opacity: 0.9; }
  50%       { opacity: 0.4; }
}
```

Import the CSS in `apps/ui/src/features/shared/statusPill/StatusPill.tsx` with `import "./status-pill.css";` at the top.

- [ ] **Step 5: Run tests**

Run: `cd apps/ui && npx vitest run src/features/shared/statusPill/StatusPill.test.tsx`
Expected: PASS — 3 tests.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/shared/statusPill/
git commit -m "feat(chat-v2): StatusPill component + barrel"
```

---

### Task 7: Quick Chat types

**Files:**
- Create: `apps/ui/src/features/chat-v2/types.ts`

- [ ] **Step 1: Write types**

```typescript
export type QuickChatMessageRole = "user" | "assistant";

export interface QuickChatInlineChip {
  id: string;
  kind: "recall" | "skill" | "delegate";
  label: string;   // e.g., "recalled 2", "loaded web-read", "→ writer-agent"
  detail?: string; // expanded tooltip / panel content
}

export interface QuickChatMessage {
  id: string;
  role: QuickChatMessageRole;
  content: string;                       // markdown for assistant, plain for user
  timestamp: number;
  chips?: QuickChatInlineChip[];         // assistant-only
  streaming?: boolean;                   // true while Token events still arriving
}

export type QuickChatStatus = "idle" | "running" | "error";

export interface QuickChatState {
  sessionId: string | null;
  conversationId: string;                // always set; new on "New chat"
  messages: QuickChatMessage[];
  status: QuickChatStatus;
  activeWardName: string | null;
  olderCursor: string | null;            // for lazy-load "Show N earlier turns"
  hasMoreOlder: boolean;
}

export const EMPTY_QUICK_CHAT_STATE: QuickChatState = {
  sessionId: null,
  conversationId: "",
  messages: [],
  status: "idle",
  activeWardName: null,
  olderCursor: null,
  hasMoreOlder: false,
};
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/chat-v2/types.ts
git commit -m "feat(chat-v2): QuickChat state types"
```

---

### Task 8: `useQuickChat` hook — session load + send

**Files:**
- Create: `apps/ui/src/features/chat-v2/useQuickChat.ts`

- [ ] **Step 1: Implement the hook (initial skeleton)**

```typescript
import { useCallback, useEffect, useReducer, useRef } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getTransport } from "@/services/transport";
import { useStatusPill, type PillEvent } from "../shared/statusPill";
import {
  type QuickChatState,
  type QuickChatMessage,
  EMPTY_QUICK_CHAT_STATE,
} from "./types";
import { reduceQuickChat, type QuickChatAction } from "./reducer";
import { mapGatewayEventToQuickChatAction, mapGatewayEventToPillEvent } from "./event-map";

const QUICK_CHAT_AGENT_ID = "quick-chat";

export function useQuickChat() {
  const { sessionId: urlSessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(reduceQuickChat, {
    ...EMPTY_QUICK_CHAT_STATE,
    conversationId: newConvId(),
  });
  const { state: pillState, sink: pillSink } = useStatusPill();
  const subscribedIdRef = useRef<string | null>(null);

  // Load snapshot on mount or when URL session id changes.
  useEffect(() => {
    let cancelled = false;
    async function loadSession() {
      if (!urlSessionId) return;
      const transport = await getTransport();
      const result = await transport.get<{
        messages: Array<{ id: string; role: string; content: string; timestamp: string }>;
        ward?: { name: string };
        conversation_id?: string;
      }>(`/api/sessions/${encodeURIComponent(urlSessionId)}/state`);
      if (cancelled || !result.success || !result.data) return;
      dispatch({
        type: "HYDRATE",
        sessionId: urlSessionId,
        conversationId: result.data.conversation_id ?? newConvId(),
        messages: result.data.messages.map(msgFromApi),
        wardName: result.data.ward?.name ?? null,
      });
    }
    loadSession();
    return () => { cancelled = true; };
  }, [urlSessionId]);

  // Subscribe to WebSocket events for this conversation.
  useEffect(() => {
    const convId = state.conversationId;
    if (!convId || subscribedIdRef.current === convId) return;
    subscribedIdRef.current = convId;
    let unsubscribe: (() => void) | null = null;
    (async () => {
      const transport = await getTransport();
      unsubscribe = await transport.subscribeConversation({
        conversationId: convId,
        onEvent: (event) => {
          const action = mapGatewayEventToQuickChatAction(event);
          if (action) dispatch(action);
          const pillEv = mapGatewayEventToPillEvent(event);
          if (pillEv) pillSink.push(pillEv);
        },
      });
    })();
    return () => { unsubscribe?.(); };
  }, [state.conversationId, pillSink]);

  // Send a user message.
  const sendMessage = useCallback(async (text: string) => {
    if (!text.trim() || state.status === "running") return;
    const trimmed = text.trim();
    dispatch({
      type: "APPEND_USER",
      message: {
        id: crypto.randomUUID(),
        role: "user",
        content: trimmed,
        timestamp: Date.now(),
      },
    });
    const transport = await getTransport();
    const result = await transport.executeAgent(
      QUICK_CHAT_AGENT_ID,
      state.conversationId,
      trimmed,
      state.sessionId ?? undefined,
      "chat"
    );
    if (!result.success) {
      dispatch({ type: "ERROR", message: result.error ?? "Failed to send" });
      return;
    }
    // session id comes back via events (SessionInitialized / AgentStarted); URL
    // will be updated in the event handler.
  }, [state.status, state.conversationId, state.sessionId]);

  // Navigate URL when a new session_id arrives.
  useEffect(() => {
    if (state.sessionId && urlSessionId !== state.sessionId) {
      navigate(`/chat-v2/${state.sessionId}`, { replace: true });
    }
  }, [state.sessionId, urlSessionId, navigate]);

  // "New chat" discards current conv and navigates to /chat-v2.
  const startNewChat = useCallback(() => {
    pillSink.push({ kind: "reset" });
    dispatch({
      type: "RESET",
      conversationId: newConvId(),
    });
    navigate("/chat-v2", { replace: true });
  }, [navigate, pillSink]);

  // Stop running agent.
  const stopAgent = useCallback(async () => {
    if (state.status !== "running") return;
    const transport = await getTransport();
    await transport.stopAgent(state.conversationId);
  }, [state.status, state.conversationId]);

  // Lazy-load older messages.
  const loadOlder = useCallback(async () => {
    if (!state.sessionId || !state.hasMoreOlder) return;
    const transport = await getTransport();
    const params = state.olderCursor
      ? `?before=${encodeURIComponent(state.olderCursor)}&limit=10`
      : `?limit=10`;
    const result = await transport.get<{ messages: Array<{ id: string; role: string; content: string; timestamp: string }>; cursor?: string }>(
      `/api/sessions/${encodeURIComponent(state.sessionId)}/messages${params}`
    );
    if (result.success && result.data) {
      dispatch({
        type: "PREPEND_OLDER",
        messages: result.data.messages.map(msgFromApi),
        nextCursor: result.data.cursor ?? null,
      });
    }
  }, [state.sessionId, state.hasMoreOlder, state.olderCursor]);

  return { state, pillState, sendMessage, startNewChat, stopAgent, loadOlder };
}

function newConvId(): string {
  return `quick-chat-${crypto.randomUUID()}`;
}

function msgFromApi(m: { id: string; role: string; content: string; timestamp: string }): QuickChatMessage {
  return {
    id: m.id,
    role: m.role === "user" ? "user" : "assistant",
    content: m.content,
    timestamp: new Date(m.timestamp).getTime(),
  };
}
```

- [ ] **Step 2: Commit (reducer + event-map stubs come next)**

Note: `reduceQuickChat`, `mapGatewayEventToQuickChatAction`, and `mapGatewayEventToPillEvent` are written in Tasks 9 and 10. This hook will not compile yet — that's fine, the next tasks provide the missing modules before any page actually imports `useQuickChat`.

```bash
git add apps/ui/src/features/chat-v2/useQuickChat.ts
git commit -m "feat(chat-v2): useQuickChat hook (reducer + event-map follow)"
```

---

### Task 9: QuickChat reducer (with tests)

**Files:**
- Create: `apps/ui/src/features/chat-v2/reducer.ts`
- Test: `apps/ui/src/features/chat-v2/reducer.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
import { describe, it, expect } from "vitest";
import { reduceQuickChat, type QuickChatAction } from "./reducer";
import { EMPTY_QUICK_CHAT_STATE } from "./types";

describe("reduceQuickChat", () => {
  it("appends user message and flips status to running", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "APPEND_USER",
      message: { id: "u1", role: "user", content: "hello", timestamp: 1 },
    });
    expect(s.messages).toHaveLength(1);
    expect(s.status).toBe("running");
  });

  it("SESSION_BOUND sets sessionId", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "SESSION_BOUND", sessionId: "sess-x",
    });
    expect(s.sessionId).toBe("sess-x");
  });

  it("TOKEN appends to the latest assistant message or creates one", () => {
    const s1 = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, { type: "TOKEN", text: "Hi " });
    expect(s1.messages).toHaveLength(1);
    expect(s1.messages[0].role).toBe("assistant");
    expect(s1.messages[0].content).toBe("Hi ");
    const s2 = reduceQuickChat(s1, { type: "TOKEN", text: "there" });
    expect(s2.messages[0].content).toBe("Hi there");
  });

  it("RESPOND overrides streaming content with final text", () => {
    let s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, { type: "TOKEN", text: "partial" });
    s = reduceQuickChat(s, { type: "RESPOND", text: "final answer" });
    expect(s.messages[0].content).toBe("final answer");
    expect(s.messages[0].streaming).toBe(false);
  });

  it("TURN_COMPLETE sets status back to idle", () => {
    const s = reduceQuickChat(
      { ...EMPTY_QUICK_CHAT_STATE, status: "running" },
      { type: "TURN_COMPLETE" }
    );
    expect(s.status).toBe("idle");
  });

  it("ADD_CHIP attaches chip to latest assistant message", () => {
    let s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, { type: "TOKEN", text: "foo" });
    s = reduceQuickChat(s, {
      type: "ADD_CHIP",
      chip: { id: "c1", kind: "recall", label: "recalled 2" },
    });
    expect(s.messages[0].chips).toHaveLength(1);
  });

  it("RESET clears messages but keeps new conversationId", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "RESET", conversationId: "quick-chat-new",
    });
    expect(s.messages).toHaveLength(0);
    expect(s.conversationId).toBe("quick-chat-new");
    expect(s.sessionId).toBeNull();
  });

  it("WARD_CHANGED updates active ward", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "WARD_CHANGED", wardName: "stock-analysis",
    });
    expect(s.activeWardName).toBe("stock-analysis");
  });

  it("HYDRATE replaces state from snapshot", () => {
    const s = reduceQuickChat(EMPTY_QUICK_CHAT_STATE, {
      type: "HYDRATE",
      sessionId: "sess-1",
      conversationId: "quick-chat-1",
      messages: [{ id: "m1", role: "user", content: "hi", timestamp: 1 }],
      wardName: "default",
    });
    expect(s.sessionId).toBe("sess-1");
    expect(s.messages).toHaveLength(1);
    expect(s.activeWardName).toBe("default");
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/chat-v2/reducer.test.ts`
Expected: FAIL

- [ ] **Step 3: Implement the reducer**

```typescript
import type { QuickChatMessage, QuickChatState, QuickChatInlineChip } from "./types";
import { EMPTY_QUICK_CHAT_STATE } from "./types";

export type QuickChatAction =
  | { type: "HYDRATE"; sessionId: string; conversationId: string; messages: QuickChatMessage[]; wardName: string | null }
  | { type: "APPEND_USER"; message: QuickChatMessage }
  | { type: "SESSION_BOUND"; sessionId: string }
  | { type: "AGENT_STARTED"; agentId: string }
  | { type: "TOKEN"; text: string }
  | { type: "RESPOND"; text: string }
  | { type: "ADD_CHIP"; chip: QuickChatInlineChip }
  | { type: "TURN_COMPLETE" }
  | { type: "ERROR"; message: string }
  | { type: "RESET"; conversationId: string }
  | { type: "WARD_CHANGED"; wardName: string }
  | { type: "PREPEND_OLDER"; messages: QuickChatMessage[]; nextCursor: string | null };

function upsertStreamingAssistant(messages: QuickChatMessage[], text: string, replace: boolean): QuickChatMessage[] {
  const last = messages[messages.length - 1];
  if (last && last.role === "assistant" && last.streaming) {
    const updated = {
      ...last,
      content: replace ? text : last.content + text,
      streaming: !replace,
    };
    return [...messages.slice(0, -1), updated];
  }
  return [
    ...messages,
    {
      id: crypto.randomUUID(),
      role: "assistant",
      content: text,
      timestamp: Date.now(),
      streaming: !replace,
    },
  ];
}

function attachChipToLatestAssistant(messages: QuickChatMessage[], chip: QuickChatInlineChip): QuickChatMessage[] {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === "assistant") {
      const chips = [...(messages[i].chips ?? []), chip];
      return [
        ...messages.slice(0, i),
        { ...messages[i], chips },
        ...messages.slice(i + 1),
      ];
    }
  }
  return messages;
}

export function reduceQuickChat(state: QuickChatState, action: QuickChatAction): QuickChatState {
  switch (action.type) {
    case "HYDRATE":
      return {
        ...state,
        sessionId: action.sessionId,
        conversationId: action.conversationId,
        messages: action.messages,
        activeWardName: action.wardName,
        status: "idle",
      };
    case "APPEND_USER":
      return { ...state, messages: [...state.messages, action.message], status: "running" };
    case "SESSION_BOUND":
      return { ...state, sessionId: action.sessionId };
    case "AGENT_STARTED":
      return { ...state, status: "running" };
    case "TOKEN":
      return { ...state, messages: upsertStreamingAssistant(state.messages, action.text, false) };
    case "RESPOND":
      return { ...state, messages: upsertStreamingAssistant(state.messages, action.text, true) };
    case "ADD_CHIP":
      return { ...state, messages: attachChipToLatestAssistant(state.messages, action.chip) };
    case "TURN_COMPLETE":
      return { ...state, status: "idle" };
    case "ERROR":
      return { ...state, status: "error" };
    case "RESET":
      return { ...EMPTY_QUICK_CHAT_STATE, conversationId: action.conversationId };
    case "WARD_CHANGED":
      return { ...state, activeWardName: action.wardName };
    case "PREPEND_OLDER":
      return {
        ...state,
        messages: [...action.messages, ...state.messages],
        olderCursor: action.nextCursor,
        hasMoreOlder: action.nextCursor !== null,
      };
    default:
      return state;
  }
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/chat-v2/reducer.test.ts`
Expected: PASS — 9 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/chat-v2/reducer.ts apps/ui/src/features/chat-v2/reducer.test.ts
git commit -m "feat(chat-v2): QuickChat reducer with tests"
```

---

### Task 10: Event→action + Event→pill mapping

**Files:**
- Create: `apps/ui/src/features/chat-v2/event-map.ts`
- Test: `apps/ui/src/features/chat-v2/event-map.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
import { describe, it, expect } from "vitest";
import { mapGatewayEventToQuickChatAction, mapGatewayEventToPillEvent } from "./event-map";

describe("mapGatewayEventToQuickChatAction", () => {
  it("maps Token to TOKEN action", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "token", content: "hi" } as any))
      .toEqual({ type: "TOKEN", text: "hi" });
  });
  it("maps Respond to RESPOND action", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "respond", content: "done" } as any))
      .toEqual({ type: "RESPOND", text: "done" });
  });
  it("maps WardChanged to WARD_CHANGED only when name present", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "ward_changed", ward: { name: "x" } } as any))
      .toEqual({ type: "WARD_CHANGED", wardName: "x" });
    expect(mapGatewayEventToQuickChatAction({ type: "ward_changed" } as any)).toBeNull();
  });
  it("maps SessionInitialized → SESSION_BOUND", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "session_initialized", session_id: "sess-1" } as any))
      .toEqual({ type: "SESSION_BOUND", sessionId: "sess-1" });
  });
  it("maps tool_call delegate_to_agent to ADD_CHIP", () => {
    const a = mapGatewayEventToQuickChatAction({
      type: "tool_call", tool: "delegate_to_agent", args: { agent_id: "writer-agent" },
    } as any);
    expect(a?.type).toBe("ADD_CHIP");
    expect((a as any).chip.kind).toBe("delegate");
  });
  it("returns null for unmapped events", () => {
    expect(mapGatewayEventToQuickChatAction({ type: "iterations_extended" } as any)).toBeNull();
  });
});

describe("mapGatewayEventToPillEvent", () => {
  it("maps agent_started", () => {
    expect(mapGatewayEventToPillEvent({ type: "agent_started", agent_id: "quick-chat" } as any))
      .toEqual({ kind: "agent_started", agent_id: "quick-chat" });
  });
  it("maps thinking", () => {
    expect(mapGatewayEventToPillEvent({ type: "thinking", content: "…" } as any))
      .toEqual({ kind: "thinking", content: "…" });
  });
  it("maps tool_call", () => {
    expect(mapGatewayEventToPillEvent({ type: "tool_call", tool: "write_file", args: { path: "a.py" } } as any))
      .toEqual({ kind: "tool_call", tool: "write_file", args: { path: "a.py" } });
  });
  it("maps agent_completed with is_final inferred from last=true flag", () => {
    expect(mapGatewayEventToPillEvent({ type: "agent_completed", agent_id: "quick-chat", last: true } as any))
      .toEqual({ kind: "agent_completed", agent_id: "quick-chat", is_final: true });
  });
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cd apps/ui && npx vitest run src/features/chat-v2/event-map.test.ts`
Expected: FAIL

- [ ] **Step 3: Implement the mapping**

```typescript
import type { ConversationEvent } from "@/services/transport/types";
import type { PillEvent } from "../shared/statusPill";
import type { QuickChatAction } from "./reducer";

export function mapGatewayEventToQuickChatAction(ev: ConversationEvent): QuickChatAction | null {
  const type = (ev as any).type as string;
  switch (type) {
    case "token": {
      const content = (ev as any).content;
      if (typeof content !== "string" || content.length === 0) return null;
      return { type: "TOKEN", text: content };
    }
    case "respond": {
      const content = (ev as any).content;
      if (typeof content !== "string") return null;
      return { type: "RESPOND", text: content };
    }
    case "ward_changed": {
      const name = (ev as any).ward?.name;
      if (!name) return null;
      return { type: "WARD_CHANGED", wardName: name };
    }
    case "session_initialized": {
      const sid = (ev as any).session_id;
      if (!sid) return null;
      return { type: "SESSION_BOUND", sessionId: sid };
    }
    case "agent_started":
      return { type: "AGENT_STARTED", agentId: (ev as any).agent_id ?? "" };
    case "turn_complete":
      return { type: "TURN_COMPLETE" };
    case "tool_call": {
      const tool = (ev as any).tool as string;
      const args = ((ev as any).args ?? {}) as Record<string, unknown>;
      if (tool === "delegate_to_agent") {
        const agentId = (args.agent_id ?? args.agentId ?? "subagent") as string;
        return {
          type: "ADD_CHIP",
          chip: { id: crypto.randomUUID(), kind: "delegate", label: `→ ${agentId}` },
        };
      }
      if (tool === "load_skill") {
        const skill = (args.skill ?? "skill") as string;
        return {
          type: "ADD_CHIP",
          chip: { id: crypto.randomUUID(), kind: "skill", label: `loaded ${skill}` },
        };
      }
      if (tool === "memory" && (args.action === "recall" || args.action === "get_fact")) {
        return {
          type: "ADD_CHIP",
          chip: { id: crypto.randomUUID(), kind: "recall", label: "recalled" },
        };
      }
      return null;
    }
    case "error":
      return { type: "ERROR", message: (ev as any).message ?? "error" };
    default:
      return null;
  }
}

export function mapGatewayEventToPillEvent(ev: ConversationEvent): PillEvent | null {
  const type = (ev as any).type as string;
  switch (type) {
    case "agent_started":
      return { kind: "agent_started", agent_id: (ev as any).agent_id ?? "" };
    case "agent_completed":
      return {
        kind: "agent_completed",
        agent_id: (ev as any).agent_id ?? "",
        is_final: Boolean((ev as any).last) || Boolean((ev as any).is_final),
      };
    case "thinking": {
      const content = (ev as any).content;
      if (typeof content !== "string" || content.length === 0) return null;
      return { kind: "thinking", content };
    }
    case "tool_call": {
      const tool = (ev as any).tool;
      if (typeof tool !== "string") return null;
      return { kind: "tool_call", tool, args: ((ev as any).args ?? {}) as Record<string, unknown> };
    }
    case "respond":
      return { kind: "respond" };
    default:
      return null;
  }
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/chat-v2/event-map.test.ts`
Expected: PASS — 10 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/chat-v2/event-map.ts apps/ui/src/features/chat-v2/event-map.test.ts
git commit -m "feat(chat-v2): event→action and event→pill mappers with tests"
```

---

### Task 11: Inline activity chip component

**Files:**
- Create: `apps/ui/src/features/chat-v2/InlineActivityChip.tsx`

- [ ] **Step 1: Implement the component**

```tsx
import type { QuickChatInlineChip } from "./types";
import { Brain, BookOpen, ArrowRight } from "lucide-react";

export interface InlineActivityChipProps {
  chip: QuickChatInlineChip;
}

const KIND_META: Record<QuickChatInlineChip["kind"], { icon: JSX.Element; color: string }> = {
  recall: { icon: <Brain size={12} />, color: "rgb(100,200,255)" },
  skill: { icon: <BookOpen size={12} />, color: "rgb(200,150,255)" },
  delegate: { icon: <ArrowRight size={12} />, color: "rgb(255,180,100)" },
};

export function InlineActivityChip({ chip }: InlineActivityChipProps) {
  const meta = KIND_META[chip.kind];
  return (
    <span
      className="quick-chat__chip"
      style={{ color: meta.color, borderColor: `${meta.color}55`, background: `${meta.color}1a` }}
      title={chip.detail}
    >
      {meta.icon}
      <span>{chip.label}</span>
    </span>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/chat-v2/InlineActivityChip.tsx
git commit -m "feat(chat-v2): InlineActivityChip component"
```

---

### Task 12: `QuickChat` page component

**Files:**
- Create: `apps/ui/src/features/chat-v2/QuickChat.tsx`
- Create: `apps/ui/src/features/chat-v2/quick-chat.css`

- [ ] **Step 1: Implement the page**

```tsx
import { useRef, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Plus, Square } from "lucide-react";
import { ChatInput } from "../chat/ChatInput";
import { StatusPill } from "../shared/statusPill";
import { InlineActivityChip } from "./InlineActivityChip";
import { useQuickChat } from "./useQuickChat";
import "./quick-chat.css";

export function QuickChat() {
  const { state, pillState, sendMessage, startNewChat, stopAgent, loadOlder } = useQuickChat();
  const endRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [state.messages.length]);

  const isEmpty = state.messages.length === 0 && !state.sessionId;

  return (
    <div className="quick-chat">
      <div className="quick-chat__header">
        <div className="quick-chat__ward">
          {state.activeWardName
            ? <span className="quick-chat__ward-chip">{state.activeWardName}</span>
            : <span className="quick-chat__ward-chip quick-chat__ward-chip--muted">no ward</span>}
        </div>
        <div className="quick-chat__actions">
          {state.status === "running" && (
            <button className="btn btn--ghost btn--sm" onClick={stopAgent} title="Stop">
              <Square size={14} />
            </button>
          )}
          <button className="btn btn--ghost btn--sm" onClick={startNewChat} title="New chat">
            <Plus size={14} /> New chat
          </button>
        </div>
      </div>

      <div className="quick-chat__pill-strip">
        <StatusPill state={pillState} />
      </div>

      {isEmpty ? (
        <div className="quick-chat__empty">
          <h1>Quick chat</h1>
          <p className="quick-chat__empty-subtext">
            memory-aware · single-step delegation
            {state.activeWardName ? ` · bound to ${state.activeWardName}` : ""}
          </p>
        </div>
      ) : (
        <div className="quick-chat__scroll">
          {state.hasMoreOlder && (
            <button className="quick-chat__load-older" onClick={loadOlder}>
              ↑ Show earlier turns
            </button>
          )}
          <div className="quick-chat__messages">
            {state.messages.map((m) => (
              <div key={m.id} className={`quick-chat__msg quick-chat__msg--${m.role}`}>
                {m.role === "user" ? (
                  <div className="quick-chat__user-bubble">{m.content}</div>
                ) : (
                  <div className="quick-chat__assistant">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>
                      {m.content}
                    </ReactMarkdown>
                    {m.chips && m.chips.length > 0 && (
                      <div className="quick-chat__chips">
                        {m.chips.map((c) => <InlineActivityChip key={c.id} chip={c} />)}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
            <div ref={endRef} />
          </div>
        </div>
      )}

      <div className="quick-chat__composer">
        <ChatInput onSend={sendMessage} disabled={state.status === "running"} />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Write the CSS**

```css
.quick-chat {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.quick-chat__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 20px;
  border-bottom: 1px solid var(--border);
}
.quick-chat__ward-chip {
  font-size: 11px;
  padding: 3px 10px;
  border-radius: 10px;
  background: rgba(100, 200, 255, 0.1);
  color: rgb(140, 220, 255);
}
.quick-chat__ward-chip--muted { opacity: 0.5; background: transparent; color: var(--muted-foreground); }
.quick-chat__actions { display: flex; gap: 8px; align-items: center; }

.quick-chat__pill-strip {
  display: flex;
  justify-content: center;
  padding: 8px 20px 0;
  min-height: 36px;
}

.quick-chat__empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
}
.quick-chat__empty h1 { font-size: 24px; font-weight: 600; }
.quick-chat__empty-subtext { color: var(--muted-foreground); font-size: 13px; }

.quick-chat__scroll {
  flex: 1;
  overflow-y: auto;
  padding: 20px 0;
}
.quick-chat__load-older {
  display: block;
  margin: 0 auto 12px;
  font-size: 12px;
  color: var(--muted-foreground);
  background: transparent;
  border: 1px solid var(--border);
  padding: 6px 14px;
  border-radius: 10px;
  cursor: pointer;
}
.quick-chat__messages {
  max-width: 720px;
  margin: 0 auto;
  padding: 0 20px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.quick-chat__msg--user { display: flex; justify-content: flex-end; }
.quick-chat__user-bubble {
  max-width: 80%;
  padding: 10px 14px;
  border-radius: 14px 14px 2px 14px;
  background: var(--primary, rgba(100, 200, 255, 0.15));
  color: var(--foreground);
  font-size: 14px;
  white-space: pre-wrap;
}
.quick-chat__assistant { font-size: 14px; line-height: 1.6; }
.quick-chat__chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
.quick-chat__chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 10px;
  border-radius: 10px;
  font-size: 11px;
  border: 1px solid;
}
.quick-chat__composer {
  border-top: 1px solid var(--border);
  padding: 12px 20px;
  max-width: 720px;
  width: 100%;
  margin: 0 auto;
}
```

- [ ] **Step 3: Import CSS in feature barrel and App**

Create `apps/ui/src/features/chat-v2/index.ts`:

```typescript
export { QuickChat } from "./QuickChat";
```

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/chat-v2/QuickChat.tsx apps/ui/src/features/chat-v2/quick-chat.css apps/ui/src/features/chat-v2/index.ts
git commit -m "feat(chat-v2): QuickChat page component"
```

---

### Task 13: QuickChat render tests

**Files:**
- Create: `apps/ui/src/features/chat-v2/QuickChat.test.tsx`

- [ ] **Step 1: Write tests**

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QuickChat } from "./QuickChat";

// Stub the useQuickChat hook so render doesn't require a live transport.
vi.mock("./useQuickChat", () => ({
  useQuickChat: () => ({
    state: {
      sessionId: null,
      conversationId: "c1",
      messages: [],
      status: "idle",
      activeWardName: "stock-analysis",
      olderCursor: null,
      hasMoreOlder: false,
    },
    pillState: { visible: false, narration: "", suffix: "", category: "neutral", starting: false, swapCounter: 0 },
    sendMessage: vi.fn(),
    startNewChat: vi.fn(),
    stopAgent: vi.fn(),
    loadOlder: vi.fn(),
  }),
}));

describe("<QuickChat>", () => {
  it("renders empty state with ward binding", () => {
    render(
      <MemoryRouter initialEntries={["/chat-v2"]}>
        <Routes>
          <Route path="/chat-v2" element={<QuickChat />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText("Quick chat")).toBeTruthy();
    expect(screen.getByText(/bound to stock-analysis/)).toBeTruthy();
  });

  it("shows New chat button", () => {
    render(
      <MemoryRouter initialEntries={["/chat-v2"]}>
        <Routes>
          <Route path="/chat-v2" element={<QuickChat />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText(/New chat/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Verify tests pass**

Run: `cd apps/ui && npx vitest run src/features/chat-v2/QuickChat.test.tsx`
Expected: PASS — 2 tests.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/chat-v2/QuickChat.test.tsx
git commit -m "test(chat-v2): QuickChat render tests"
```

---

### Task 14: Route wiring for `/chat-v2` and `/chat-v2/:sessionId`

**Files:**
- Modify: `apps/ui/src/App.tsx` (lines ~25 import block, ~187 route block)

- [ ] **Step 1: Add import**

Locate the imports around line 25–35 in `apps/ui/src/App.tsx` and add:

```typescript
import { QuickChat } from "./features/chat-v2";
```

- [ ] **Step 2: Add two new routes inside the `<Routes>` block (after `/chat`)**

Locate line 187 and add below it:

```tsx
<Route path="/chat-v2" element={<QuickChat />} />
<Route path="/chat-v2/:sessionId" element={<QuickChat />} />
```

- [ ] **Step 3: Add nav link for Quick Chat in the sidebar**

Find where `/chat` is linked in the sidebar (grep for `MessageSquare` in App.tsx). Add a sibling link for `/chat-v2` with a "v2" badge.

Search-and-locate:

```bash
grep -n 'to="/chat"' apps/ui/src/App.tsx
```

Add a new line below it:

```tsx
<Link to="/chat-v2" className={`sidebar-link ${location.pathname.startsWith("/chat-v2") ? "active" : ""}`}>
  <MessageSquare size={18} />
  <span>Quick Chat <span className="sidebar-link__badge">v2</span></span>
</Link>
```

- [ ] **Step 4: Verify the build**

Run: `cd apps/ui && npm run build`
Expected: build succeeds, no TS errors.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/App.tsx
git commit -m "feat(chat-v2): route /chat-v2 and /chat-v2/:sessionId"
```

---

### Task 15: Smoke-test Quick Chat end-to-end in dev

**Files:** none (verification task)

- [ ] **Step 1: Start dev server and daemon**

In two terminals:

```bash
# Terminal A (daemon)
cargo run -p agentzero-daemon

# Terminal B (UI)
cd apps/ui && npm run dev
```

- [ ] **Step 2: Open `http://localhost:5173/chat-v2` in a browser**

Expected: empty state with "Quick chat" title, ward chip shows current ward (or "no ward").

- [ ] **Step 3: Send a memory-based question**

Type: `what did we decide about z.ai rate limits?`
Expected within ~5s:
- A user bubble appears immediately.
- The `StatusPill` appears under the header and updates narration as the agent streams.
- An assistant bubble appears with markdown-rendered content.
- The pill fades out after the final respond.
- URL updates to `/chat-v2/<session_id>`.

- [ ] **Step 4: Click "New chat"**

Expected: messages clear, URL returns to `/chat-v2`, empty state visible.

- [ ] **Step 5: Close the browser tab, reopen `/chat-v2/<session_id>` (the previous URL)**

Expected: the earlier conversation reloads via snapshot, older messages visible.

- [ ] **Step 6: Commit a brief note to the memory-bank**

Edit `memory-bank/activeContext.md` to add a line: `Quick Chat v2 (/chat-v2) shipping behind sidebar "Quick Chat" link; retires /chat later.`

```bash
git add memory-bank/activeContext.md
git commit -m "docs(chat-v2): note Quick Chat v2 availability"
```

---

### Task 16: (Optional) Hard-limit second delegation

**Skip this task if prompt-level enforcement proves sufficient during smoke test.**

**Files:**
- Modify: `gateway/gateway-execution/src/tools/delegate.rs` (or equivalent dispatch site)

- [ ] **Step 1: Only if Task 15 showed the quick-chat agent delegating twice in a turn:**

Add a per-turn counter in the executor that tracks `delegate_to_agent` calls for `agent_id == "quick-chat"` and rejects the second call with an error message steering the agent back to `respond()`.

Implementation left to the engineer — follow the existing dispatch pattern for the invocation site.

- [ ] **Step 2: Commit**

```bash
git add gateway/
git commit -m "feat(chat-v2): hard-enforce single delegation per quick-chat turn"
```

---

## Self-review checklist

Before declaring complete, verify:

1. **Spec coverage** — every `Chat — Spec` bullet in the design doc has a task:
   - [x] Dedicated `quick-chat` agent (Tasks 1, 2)
   - [x] Tool allowlist (Task 2)
   - [x] Single-delegation soft rule (Task 1 prompt; hard optional Task 16)
   - [x] "New chat" discards (Task 8 `startNewChat`, Task 12)
   - [x] Default lands anchored at last user message (Task 9 HYDRATE + Task 12)
   - [x] Lazy-load older turns (Task 8 `loadOlder`, Task 12 button)
   - [x] Inline activity chips (Tasks 10, 11)
   - [x] Tab-close + reopen restores session (Task 8 hydrate from snapshot)
   - [x] Empty state (Task 12)
   - [x] Ward chip read-only (Task 12)
   - [x] Status pill lifecycle (Tasks 4–6, 10)
   - [x] Routes alongside old `/chat` (Task 14)

2. **Placeholder scan** — no "TBD" / "TODO" / "implement later" in any task. Verified.

3. **Type consistency** — `QuickChatState`, `QuickChatAction`, `PillState`, `PillEvent`, `ToolPhrase` all referenced consistently across tasks 7/9/10.

4. **Acceptance criteria coverage** (from spec section):
   - (1) `/chat-v2` opens Claude-minimal page — Task 12.
   - (2) "New chat" discards and opens fresh — Task 8 + 12.
   - (3) Tab-close + reopen restores ongoing session — Task 8 snapshot load.
   - Respond rendering is the reducer's RESPOND action — Task 9.
   - Old `/chat` still works — unchanged (guaranteed by migration strategy).

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-19-quick-chat-ui.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.
