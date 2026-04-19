import type React from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  ChevronRight,
  CheckCircle2,
  Square,
  AlertCircle,
  Loader2,
} from "lucide-react";
import type { AgentTurn, AgentTurnStatus } from "./types";
import { ThinkingTimeline } from "./ThinkingTimeline";
import { childrenOf } from "./turn-tree";

export interface AgentTurnBlockProps {
  turn: AgentTurn;
  onToggleThinking(turnId: string): void;
  /**
   * Direct children of `turn`. Optional — omit for leaf-only rendering.
   * Callers derive this with `childrenOf(turn, allTurns)`.
   */
  children?: AgentTurn[];
  /**
   * Full flat turn list used to recurse past the first child level.
   * Choice A (see R14b spec): passing allTurns down keeps the component pure
   * and the tree shape derived at render. Alternative B (pre-computed nested
   * children) would push recursion into the parent and make the block
   * artificially dumb, but would couple parents to grand-children and make
   * tests brittle. A wins on testability and separation of concerns.
   */
  allTurns?: AgentTurn[];
}

// Agent identity → accent colour. Theme tokens where possible.
const AGENT_COLOR: Record<string, string> = {
  planner: "var(--success)",
  "planner-agent": "var(--success)",
  solution: "var(--purple)",
  "solution-agent": "var(--purple)",
  builder: "var(--warning)",
  "builder-agent": "var(--warning)",
  writer: "var(--blue)",
  "writer-agent": "var(--blue)",
  root: "var(--foreground)",
  "quick-chat": "var(--teal)",
};

function agentColour(agentId: string): string {
  return AGENT_COLOR[agentId] ?? "var(--muted-foreground)";
}

function formatDuration(startedAt: number, completedAt: number | null): string {
  const end = completedAt ?? Date.now();
  const ms = end - startedAt;
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms / 1000)}s`;
}

function StatusIcon({ status }: { status: AgentTurnStatus }) {
  switch (status) {
    case "running":   return <Loader2 size={14} className="spin" />;
    case "completed": return <CheckCircle2 size={14} />;
    case "stopped":   return <Square size={14} />;
    case "error":     return <AlertCircle size={14} />;
  }
}

function ErrorBanner({ message }: { message: string }) {
  return (
    <div
      className="agent-turn-block__error"
      data-testid="turn-error-banner"
      role="alert"
    >
      <AlertCircle size={14} aria-hidden="true" />
      <span>{message}</span>
    </div>
  );
}

function RespondMarkdown({ content }: { content: string }) {
  return (
    <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
  );
}

function StreamingBuffer({ text }: { text: string }) {
  return <span className="agent-turn-block__streaming">{text}</span>;
}

function WaitingPlaceholder() {
  return <span className="agent-turn-block__placeholder">waiting…</span>;
}

/** Decides what to render in the Respond body slot. Each branch is a named helper. */
function RespondBody({ turn }: { turn: AgentTurn }): React.ReactElement {
  if (turn.status === "error" && turn.errorMessage) {
    return <ErrorBanner message={turn.errorMessage} />;
  }
  if (turn.respond) {
    return <RespondMarkdown content={turn.respond} />;
  }
  if (turn.respondStreaming) {
    return <StreamingBuffer text={turn.respondStreaming} />;
  }
  return <WaitingPlaceholder />;
}

interface ThinkingChevronProps {
  turnId: string;
  count: number;
  expanded: boolean;
  onToggle(turnId: string): void;
}

function ThinkingChevron({ turnId, count, expanded, onToggle }: ThinkingChevronProps) {
  const label = `${count} ${count === 1 ? "action" : "actions"}`;
  return (
    <button
      type="button"
      data-testid={`thinking-chevron-${turnId}`}
      className="agent-turn-block__chevron"
      onClick={() => onToggle(turnId)}
      aria-expanded={expanded}
    >
      <ChevronRight
        size={14}
        style={{ transform: expanded ? "rotate(90deg)" : "rotate(0deg)" }}
      />
      <span>Thinking ({label})</span>
    </button>
  );
}

interface TurnMetaProps {
  turn: AgentTurn;
  color: string;
}

function TurnHeader({ turn, color }: TurnMetaProps) {
  return (
    <div className="agent-turn-block__header">
      <span className="agent-turn-block__agent" style={{ color }}>
        {turn.agentId}
      </span>
      <span className="agent-turn-block__meta">
        <StatusIcon status={turn.status} />
        <span>{formatDuration(turn.startedAt, turn.completedAt)}</span>
        {turn.tokenCount > 0 && <span>{turn.tokenCount}tok</span>}
        {turn.status === "running" && (
          <span
            data-testid="turn-running-badge"
            className="agent-turn-block__running"
          >
            · running
          </span>
        )}
      </span>
    </div>
  );
}

function respondIsStreaming(turn: AgentTurn): boolean {
  return turn.respond === null && turn.respondStreaming.length > 0;
}

interface NestedChildrenProps {
  children: AgentTurn[];
  allTurns: AgentTurn[];
  onToggleThinking(turnId: string): void;
}

/** Recursively renders child turns indented under their parent. */
function NestedChildren({ children, allTurns, onToggleThinking }: NestedChildrenProps) {
  if (children.length === 0) return null;
  return (
    <div
      className="agent-turn-block__children"
      data-testid="nested-children"
    >
      {children.map((child) => (
        <AgentTurnBlock
          key={child.id}
          turn={child}
          onToggleThinking={onToggleThinking}
          children={childrenOf(child, allTurns)}
          allTurns={allTurns}
        />
      ))}
    </div>
  );
}

export function AgentTurnBlock({
  turn,
  onToggleThinking,
  children,
  allTurns,
}: AgentTurnBlockProps) {
  const color = agentColour(turn.agentId);
  const childList = children ?? [];
  const fullList = allTurns ?? childList;

  return (
    <div
      className="agent-turn-block"
      style={{ borderLeft: `3px solid ${color}` }}
      data-parent={turn.parentExecutionId ?? ""}
    >
      <TurnHeader turn={turn} color={color} />

      <ThinkingChevron
        turnId={turn.id}
        count={turn.timeline.length}
        expanded={turn.thinkingExpanded}
        onToggle={onToggleThinking}
      />

      {turn.thinkingExpanded && (
        <div className="agent-turn-block__timeline">
          <ThinkingTimeline entries={turn.timeline} />
        </div>
      )}

      <div
        className={`agent-turn-block__respond${respondIsStreaming(turn) ? " agent-turn-block__respond--streaming" : ""}`}
      >
        <RespondBody turn={turn} />
      </div>

      <NestedChildren
        children={childList}
        allTurns={fullList}
        onToggleThinking={onToggleThinking}
      />
    </div>
  );
}
