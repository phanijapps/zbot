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
  const [childrenCollapsed, setChildrenCollapsed] = useState(node.type === "delegation");

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
