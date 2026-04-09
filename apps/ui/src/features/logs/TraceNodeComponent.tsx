import { useState } from "react";
import {
  Wrench,
  Brain,
  Users,
  MessageSquare,
  AlertCircle,
  Bot,
  ChevronRight,
  ChevronDown,
  Terminal,
  FileEdit,
  Eye,
  Search,
  Globe,
  FolderSearch,
} from "lucide-react";
import type { TraceNode } from "./trace-types";
import { formatDuration, formatTokens } from "./trace-types";
import { TraceNodeDetail } from "./TraceNodeDetail";

/** Map tool names to specific icons */
function getToolIcon(toolName: string, size: number) {
  switch (toolName) {
    case "shell":
      return <Terminal size={size} />;
    case "edit":
    case "write":
      return <FileEdit size={size} />;
    case "read":
      return <Eye size={size} />;
    case "grep":
      return <Search size={size} />;
    case "glob":
      return <FolderSearch size={size} />;
    case "web_fetch":
      return <Globe size={size} />;
    case "recall":
    case "save_fact":
    case "memory":
      return <Brain size={size} />;
    case "delegate_to_agent":
    case "list_agents":
      return <Users size={size} />;
    case "respond":
      return <MessageSquare size={size} />;
    default:
      return <Wrench size={size} />;
  }
}

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

  const iconSize = 14;
  const iconColor = isError ? "var(--destructive)" : "var(--muted-foreground)";

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
        <span className="trace-node__icon" style={{ color: iconColor }}>
          {node.type === "root" ? (
            <Bot size={iconSize} />
          ) : isDelegation ? (
            hasChildren && !childrenCollapsed ? <ChevronDown size={iconSize} /> : <ChevronRight size={iconSize} />
          ) : node.type === "error" ? (
            <AlertCircle size={iconSize} />
          ) : (
            getToolIcon(node.label, iconSize)
          )}
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
