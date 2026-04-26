// ============================================================================
// MISSION CONTROL — AgentToolGroup (recursive)
// One card per agent execution. Header carries the agent name + status +
// duration + tool count + sub-agent count. Body is the agent's tool calls
// plus any nested sub-agent groups (rendered recursively).
//
// Reads from the existing `TraceNode` tree built by useSessionTrace — no new
// data shape, no backend change. Tree shape:
//   root  → children: [tool_call, delegation, error, ...]
//   delegation → children: [tool_call, delegation (nested), ...]
//   tool_call → leaf
// ============================================================================

import { useState } from "react";
import type { TraceNode } from "../logs/trace-types";
import { formatDuration } from "../logs/trace-types";

interface AgentToolGroupProps {
  /** Either a `root` or `delegation` node — leaves render via ToolRow. */
  node: TraceNode;
  /** Recursion depth for indentation tier (0 = root, 1 = direct child, …). */
  depth?: number;
  /** Optional click handler — when provided, each tool row becomes clickable
   *  and fires this callback with the tool node so the parent can open a
   *  detail popover. */
  onToolClick?: (tool: TraceNode) => void;
  /** Optional per-agent token totals keyed by agent_id. When provided and the
   *  current node's agent has a non-zero entry, the header shows ↓in / ↑out
   *  in addition to duration + tool count. */
  tokensByAgent?: Map<string, { in: number; out: number }>;
  /** When true, the body starts collapsed; click the header to expand. The
   *  root group is always expanded — only delegations honor this. */
  defaultCollapsed?: boolean;
}

export function AgentToolGroup({ node, depth = 0, onToolClick, tokensByAgent, defaultCollapsed = false }: AgentToolGroupProps) {
  // Root is always expanded; only delegations can be collapsed by default.
  const [collapsed, setCollapsed] = useState<boolean>(node.type === "delegation" && defaultCollapsed);

  // Defensive: a tool_call leaf shouldn't be rendered here, but if it is,
  // degrade gracefully into a single row.
  if (node.type === "tool_call") return <ToolRow tool={node} onClick={onToolClick} />;

  const tools = node.children.filter((c) => c.type === "tool_call");
  const subagents = node.children.filter((c) => c.type === "delegation");
  const errors = node.children.filter((c) => c.type === "error");
  const status = mapStatus(node.status);
  const avatar = avatarTone(node.agentId, depth);
  const tokens = tokensByAgent?.get(node.agentId);
  const showTokens = tokens && (tokens.in + tokens.out) > 0;
  const collapsible = node.type === "delegation";

  return (
    <div
      className={`agent-tool-group agent-tool-group--depth-${Math.min(depth, 2)}${collapsed ? " agent-tool-group--collapsed" : ""}`}
      data-status={status}
    >
      <HeaderShell
        collapsible={collapsible}
        collapsed={collapsed}
        onToggle={() => setCollapsed((c) => !c)}
        label={node.label || node.agentId}
      >
        {collapsible && (
          <span
            className={`agent-tool-group__chevron agent-tool-group__chevron--${collapsed ? "right" : "down"}`}
            aria-hidden="true"
          >
            {collapsed ? "▸" : "▾"}
          </span>
        )}
        <span className={`agent-tool-group__avatar agent-tool-group__avatar--${avatar}`} aria-hidden="true">
          {avatarLabel(node.agentId)}
        </span>
        <span className="agent-tool-group__name">{node.label || node.agentId}</span>
        <span className={`agent-tool-group__status agent-tool-group__status--${status}`}>
          {status}
        </span>
        <span className="agent-tool-group__meta">
          {node.durationMs !== undefined && <span>{formatDuration(node.durationMs)}</span>}
          {showTokens && (
            <span className="agent-tool-group__tokens">
              <span title={`${tokens.in.toLocaleString()} input tokens`}>↓ {compactTokens(tokens.in)}</span>
              <span className="agent-tool-group__tokens-sep">/</span>
              <span title={`${tokens.out.toLocaleString()} output tokens`}>↑ {compactTokens(tokens.out)}</span>
            </span>
          )}
          {!showTokens && node.tokenCount !== undefined && node.tokenCount > 0 && (
            <span>{formatTokenCount(node.tokenCount)}</span>
          )}
          <span>{tools.length} tool{tools.length === 1 ? "" : "s"}</span>
          {subagents.length > 0 && <span>{subagents.length} ↳</span>}
        </span>
      </HeaderShell>

      {!collapsed && (
        <>
          {tools.length > 0 && (
            <div className="agent-tool-group__tools">
              {tools.map((t) => (
                <ToolRow key={t.id} tool={t} onClick={onToolClick} />
              ))}
            </div>
          )}

          {errors.length > 0 && (
            <div className="agent-tool-group__errors">
              {errors.map((e) => (
                <div key={e.id} className="agent-tool-group__error">
                  ⚠ {e.summary || e.error || "error"}
                </div>
              ))}
            </div>
          )}

          {subagents.length > 0 && (
            <div className="agent-tool-group__delegate-note">
              ↳ delegated {subagents.length} subagent{subagents.length === 1 ? "" : "s"}
            </div>
          )}

          {subagents.map((sub, i) => (
            <AgentToolGroup
              key={sub.id}
              node={sub}
              depth={depth + 1}
              onToolClick={onToolClick}
              tokensByAgent={tokensByAgent}
              defaultCollapsed={i !== subagents.length - 1}
            />
          ))}
        </>
      )}
    </div>
  );
}

interface HeaderShellProps {
  collapsible: boolean;
  collapsed: boolean;
  onToggle(): void;
  label: string;
  children: React.ReactNode;
}

function HeaderShell({ collapsible, collapsed, onToggle, label, children }: HeaderShellProps) {
  if (!collapsible) {
    return <header className="agent-tool-group__head">{children}</header>;
  }
  return (
    <button
      type="button"
      className="agent-tool-group__head agent-tool-group__head--clickable"
      onClick={onToggle}
      aria-expanded={!collapsed}
      aria-label={`${collapsed ? "Expand" : "Collapse"} ${label}`}
    >
      {children}
    </button>
  );
}

function compactTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

interface ToolRowProps {
  tool: TraceNode;
  onClick?: (tool: TraceNode) => void;
}

function ToolRow({ tool, onClick }: ToolRowProps) {
  const status = mapStatus(tool.status);
  const isPending = status === "running";
  const interactive = Boolean(onClick);

  const inner = (
    <>
      <span className="agent-tool-group__row-name">
        {tool.label}
        {tool.summary && <code className="agent-tool-group__row-arg">{tool.summary}</code>}
      </span>
      <span className="agent-tool-group__row-status">
        {isPending && <span className="agent-tool-group__row-pending" aria-label="In flight">▊</span>}
        {!isPending && status === "completed" && (
          <span className="agent-tool-group__row-check">
            ✓ {tool.durationMs !== undefined ? formatDuration(tool.durationMs) : "ok"}
          </span>
        )}
        {!isPending && (status === "failed" || status === "error") && (
          <span className="agent-tool-group__row-failed">✗ {tool.error || "failed"}</span>
        )}
      </span>
    </>
  );

  if (!interactive) {
    return <div className={`agent-tool-group__row agent-tool-group__row--${status}`}>{inner}</div>;
  }

  return (
    <button
      type="button"
      className={`agent-tool-group__row agent-tool-group__row--${status} agent-tool-group__row--clickable`}
      onClick={() => onClick?.(tool)}
      aria-label={`Open details for ${tool.label}`}
    >
      {inner}
    </button>
  );
}

/** Compact token formatter shared with the session row. */
function formatTokenCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M tok`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k tok`;
  return `${n} tok`;
}

// ----------------------------------------------------------------------------
// Visual tone helpers — pure functions, easy to unit-test
// ----------------------------------------------------------------------------

type StatusKind = "running" | "completed" | "queued" | "failed" | "error";

function mapStatus(status: TraceNode["status"]): StatusKind {
  switch (status) {
    case "running": return "running";
    case "completed": return "completed";
    case "error": return "error";
    case "crashed": return "failed";
    default: return "queued";
  }
}

/** Two-letter mono badge for the agent. */
export function avatarLabel(agentId: string): string {
  if (!agentId) return "··";
  const cleaned = agentId.replace(/^agent:/, "").replace(/^z-bot[\W_]*/i, "").toUpperCase();
  if (cleaned.length === 0) return "··";
  if (cleaned.length === 1) return `${cleaned}·`;
  // Pick first letter + first letter that appears AFTER a separator. If
  // the id is one solid token, fall back to the second letter.
  const first = cleaned[0];
  const afterSep = cleaned.match(/[-_/. ]([A-Z0-9])/);
  if (afterSep) return `${first}${afterSep[1]}`;
  return cleaned.slice(0, 2);
}

/** Choose a colored avatar tone — varies by depth + agent id so siblings differ. */
export function avatarTone(agentId: string, depth: number): "root" | "code" | "research" | "planner" {
  if (depth === 0) return "root";
  const id = agentId.toLowerCase();
  if (id.includes("research")) return "research";
  if (id.includes("plan") || id.includes("test") || id.includes("tutor")) return "planner";
  return "code";
}
