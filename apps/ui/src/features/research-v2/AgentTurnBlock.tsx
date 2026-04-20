import type React from "react";
import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Square,
  AlertCircle,
  Loader2,
} from "lucide-react";
import type { AgentTurn, AgentTurnStatus } from "./types";
import { childrenOf } from "./turn-tree";
import { AgentAvatar, CopyButton } from "./ResearchMessages";

export interface AgentTurnBlockProps {
  turn: AgentTurn;
  /** Kept for API stability; the redesigned layout no longer exposes a
   *  toggle surface — all tool/thinking events land in the top pill ticker. */
  onToggleThinking?(turnId: string): void;
  /** Direct children of `turn`. */
  childTurns?: AgentTurn[];
  /** Full flat turn list so subagent cards can recurse into grand-children. */
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
  // Parity with chat-v2: stream tokens render as markdown too (same component
  // as the final respond, just styled with a "streaming" class for cursor/
  // opacity). Without this, code fences and lists flash as raw text until the
  // turn completes.
  return (
    <div className="agent-turn-block__streaming">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
    </div>
  );
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

function respondIsStreaming(turn: AgentTurn): boolean {
  return turn.respond === null && turn.respondStreaming.length > 0;
}

/** Collapse a subagent card when it transitions out of the "running" state.
 *  Keeps the effect local to the card so there's no reducer bookkeeping.
 *  User-toggled expand wins until the status changes again. */
function useSubagentStatusTransition(
  status: AgentTurnStatus,
  setExpanded: (v: boolean) => void,
): AgentTurnStatus {
  const [prev, setPrev] = useState<AgentTurnStatus>(status);
  useEffect(() => {
    if (prev !== status) {
      setExpanded(status === "running");
      setPrev(status);
    }
  }, [status, prev, setExpanded]);
  return prev;
}

/** What the copy button should copy. Prefer finalized respond, then the
 *  streaming buffer; return null when there's nothing useful. */
function copyableRespondText(turn: AgentTurn): string | null {
  if (turn.respond && turn.respond.length > 0) return turn.respond;
  if (turn.respondStreaming && turn.respondStreaming.length > 0) return turn.respondStreaming;
  return null;
}

/**
 * Subagent card: Request + Response only, no thinking/tool timeline. All
 * subagent tool/thinking events surface in the top pill (news ticker). The
 * card's job is to show "what we asked of this delegate" and "what it came
 * back with". Running → Request + "waiting…". Completed → Request + Response.
 */
interface SubagentCardProps {
  turn: AgentTurn;
}

function SubagentResponseBody({ turn }: SubagentCardProps): React.ReactElement {
  if (turn.status === "error" && turn.errorMessage) {
    return <ErrorBanner message={turn.errorMessage} />;
  }
  if (turn.respond) return <RespondMarkdown content={turn.respond} />;
  if (turn.respondStreaming) return <StreamingBuffer text={turn.respondStreaming} />;
  if (turn.status === "running") return <WaitingPlaceholder />;
  return <span className="agent-turn-block__placeholder">(no response)</span>;
}

function SubagentCard({ turn }: SubagentCardProps) {
  const color = agentColour(turn.agentId);
  const respondText = copyableRespondText(turn);
  // Default: expanded while running, collapsed once done. User can override
  // either way by clicking the header. Reset on status transition.
  const [expanded, setExpanded] = useState(turn.status === "running");
  const prevStatus = useSubagentStatusTransition(turn.status, setExpanded);
  void prevStatus;
  return (
    <div
      className="subagent-card"
      style={{ borderLeft: `3px solid ${color}` }}
      data-parent={turn.parentExecutionId ?? ""}
      data-expanded={expanded}
    >
      <button
        type="button"
        className="subagent-card__header subagent-card__toggle"
        onClick={() => setExpanded((e) => !e)}
        aria-expanded={expanded}
        aria-label={expanded ? "Collapse subagent" : "Expand subagent"}
      >
        <span className="subagent-card__chevron" aria-hidden="true">
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </span>
        <span className="subagent-card__agent" style={{ color }}>
          {turn.agentId}
        </span>
        <span className="subagent-card__meta">
          <StatusIcon status={turn.status} />
          <span>{formatDuration(turn.startedAt, turn.completedAt)}</span>
        </span>
      </button>
      {expanded && turn.request && (
        <div className="subagent-card__section">
          <div className="subagent-card__label">Request</div>
          <div className="subagent-card__text">{turn.request}</div>
        </div>
      )}
      {expanded && (
        <div className="subagent-card__section">
          <div className="subagent-card__label">Response</div>
          <div className="subagent-card__text">
            <SubagentResponseBody turn={turn} />
          </div>
        </div>
      )}
      {expanded && respondText !== null && (
        <CopyButton text={respondText} label="Copy response" />
      )}
    </div>
  );
}

/**
 * Root block: avatar + nested subagent cards + final respond + copy.
 * No thinking chevron, no tool timeline — root's thinking/tool_calls surface
 * only in the top pill ticker. All subagent cards appear here; whether they're
 * running or complete they render as minimal Request/Response cards.
 */
interface RootTurnProps {
  turn: AgentTurn;
  childTurns: AgentTurn[];
  allTurns: AgentTurn[];
}

function RootTurn({ turn, childTurns, allTurns }: RootTurnProps) {
  const respondText = copyableRespondText(turn);
  return (
    <div
      className={`research-msg research-msg--assistant${respondIsStreaming(turn) ? " research-msg--streaming" : ""}`}
      data-parent=""
    >
      <AgentAvatar />
      <div className="research-msg__body">
        {childTurns.length > 0 && (
          <div className="root-turn__subagents">
            {childTurns.map((child) => (
              <SubagentCardTree key={child.id} turn={child} allTurns={allTurns} />
            ))}
          </div>
        )}
        <RespondBody turn={turn} />
      </div>
      {respondText !== null && (
        <CopyButton text={respondText} label="Copy response" />
      )}
    </div>
  );
}

/**
 * Subagent cards can themselves delegate — render grand-children inside the
 * card recursively. Kept as a separate component so the root block stays
 * readable and the recursion is isolated.
 */
interface SubagentCardTreeProps {
  turn: AgentTurn;
  allTurns: AgentTurn[];
}

function SubagentCardTree({ turn, allTurns }: SubagentCardTreeProps) {
  const grandChildren = childrenOf(turn, allTurns);
  return (
    <div className="subagent-card-tree">
      <SubagentCard turn={turn} />
      {grandChildren.length > 0 && (
        <div className="subagent-card-tree__nested">
          {grandChildren.map((gc) => (
            <SubagentCardTree key={gc.id} turn={gc} allTurns={allTurns} />
          ))}
        </div>
      )}
    </div>
  );
}

export function AgentTurnBlock({
  turn,
  childTurns,
  allTurns,
}: AgentTurnBlockProps) {
  const childList = childTurns ?? [];
  const fullList = allTurns ?? childList;

  // Root turn → clean assistant layout with nested subagent cards.
  if (turn.parentExecutionId === null) {
    return <RootTurn turn={turn} childTurns={childList} allTurns={fullList} />;
  }

  // Subagent turn rendered standalone (shouldn't normally happen — the root
  // wraps them — but covers orphan-subagent edge cases).
  return <SubagentCardTree turn={turn} allTurns={fullList} />;
}
