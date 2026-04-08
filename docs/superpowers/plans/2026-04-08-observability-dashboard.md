# Observability Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `/logs` page with a full observability dashboard using List+Detail split layout and Timeline Tree visualization showing root → subagent → tool call hierarchy.

**Architecture:** Pure UI change. New components in `apps/ui/src/features/logs/`. Data from existing HTTP endpoints (`/api/logs/sessions`) + WebSocket subscriptions (`scope: "all"`) for real-time. Existing transport types (`LogSession`, `SessionDetail`, `ExecutionLog`) used as-is. CSS in `apps/ui/src/styles/components.css` following BEM conventions with design tokens.

**Tech Stack:** React, TypeScript, CSS (BEM + design tokens), lucide-react icons

---

### Task 1: Add CSS Styles for Observability Dashboard

**Files:**
- Modify: `apps/ui/src/styles/components.css` (append new styles at end)

- [ ] **Step 1: Add observability dashboard styles**

Append to the end of `apps/ui/src/styles/components.css`:

```css
/* ============================================================================
   OBSERVABILITY DASHBOARD
   ============================================================================ */

.obs-dashboard {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.obs-dashboard__kpi-bar {
  display: flex;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-4);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
  font-size: var(--text-xs);
  color: var(--muted-foreground);
}

.obs-dashboard__kpi-stat {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
}

.obs-dashboard__kpi-value {
  font-weight: 700;
  color: var(--foreground);
}

.obs-dashboard__kpi-value--success {
  color: var(--success);
}

.obs-dashboard__body {
  display: flex;
  flex: 1;
  overflow: hidden;
}

/* Session List (left panel) */

.session-list {
  width: 300px;
  flex-shrink: 0;
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.session-list__filter {
  padding: var(--spacing-2);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

.session-list__filter-input {
  width: 100%;
  padding: var(--spacing-1) var(--spacing-2);
  background: var(--muted);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font-size: var(--text-xs);
  outline: none;
}

.session-list__filter-input:focus {
  border-color: var(--primary);
}

.session-list__items {
  flex: 1;
  overflow-y: auto;
}

.session-list-item {
  padding: var(--spacing-2) var(--spacing-3);
  border-bottom: 1px solid var(--border);
  cursor: pointer;
  transition: background-color 0.15s;
}

.session-list-item:hover {
  background: var(--muted);
}

.session-list-item--selected {
  background: var(--card);
  border-left: 3px solid var(--primary);
}

.session-list-item__title {
  font-weight: 600;
  font-size: var(--text-sm);
  color: var(--foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.session-list-item__meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 2px;
  font-size: 11px;
  color: var(--muted-foreground);
}

.session-list-item__status {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: var(--radius-full);
  flex-shrink: 0;
}

.session-list-item__status--completed { background: var(--success); }
.session-list-item__status--running { background: var(--primary); animation: pulse 2s infinite; }
.session-list-item__status--error { background: var(--destructive); }
.session-list-item__status--crashed { background: var(--destructive); }
.session-list-item__status--stopped { background: var(--warning); }
.session-list-item__status--paused { background: var(--warning); }

/* Trace Timeline (right panel) */

.trace-timeline {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-3) var(--spacing-4);
}

.trace-timeline__header {
  margin-bottom: var(--spacing-3);
}

.trace-timeline__title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--foreground);
}

.trace-timeline__subtitle {
  font-size: var(--text-xs);
  color: var(--muted-foreground);
  margin-top: 2px;
}

.trace-timeline__empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--muted-foreground);
  font-size: var(--text-sm);
}

.trace-timeline__tree {
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.8;
}

/* Trace Nodes */

.trace-node {
  cursor: pointer;
  padding: 1px 4px;
  border-radius: var(--radius-sm);
  transition: background-color 0.1s;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trace-node:hover {
  background: var(--muted);
}

.trace-node__icon {
  display: inline-block;
  width: 14px;
  text-align: center;
  margin-right: 4px;
}

.trace-node__agent {
  font-weight: 600;
}

.trace-node__tool {
  color: var(--muted-foreground);
}

.trace-node__summary {
  color: var(--muted-foreground);
  font-size: 12px;
}

.trace-node__duration {
  color: var(--muted-foreground);
  font-size: 11px;
  margin-left: 8px;
}

.trace-node--error .trace-node__tool,
.trace-node--error .trace-node__duration {
  color: var(--destructive);
}

.trace-node--delegation .trace-node__agent {
  color: var(--primary);
}

/* Trace Node Detail (expanded) */

.trace-node-detail {
  margin: var(--spacing-1) 0;
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--muted);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 300px;
  overflow-y: auto;
}

.trace-node-detail__label {
  font-weight: 600;
  color: var(--foreground);
  margin-bottom: var(--spacing-1);
  font-family: var(--font-sans);
  text-transform: uppercase;
  font-size: 10px;
  letter-spacing: 0.5px;
}

.trace-node-detail__content {
  color: var(--muted-foreground);
  font-family: var(--font-mono);
}

.trace-node-detail__section {
  margin-bottom: var(--spacing-2);
}

.trace-node-detail__section:last-child {
  margin-bottom: 0;
}
```

- [ ] **Step 2: Verify CSS doesn't break existing styles**

Run: `cd apps/ui && npm run build`
Expected: builds successfully

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/styles/components.css
git commit -m "style: add observability dashboard CSS classes"
```

---

### Task 2: Create Trace Data Types and Utility Functions

**Files:**
- Create: `apps/ui/src/features/logs/trace-types.ts`

- [ ] **Step 1: Create trace data types**

```typescript
// apps/ui/src/features/logs/trace-types.ts

/** A node in the trace timeline tree. */
export interface TraceNode {
  /** Unique ID (execution log ID or generated) */
  id: string;
  /** Node type determines rendering */
  type: "root" | "delegation" | "tool_call" | "error";
  /** Agent that performed this action */
  agentId: string;
  /** Display label (tool name, agent name, etc.) */
  label: string;
  /** Short summary (command, file path, task description) */
  summary?: string;
  /** Duration in milliseconds */
  durationMs?: number;
  /** Token count (for agents/delegations) */
  tokenCount?: number;
  /** Status */
  status?: "running" | "completed" | "error" | "crashed";
  /** Error message if failed */
  error?: string;
  /** Timestamp */
  timestamp: string;
  /** Full tool arguments (shown on expand) */
  args?: string;
  /** Full tool result (shown on expand) */
  result?: string;
  /** Child nodes (tool calls under a delegation, etc.) */
  children: TraceNode[];
}

/** Execution lookup map: execution_id → agent info */
export interface ExecutionEntry {
  agentId: string;
  task?: string;
  executionId: string;
}

/** Internal tools to filter out of the tree */
const INTERNAL_TOOLS = new Set([
  "analyze_intent",
  "update_plan",
  "set_session_title",
]);

/** Check if a tool should be hidden from the trace */
export function isInternalTool(toolName: string): boolean {
  return INTERNAL_TOOLS.has(toolName);
}

/** Format milliseconds to human-readable duration */
export function formatDuration(ms: number | undefined): string {
  if (ms === undefined || ms === null) return "";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60000);
  const secs = Math.round((ms % 60000) / 1000);
  return `${mins}m ${secs}s`;
}

/** Format token count to shorthand */
export function formatTokens(n: number | undefined): string {
  if (n === undefined || n === null || n === 0) return "";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M tok`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k tok`;
  return `${n} tok`;
}

/** Extract tool name summary from args (e.g., shell command, file path) */
export function extractToolSummary(
  toolName: string,
  args?: string
): string {
  if (!args) return "";
  try {
    const parsed = JSON.parse(args);
    if (toolName === "shell" && parsed.command) {
      const cmd = String(parsed.command);
      return cmd.length > 60 ? cmd.slice(0, 57) + "..." : cmd;
    }
    if ((toolName === "read" || toolName === "edit" || toolName === "write") && parsed.path) {
      return String(parsed.path);
    }
    if (toolName === "grep" && parsed.pattern) {
      return `/${parsed.pattern}/`;
    }
    if (toolName === "glob" && parsed.pattern) {
      return String(parsed.pattern);
    }
    if (toolName === "web_fetch" && parsed.url) {
      return String(parsed.url).slice(0, 60);
    }
    if (toolName === "respond" && parsed.message) {
      const msg = String(parsed.message);
      return msg.length > 50 ? msg.slice(0, 47) + "..." : msg;
    }
  } catch {
    // Not valid JSON, return empty
  }
  return "";
}
```

- [ ] **Step 2: Verify build**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: no type errors

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/trace-types.ts
git commit -m "feat: add trace tree data types and utility functions"
```

---

### Task 3: Create TraceNodeDetail Component

**Files:**
- Create: `apps/ui/src/features/logs/TraceNodeDetail.tsx`

- [ ] **Step 1: Create the expanded detail component**

```tsx
// apps/ui/src/features/logs/TraceNodeDetail.tsx
import type { TraceNode } from "./trace-types";

interface TraceNodeDetailProps {
  node: TraceNode;
}

export function TraceNodeDetail({ node }: TraceNodeDetailProps) {
  return (
    <div className="trace-node-detail">
      {node.args && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Arguments</div>
          <div className="trace-node-detail__content">{formatArgs(node.args)}</div>
        </div>
      )}
      {node.result && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Result</div>
          <div className="trace-node-detail__content">{truncateResult(node.result)}</div>
        </div>
      )}
      {node.error && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Error</div>
          <div className="trace-node-detail__content" style={{ color: "var(--destructive)" }}>
            {node.error}
          </div>
        </div>
      )}
      {node.type === "delegation" && node.summary && (
        <div className="trace-node-detail__section">
          <div className="trace-node-detail__label">Task</div>
          <div className="trace-node-detail__content">{node.summary}</div>
        </div>
      )}
    </div>
  );
}

function formatArgs(args: string): string {
  try {
    const parsed = JSON.parse(args);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return args;
  }
}

function truncateResult(result: string): string {
  if (result.length <= 2000) return result;
  return result.slice(0, 1000) + "\n\n--- truncated ---\n\n" + result.slice(-500);
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/logs/TraceNodeDetail.tsx
git commit -m "feat: add TraceNodeDetail component for expanded node view"
```

---

### Task 4: Create TraceNode Component

**Files:**
- Create: `apps/ui/src/features/logs/TraceNodeComponent.tsx`

- [ ] **Step 1: Create the tree node component**

```tsx
// apps/ui/src/features/logs/TraceNodeComponent.tsx
import { useState } from "react";
import type { TraceNode } from "./trace-types";
import { formatDuration, formatTokens } from "./trace-types";
import { TraceNodeDetail } from "./TraceNodeDetail";

interface TraceNodeComponentProps {
  node: TraceNode;
  depth: number;
}

export function TraceNodeComponent({ node, depth }: TraceNodeComponentProps) {
  const [expanded, setExpanded] = useState(false);
  const [childrenCollapsed, setChildrenCollapsed] = useState(false);

  const isDelegation = node.type === "delegation";
  const isError = node.status === "error" || node.status === "crashed" || !!node.error;
  const hasChildren = node.children.length > 0;

  const nodeClass = [
    "trace-node",
    isError ? "trace-node--error" : "",
    isDelegation ? "trace-node--delegation" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const icon = isDelegation ? (hasChildren && !childrenCollapsed ? "▼" : "▶") : "●";
  const iconColor = isDelegation
    ? undefined
    : isError
      ? "var(--destructive)"
      : "var(--muted-foreground)";

  function handleClick() {
    if (isDelegation && hasChildren) {
      setChildrenCollapsed(!childrenCollapsed);
    } else {
      setExpanded(!expanded);
    }
  }

  function handleDoubleClick() {
    setExpanded(!expanded);
  }

  return (
    <div>
      <div
        className={nodeClass}
        style={{ paddingLeft: `${depth * 20}px` }}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
      >
        <span className="trace-node__icon" style={iconColor ? { color: iconColor } : undefined}>
          {icon}
        </span>
        {isDelegation || node.type === "root" ? (
          <span className="trace-node__agent">{node.label}</span>
        ) : (
          <span className="trace-node__tool">{node.label}</span>
        )}
        {node.summary && (
          <span className="trace-node__summary">
            {" "}
            — {node.summary}
          </span>
        )}
        <span className="trace-node__duration">
          {formatDuration(node.durationMs)}
          {node.tokenCount ? ` · ${formatTokens(node.tokenCount)}` : ""}
        </span>
      </div>

      {expanded && (
        <div style={{ paddingLeft: `${depth * 20}px` }}>
          <TraceNodeDetail node={node} />
        </div>
      )}

      {hasChildren && !childrenCollapsed &&
        node.children.map((child) => (
          <TraceNodeComponent key={child.id} node={child} depth={depth + 1} />
        ))}
    </div>
  );
}
```

- [ ] **Step 2: Verify build**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: no type errors

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/TraceNodeComponent.tsx
git commit -m "feat: add TraceNodeComponent for timeline tree rendering"
```

---

### Task 5: Create TraceTimeline Component

**Files:**
- Create: `apps/ui/src/features/logs/TraceTimeline.tsx`

- [ ] **Step 1: Create the timeline panel**

```tsx
// apps/ui/src/features/logs/TraceTimeline.tsx
import type { TraceNode } from "./trace-types";
import { formatDuration, formatTokens } from "./trace-types";
import { TraceNodeComponent } from "./TraceNodeComponent";

interface TraceTimelineProps {
  /** Root trace node (null if no session selected) */
  trace: TraceNode | null;
  /** Whether the trace is still loading */
  loading: boolean;
}

export function TraceTimeline({ trace, loading }: TraceTimelineProps) {
  if (loading) {
    return (
      <div className="trace-timeline">
        <div className="trace-timeline__empty">
          <span className="loading-spinner" />
        </div>
      </div>
    );
  }

  if (!trace) {
    return (
      <div className="trace-timeline">
        <div className="trace-timeline__empty">Select a session to view its trace</div>
      </div>
    );
  }

  return (
    <div className="trace-timeline">
      <div className="trace-timeline__header">
        <div className="trace-timeline__title">
          {trace.summary || trace.label}
        </div>
        <div className="trace-timeline__subtitle">
          {formatDuration(trace.durationMs)} · {formatTokens(trace.tokenCount)} · {trace.status}
        </div>
      </div>
      <div className="trace-timeline__tree">
        <TraceNodeComponent node={trace} depth={0} />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/ui/src/features/logs/TraceTimeline.tsx
git commit -m "feat: add TraceTimeline component"
```

---

### Task 6: Create SessionListItem and SessionList Components

**Files:**
- Create: `apps/ui/src/features/logs/SessionListItem.tsx`
- Create: `apps/ui/src/features/logs/SessionList.tsx`

- [ ] **Step 1: Create SessionListItem**

```tsx
// apps/ui/src/features/logs/SessionListItem.tsx
import type { LogSession } from "../../services/transport/types";
import { formatDuration, formatTokens } from "./trace-types";

interface SessionListItemProps {
  session: LogSession;
  isSelected: boolean;
  onClick: () => void;
}

export function SessionListItem({ session, isSelected, onClick }: SessionListItemProps) {
  const title = session.title || session.agent_name || session.agent_id;
  const statusClass = `session-list-item__status session-list-item__status--${session.status}`;
  const itemClass = isSelected
    ? "session-list-item session-list-item--selected"
    : "session-list-item";

  const agentCount = session.child_session_ids?.length || 0;

  return (
    <div className={itemClass} onClick={onClick}>
      <div className="session-list-item__title">{title}</div>
      <div className="session-list-item__meta">
        <span>
          <span className={statusClass} />{" "}
          {agentCount > 0 ? `${agentCount} agent${agentCount > 1 ? "s" : ""}` : "direct"}
          {session.duration_ms ? ` · ${formatDuration(session.duration_ms)}` : ""}
        </span>
        <span>{formatTokens(session.token_count)}</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create SessionList**

```tsx
// apps/ui/src/features/logs/SessionList.tsx
import { useState, useMemo } from "react";
import type { LogSession } from "../../services/transport/types";
import { SessionListItem } from "./SessionListItem";

interface SessionListProps {
  sessions: LogSession[];
  selectedId: string | null;
  onSelect: (sessionId: string) => void;
  loading: boolean;
}

export function SessionList({ sessions, selectedId, onSelect, loading }: SessionListProps) {
  const [filter, setFilter] = useState("");

  const filtered = useMemo(() => {
    if (!filter) return sessions;
    const lower = filter.toLowerCase();
    return sessions.filter(
      (s) =>
        (s.title || "").toLowerCase().includes(lower) ||
        s.agent_id.toLowerCase().includes(lower) ||
        s.agent_name.toLowerCase().includes(lower)
    );
  }, [sessions, filter]);

  return (
    <div className="session-list">
      <div className="session-list__filter">
        <input
          className="session-list__filter-input"
          type="text"
          placeholder="Filter sessions..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      <div className="session-list__items">
        {loading && filtered.length === 0 && (
          <div style={{ padding: "var(--spacing-4)", textAlign: "center" }}>
            <span className="loading-spinner" />
          </div>
        )}
        {!loading && filtered.length === 0 && (
          <div style={{ padding: "var(--spacing-4)", textAlign: "center", color: "var(--muted-foreground)", fontSize: "var(--text-sm)" }}>
            No sessions found
          </div>
        )}
        {filtered.map((session) => (
          <SessionListItem
            key={session.session_id}
            session={session}
            isSelected={session.session_id === selectedId}
            onClick={() => onSelect(session.session_id)}
          />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Verify build**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: no type errors

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/logs/SessionListItem.tsx apps/ui/src/features/logs/SessionList.tsx
git commit -m "feat: add SessionList and SessionListItem components"
```

---

### Task 7: Create useSessionTrace Hook

**Files:**
- Create: `apps/ui/src/features/logs/useSessionTrace.ts`

This hook fetches session detail via HTTP and builds the trace tree from log entries. It also merges real-time WebSocket events.

- [ ] **Step 1: Create the hook**

```typescript
// apps/ui/src/features/logs/useSessionTrace.ts
import { useState, useEffect, useCallback, useRef } from "react";
import { useTransport } from "../../services/transport/context";
import type { SessionDetail, ExecutionLog } from "../../services/transport/types";
import type { TraceNode, ExecutionEntry } from "./trace-types";
import { isInternalTool, extractToolSummary } from "./trace-types";

interface UseSessionTraceResult {
  trace: TraceNode | null;
  loading: boolean;
}

export function useSessionTrace(sessionId: string | null): UseSessionTraceResult {
  const transport = useTransport();
  const [trace, setTrace] = useState<TraceNode | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!sessionId) {
      setTrace(null);
      return;
    }

    let cancelled = false;
    setLoading(true);

    async function fetchTrace() {
      try {
        // Fetch root session detail
        const rootResult = await transport.getLogSession(sessionId!);
        if (cancelled || !rootResult.success || !rootResult.data) return;

        const rootDetail = rootResult.data;
        const childSessionIds = rootDetail.session.child_session_ids || [];

        // Fetch all child session details in parallel
        const childDetails: SessionDetail[] = [];
        const childPromises = childSessionIds.map(async (childId) => {
          const result = await transport.getLogSession(childId);
          if (result.success && result.data) {
            childDetails.push(result.data);
          }
        });
        await Promise.all(childPromises);

        if (cancelled) return;

        // Build the trace tree
        const tree = buildTraceTree(rootDetail, childDetails);
        setTrace(tree);
      } catch (err) {
        console.error("Failed to fetch session trace:", err);
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    fetchTrace();
    return () => { cancelled = true; };
  }, [sessionId, transport]);

  return { trace, loading };
}

/** Build a TraceNode tree from root + child session details. */
function buildTraceTree(
  rootDetail: SessionDetail,
  childDetails: SessionDetail[]
): TraceNode {
  const rootSession = rootDetail.session;
  const rootLogs = rootDetail.logs;

  // Build child session lookup: agent_id → SessionDetail
  const childByAgent = new Map<string, SessionDetail>();
  for (const child of childDetails) {
    childByAgent.set(child.session.agent_id, child);
  }

  // Build root node
  const rootNode: TraceNode = {
    id: rootSession.session_id,
    type: "root",
    agentId: rootSession.agent_id,
    label: rootSession.agent_name || rootSession.agent_id,
    summary: rootSession.title,
    durationMs: rootSession.duration_ms,
    tokenCount: rootSession.token_count,
    status: mapStatus(rootSession.status),
    timestamp: rootSession.started_at,
    children: [],
  };

  // Process root logs in order
  for (const log of rootLogs) {
    if (log.category === "tool_call") {
      const toolName = extractMetaField(log, "tool_name") || log.message;
      if (isInternalTool(toolName)) continue;

      const toolArgs = extractMetaField(log, "args");
      const matchingResult = findMatchingResult(rootLogs, log);

      rootNode.children.push({
        id: log.id,
        type: "tool_call",
        agentId: rootSession.agent_id,
        label: toolName,
        summary: extractToolSummary(toolName, toolArgs),
        durationMs: matchingResult?.duration_ms ?? log.duration_ms,
        status: matchingResult?.level === "error" ? "error" : "completed",
        error: matchingResult?.level === "error" ? matchingResult.message : undefined,
        timestamp: log.timestamp,
        args: toolArgs,
        result: matchingResult ? extractMetaField(matchingResult, "result") : undefined,
        children: [],
      });
    } else if (log.category === "delegation") {
      const childAgentId = extractMetaField(log, "child_agent_id") || "";
      const task = extractMetaField(log, "task") || log.message;
      const childDetail = childByAgent.get(childAgentId);

      const delegationNode: TraceNode = {
        id: log.id,
        type: "delegation",
        agentId: childAgentId,
        label: childAgentId,
        summary: task,
        durationMs: childDetail?.session.duration_ms ?? log.duration_ms,
        tokenCount: childDetail?.session.token_count,
        status: childDetail ? mapStatus(childDetail.session.status) : "completed",
        timestamp: log.timestamp,
        children: [],
      };

      // Add child session's tool calls as children
      if (childDetail) {
        for (const childLog of childDetail.logs) {
          if (childLog.category === "tool_call") {
            const toolName = extractMetaField(childLog, "tool_name") || childLog.message;
            if (isInternalTool(toolName)) continue;

            const toolArgs = extractMetaField(childLog, "args");
            const matchingResult = findMatchingResult(childDetail.logs, childLog);

            delegationNode.children.push({
              id: childLog.id,
              type: "tool_call",
              agentId: childAgentId,
              label: toolName,
              summary: extractToolSummary(toolName, toolArgs),
              durationMs: matchingResult?.duration_ms ?? childLog.duration_ms,
              status: matchingResult?.level === "error" ? "error" : "completed",
              error: matchingResult?.level === "error" ? matchingResult.message : undefined,
              timestamp: childLog.timestamp,
              args: toolArgs,
              result: matchingResult ? extractMetaField(matchingResult, "result") : undefined,
              children: [],
            });
          } else if (childLog.category === "error") {
            delegationNode.children.push({
              id: childLog.id,
              type: "error",
              agentId: childAgentId,
              label: "error",
              summary: childLog.message,
              error: childLog.message,
              timestamp: childLog.timestamp,
              children: [],
            });
          }
        }
      }

      rootNode.children.push(delegationNode);
    } else if (log.category === "error") {
      rootNode.children.push({
        id: log.id,
        type: "error",
        agentId: rootSession.agent_id,
        label: "error",
        summary: log.message,
        error: log.message,
        timestamp: log.timestamp,
        children: [],
      });
    }
  }

  return rootNode;
}

function extractMetaField(log: ExecutionLog, field: string): string | undefined {
  if (!log.metadata) return undefined;
  const value = log.metadata[field];
  if (value === undefined || value === null) return undefined;
  if (typeof value === "string") return value;
  return JSON.stringify(value);
}

function findMatchingResult(logs: ExecutionLog[], toolCallLog: ExecutionLog): ExecutionLog | undefined {
  const toolId = extractMetaField(toolCallLog, "tool_id");
  if (!toolId) return undefined;
  return logs.find(
    (l) => l.category === "tool_result" && extractMetaField(l, "tool_id") === toolId
  );
}

function mapStatus(status: string): "running" | "completed" | "error" | "crashed" {
  if (status === "running") return "running";
  if (status === "completed") return "completed";
  if (status === "crashed") return "crashed";
  return "error";
}
```

- [ ] **Step 2: Verify build**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: no type errors. Note: if `useTransport` import path differs, check `apps/ui/src/services/transport/context.ts` or `context.tsx` for the actual export.

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/useSessionTrace.ts
git commit -m "feat: add useSessionTrace hook for building trace tree from logs"
```

---

### Task 8: Create useTraceSubscription Hook

**Files:**
- Create: `apps/ui/src/features/logs/useTraceSubscription.ts`

This hook manages WebSocket subscription for real-time updates on running sessions.

- [ ] **Step 1: Create the hook**

```typescript
// apps/ui/src/features/logs/useTraceSubscription.ts
import { useEffect, useRef } from "react";
import { useTransport } from "../../services/transport/context";
import type { LogSession } from "../../services/transport/types";

interface UseTraceSubscriptionOptions {
  /** The session to subscribe to (null = no subscription) */
  session: LogSession | null;
  /** Callback when any event arrives (triggers trace refetch) */
  onEvent: () => void;
}

/**
 * Subscribes to WebSocket events for a running session.
 * Automatically unsubscribes on session change or when session completes.
 * Only subscribes if session status is "running".
 */
export function useTraceSubscription({ session, onEvent }: UseTraceSubscriptionOptions) {
  const transport = useTransport();
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    if (!session || session.status !== "running") return;

    const sessionId = session.session_id;

    // Subscribe with scope "all" to see subagent events
    const unsubscribe = transport.subscribeConversation(sessionId, {
      scope: "all",
      onMessage: () => {
        // Any event triggers a trace rebuild
        onEventRef.current();
      },
    });

    return () => {
      unsubscribe();
    };
  }, [session?.session_id, session?.status, transport]);
}
```

**Note:** The exact `subscribeConversation` API may differ. The implementer should check `apps/ui/src/services/transport/http.ts` for the actual method signature and callback shape. The key behavior is: subscribe to the session with `scope: "all"`, call `onEvent` on any incoming message, and unsubscribe on cleanup. If the transport API uses a different method name or callback pattern, adapt accordingly.

- [ ] **Step 2: Verify build**

Run: `cd apps/ui && npx tsc --noEmit`
Expected: no type errors (may need to adapt to actual transport API)

- [ ] **Step 3: Commit**

```bash
git add apps/ui/src/features/logs/useTraceSubscription.ts
git commit -m "feat: add useTraceSubscription hook for real-time WebSocket updates"
```

---

### Task 9: Create ObservabilityDashboard and Wire Up Route

**Files:**
- Create: `apps/ui/src/features/logs/ObservabilityDashboard.tsx`
- Modify: `apps/ui/src/features/logs/WebLogsPanel.tsx`

- [ ] **Step 1: Create the main dashboard component**

```tsx
// apps/ui/src/features/logs/ObservabilityDashboard.tsx
import { useState, useMemo, useCallback } from "react";
import type { LogSession } from "../../services/transport/types";
import { useLogSessions, useAutoRefresh } from "./log-hooks";
import { useSessionTrace } from "./useSessionTrace";
import { useTraceSubscription } from "./useTraceSubscription";
import { SessionList } from "./SessionList";
import { TraceTimeline } from "./TraceTimeline";
import { formatTokens } from "./trace-types";

export function ObservabilityDashboard() {
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const { sessions, loading: sessionsLoading, refetch } = useLogSessions({ root_only: true });

  // Auto-refresh while any session is running
  useAutoRefresh(sessions, refetch);

  // Build trace for selected session
  const { trace, loading: traceLoading } = useSessionTrace(selectedSessionId);

  // Find selected session object for subscription check
  const selectedSession = useMemo(
    () => sessions.find((s) => s.session_id === selectedSessionId) || null,
    [sessions, selectedSessionId]
  );

  // Real-time subscription for running sessions
  useTraceSubscription({
    session: selectedSession,
    onEvent: useCallback(() => {
      // Refetch trace on new events
      // useSessionTrace will re-run when sessionId reference changes,
      // but we need a way to trigger refetch for same session.
      // Simple approach: refetch the session list (which triggers auto-refresh)
      refetch();
    }, [refetch]),
  });

  // KPI aggregates
  const kpis = useMemo(() => {
    const total = sessions.length;
    const completed = sessions.filter((s) => s.status === "completed").length;
    const successRate = total > 0 ? Math.round((completed / total) * 100) : 0;
    const totalTokens = sessions.reduce((sum, s) => sum + (s.token_count || 0), 0);
    const avgDuration = total > 0
      ? Math.round(sessions.reduce((sum, s) => sum + (s.duration_ms || 0), 0) / total)
      : 0;
    return { total, successRate, totalTokens, avgDuration };
  }, [sessions]);

  return (
    <div className="obs-dashboard">
      <div className="obs-dashboard__kpi-bar">
        <div className="obs-dashboard__kpi-stat">
          <span className="obs-dashboard__kpi-value">{kpis.total}</span> sessions
        </div>
        <div className="obs-dashboard__kpi-stat">
          <span className={`obs-dashboard__kpi-value${kpis.successRate >= 80 ? " obs-dashboard__kpi-value--success" : ""}`}>
            {kpis.successRate}%
          </span> success
        </div>
        <div className="obs-dashboard__kpi-stat">
          <span className="obs-dashboard__kpi-value">{formatTokens(kpis.totalTokens)}</span> total
        </div>
        <div className="obs-dashboard__kpi-stat">
          <span className="obs-dashboard__kpi-value">{formatAvgDuration(kpis.avgDuration)}</span> avg
        </div>
      </div>
      <div className="obs-dashboard__body">
        <SessionList
          sessions={sessions}
          selectedId={selectedSessionId}
          onSelect={setSelectedSessionId}
          loading={sessionsLoading}
        />
        <TraceTimeline trace={trace} loading={traceLoading} />
      </div>
    </div>
  );
}

function formatAvgDuration(ms: number): string {
  if (ms === 0) return "—";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60000);
  const secs = Math.round((ms % 60000) / 1000);
  return `${mins}m ${secs}s`;
}
```

- [ ] **Step 2: Update WebLogsPanel to use ObservabilityDashboard**

Replace the content of `apps/ui/src/features/logs/WebLogsPanel.tsx` with:

```tsx
import { ObservabilityDashboard } from "./ObservabilityDashboard";

export function WebLogsPanel() {
  return <ObservabilityDashboard />;
}
```

- [ ] **Step 3: Verify full build**

Run: `cd apps/ui && npm run build`
Expected: builds successfully

- [ ] **Step 4: Commit**

```bash
git add apps/ui/src/features/logs/ObservabilityDashboard.tsx apps/ui/src/features/logs/WebLogsPanel.tsx
git commit -m "feat: add ObservabilityDashboard and wire up to /logs route"
```

---

### Task 10: Integration Verification and Polish

- [ ] **Step 1: Run full UI build**

Run: `cd apps/ui && npm run build`
Expected: builds successfully with no errors

- [ ] **Step 2: Manual testing checklist**

Start the app and verify:
1. Navigate to `/logs` — see the new split layout
2. KPI bar shows session count, success rate, tokens, avg duration
3. Left panel lists sessions with title, status dot, agent count
4. Clicking a session loads the timeline tree on the right
5. Timeline tree shows root → delegation → tool calls hierarchy
6. Tool calls show name + summary (command, file path)
7. Delegation nodes show agent name + task
8. Click a tool call node — detail expands showing args/result
9. Click a delegation node — collapses/expands its children
10. Error nodes show in red
11. Filter input in session list filters by title/agent name
12. If a running session exists, events update the tree in real-time

- [ ] **Step 3: Fix any issues found during testing**

Address build errors, missing imports, API differences, or rendering bugs.

- [ ] **Step 4: Final commit if needed**

```bash
git add -A
git commit -m "fix: address issues found during integration testing"
```
