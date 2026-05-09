// ============================================================================
// MISSION CONTROL — ToolsPane
// Right half of the detail pane. Shows the agent → tools tree for the
// selected session. Updates live via the existing useTraceSubscription hook
// (now WS-based instead of polling). Clicking a tool row opens a detail
// popover with the tool's input args + output result.
// ============================================================================

import { useMemo, useState } from "react";
import type { LogSession } from "@/services/transport/types";
import type { TraceNode } from "../logs/trace-types";
import { useSessionTrace } from "../logs/useSessionTrace";
import { useTraceSubscription } from "../logs/useTraceSubscription";
import { AgentToolGroup } from "./AgentToolGroup";
import { ToolDetailPopover } from "./ToolDetailPopover";
import { sumExecutionTokensByAgent, executionTokensById, type SessionTokenIndex } from "./useSessionTokens";

interface ToolsPaneProps {
  session: LogSession | null;
  tokenIndex?: SessionTokenIndex;
}

export function ToolsPane({ session, tokenIndex }: ToolsPaneProps) {
  const sessionId = session?.session_id ?? null;
  const { trace, loading, refetch } = useSessionTrace(sessionId);
  const [openTool, setOpenTool] = useState<TraceNode | null>(null);

  // Per-agent token totals (agent_id → combined tokens across all runs of that agent).
  // Used as fallback for the root node which has no executionId.
  const tokensByAgent = useMemo(() => {
    if (!sessionId || !tokenIndex) return new Map<string, { in: number; out: number }>();
    const entries = tokenIndex.executionsByRootExecId.get(sessionId);
    return sumExecutionTokensByAgent(entries);
  }, [sessionId, tokenIndex]);

  // Per-execution token lookup (executionId → individual run tokens).
  // This is the primary lookup for delegation nodes: when the same agent is
  // delegated multiple times, each card shows its own run's counts, not the sum.
  const tokensByExecution = useMemo(() => {
    if (!sessionId || !tokenIndex) return new Map<string, { in: number; out: number }>();
    const entries = tokenIndex.executionsByRootExecId.get(sessionId);
    return executionTokensById(entries);
  }, [sessionId, tokenIndex]);

  // Live: refetch trace whenever a tool_call/tool_result event arrives on
  // the conversation WebSocket (no polling). When the session is finished,
  // useTraceSubscription is a no-op.
  useTraceSubscription({ session, onEvent: refetch });

  return (
    <div className="mc-pane">
      <header className="mc-pane__head">
        <span className="mc-pane__title">Tools</span>
        <LiveBadge active={session?.status === "running"} />
      </header>
      <div className="mc-pane__body">
        {!session && <Empty message="Select a session to see its tool calls." />}
        {session && loading && !trace && <Empty message="Loading trace…" />}
        {session && !loading && !trace && <Empty message="No trace data for this session yet." />}
        {trace && (
          <div className="agent-tool-tree">
            <AgentToolGroup
              node={trace}
              depth={0}
              onToolClick={setOpenTool}
              tokensByAgent={tokensByAgent}
              tokensByExecution={tokensByExecution}
            />
          </div>
        )}
      </div>
      <ToolDetailPopover tool={openTool} onClose={() => setOpenTool(null)} />
    </div>
  );
}

function LiveBadge({ active }: { active: boolean }) {
  if (!active) return <span className="mc-pane__live mc-pane__live--off">Cached</span>;
  return <span className="mc-pane__live">Live</span>;
}

function Empty({ message }: { message: string }) {
  return <div className="mc-pane__empty">{message}</div>;
}
