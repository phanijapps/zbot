// ============================================================================
// USE SESSION TRACE HOOK
// Fetches root + child session details and builds a TraceNode tree.
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import { getTransport } from "@/services/transport";
import type { SessionDetail, ExecutionLog, LogSession } from "@/services/transport/types";
import type { TraceNode } from "./trace-types";
import { isInternalTool, extractToolSummary } from "./trace-types";

// ============================================================================
// Return type
// ============================================================================

interface UseSessionTraceResult {
  trace: TraceNode | null;
  loading: boolean;
  refetch: () => void;
}

// ============================================================================
// Hook
// ============================================================================

export function useSessionTrace(sessionId: string | null): UseSessionTraceResult {
  const [trace, setTrace] = useState<TraceNode | null>(null);
  const [loading, setLoading] = useState(false);
  const [refetchKey, setRefetchKey] = useState(0);

  const refetch = useCallback(() => setRefetchKey((k) => k + 1), []);

  useEffect(() => {
    if (!sessionId) {
      setTrace(null);
      setLoading(false);
      return;
    }

    let cancelled = false;

    const load = async () => {
      setLoading(true);
      try {
        const transport = await getTransport();

        // 1. Fetch root session detail
        const rootResult = await transport.getLogSession(sessionId);
        if (cancelled) return;
        if (!rootResult.success || !rootResult.data) {
          console.error("Failed to load root session:", rootResult.error);
          setLoading(false);
          return;
        }

        const rootDetail = rootResult.data;

        // 2. Fetch all child session details in parallel
        const childIds = rootDetail.session.child_session_ids ?? [];
        const childResults = await Promise.all(
          childIds.map((id) => transport.getLogSession(id)),
        );
        if (cancelled) return;

        const childDetails: SessionDetail[] = [];
        for (const cr of childResults) {
          if (cr.success && cr.data) {
            childDetails.push(cr.data);
          }
        }

        // 3. Build trace tree
        const tree = buildTraceTree(rootDetail, childDetails);
        setTrace(tree);
      } catch (err) {
        if (!cancelled) {
          console.error("Failed to build session trace:", err);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => {
      cancelled = true;
    };
  }, [sessionId, refetchKey]);

  return { trace, loading, refetch };
}

// ============================================================================
// Helper: extract metadata field safely
// ============================================================================

/** Extract agent name from delegation message like "Delegating to code-agent" */
function extractAgentFromMessage(message: string): string | undefined {
  const match = message.match(/(?:Delegating to(?: agent:?)?\s+)(.+)/i);
  return match ? match[1].trim() : undefined;
}

function extractMetaField(log: ExecutionLog, field: string): string | undefined {
  if (!log.metadata) return undefined;
  const val = log.metadata[field];
  if (val === undefined || val === null) return undefined;
  return String(val);
}

// ============================================================================
// Helper: find matching tool_result for a tool_call
// ============================================================================

function findMatchingResult(
  logs: ExecutionLog[],
  toolCallLog: ExecutionLog,
): ExecutionLog | undefined {
  const toolId = extractMetaField(toolCallLog, "tool_id");
  if (!toolId) return undefined;
  return logs.find(
    (l) => l.category === "tool_result" && extractMetaField(l, "tool_id") === toolId,
  );
}

// ============================================================================
// Helper: map session status to TraceNode status
// ============================================================================

function mapStatus(status: string): TraceNode["status"] {
  switch (status) {
    case "running":
      return "running";
    case "completed":
      return "completed";
    case "error":
    case "stopped":
    case "crashed":
      return "error";
    default:
      return "completed";
  }
}

// ============================================================================
// Build trace tree from root + child details
// ============================================================================

function buildTraceTree(
  rootDetail: SessionDetail,
  childDetails: SessionDetail[],
): TraceNode {
  const rootSession = rootDetail.session;

  // Index child details by session_id for quick lookup
  const childMap = new Map<string, SessionDetail>();
  for (const cd of childDetails) {
    childMap.set(cd.session.session_id, cd);
  }

  // Build root node
  const rootNode: TraceNode = {
    id: rootSession.session_id,
    type: "root",
    agentId: rootSession.agent_id,
    label: rootSession.title || rootSession.agent_name || rootSession.agent_id,
    summary: rootSession.title,
    durationMs: rootSession.duration_ms,
    tokenCount: rootSession.token_count,
    status: mapStatus(rootSession.status),
    timestamp: rootSession.started_at,
    children: [],
  };

  // Process root session logs into children
  rootNode.children = buildChildNodes(rootDetail.logs, rootSession, childMap);

  return rootNode;
}

// ============================================================================
// Per-category log processors
// ============================================================================

function processToolCallLog(
  log: ExecutionLog,
  logs: ExecutionLog[],
): TraceNode | null {
  const toolName = extractMetaField(log, "tool_name") || log.message;

  // Skip internal tools
  if (isInternalTool(toolName)) return null;

  const resultLog = findMatchingResult(logs, log);
  const args = extractMetaField(log, "args");
  const result = resultLog ? extractMetaField(resultLog, "result") : undefined;
  const durationMs = resultLog?.duration_ms ?? log.duration_ms;
  const hasError = resultLog ? resultLog.level === "error" : false;

  return {
    id: log.id,
    type: "tool_call",
    agentId: log.agent_id,
    label: toolName,
    summary: extractToolSummary(toolName, args),
    durationMs,
    status: hasError ? "error" : resultLog ? "completed" : "running",
    timestamp: log.timestamp,
    args,
    result,
    children: [],
  };
}

function processDelegationLog(
  log: ExecutionLog,
  session: LogSession,
  childSessionsByAgent: Map<string, SessionDetail[]>,
): TraceNode {
  // Metadata key is "child_agent" (from stream.rs), not "child_agent_id" (from service.rs)
  const childAgentId =
    extractMetaField(log, "child_agent_id") ||
    extractMetaField(log, "child_agent") ||
    extractAgentFromMessage(log.message);
  const task = extractMetaField(log, "task") || log.message;

  // Find matching child session — consume in order for repeated agent delegations
  let childSessionDetail: SessionDetail | undefined;
  if (childAgentId) {
    const queue = childSessionsByAgent.get(childAgentId);
    if (queue && queue.length > 0) {
      childSessionDetail = queue.shift();
    }
  }

  const delegationNode: TraceNode = {
    id: log.id,
    type: "delegation",
    agentId: childAgentId || session.agent_id,
    label: childAgentId || "subagent",
    summary: task,
    durationMs: childSessionDetail?.session.duration_ms ?? log.duration_ms,
    tokenCount: childSessionDetail?.session.token_count,
    status: childSessionDetail ? mapStatus(childSessionDetail.session.status) : "running",
    timestamp: log.timestamp,
    children: [],
  };

  // Nest child session tool_call logs under delegation
  if (childSessionDetail) {
    delegationNode.children = buildChildNodes(
      childSessionDetail.logs,
      childSessionDetail.session,
      // Pass empty map: we don't recurse further for grandchildren in this version
      new Map(),
    );
  }

  return delegationNode;
}

function processErrorLog(log: ExecutionLog): TraceNode {
  return {
    id: log.id,
    type: "error",
    agentId: log.agent_id,
    label: "Error",
    summary: log.message,
    error: log.message,
    status: "error",
    timestamp: log.timestamp,
    children: [],
  };
}

// ============================================================================
// Build child nodes from a session's logs
// ============================================================================

function buildChildNodes(
  logs: ExecutionLog[],
  session: LogSession,
  childMap: Map<string, SessionDetail>,
): TraceNode[] {
  const children: TraceNode[] = [];

  // Separate logs by category
  const toolCalls = logs.filter((l) => l.category === "tool_call");
  const delegations = logs.filter((l) => l.category === "delegation");
  const errors = logs.filter((l) => l.category === "error");

  // Build ordered list of child sessions matching each delegation by agent_id + order
  // If the same agent is delegated multiple times, consume child sessions in order
  const childSessionsByAgent = new Map<string, SessionDetail[]>();
  for (const [, cd] of childMap) {
    const agentId = cd.session.agent_id;
    if (!childSessionsByAgent.has(agentId)) {
      childSessionsByAgent.set(agentId, []);
    }
    childSessionsByAgent.get(agentId)!.push(cd);
  }

  // Process all logs in timestamp order to maintain execution sequence
  const orderedLogs = [...toolCalls, ...delegations, ...errors].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
  );

  for (const log of orderedLogs) {
    if (log.category === "tool_call") {
      const node = processToolCallLog(log, logs);
      if (node) children.push(node);
    } else if (log.category === "delegation") {
      const node = processDelegationLog(log, session, childSessionsByAgent);
      children.push(node);
    } else if (log.category === "error") {
      children.push(processErrorLog(log));
    }
  }

  return children;
}
