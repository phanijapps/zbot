// =============================================================================
// session-snapshot — REST fan-out that rebuilds the full research session
// state from three endpoints (logs, messages, artifacts).
//
// Why this file exists (R14f): the previous hydrate path only fetched the
// root-scoped message list and relied on the WS stream for everything else
// (subagent cards, titles, artifacts, per-turn respond). WS reconnects drop
// events silently — so opening an already-running / already-completed session
// left the UI stuck on the user prompt. Snapshot-on-open is truth; WS is
// delta-only while state.status === "running".
//
// Wire quirks this module hides from the rest of the hook:
// - `LogSession.session_id` is an execution id; the real session id lives in
//   `LogSession.conversation_id`.
// - `parent_session_id` is empty/absent on the root row; non-empty on children.
// - `[tool calls]` assistant content carries the real final answer in a
//   parallel `toolCalls` (camel) or `tool_calls` (snake) column whose JSON
//   entries use `tool_name: "respond"`.
//
// Subagents do NOT spawn subagents (hard 2-level tree), so the child list can
// be built flat from the log rows + sorted by `started_at`.
// =============================================================================

import type { Transport } from "@/services/transport";
import type {
  Artifact,
  LogSession,
  SessionMessage,
} from "@/services/transport/types";
import type {
  AgentTurn,
  AgentTurnStatus,
  ResearchArtifactRef,
  ResearchMessage,
  ResearchStatus,
} from "./types";
import { toArtifactRef } from "./artifact-poll";

// -----------------------------------------------------------------------------
// Constants
// -----------------------------------------------------------------------------

const DEFAULT_AGENT_ID = "root";
const RESPOND_TOOL_NAME = "respond";
const DELEGATE_TOOL_NAME = "delegate_to_agent";
const TOOL_CALLS_PLACEHOLDER = "[tool calls]";
const ASSISTANT_ROLE = "assistant";
const USER_ROLE = "user";

// -----------------------------------------------------------------------------
// Public surface
// -----------------------------------------------------------------------------

export interface ResearchSnapshot {
  title: string;
  status: ResearchStatus;
  /** Root + children, flat. Children sorted by startedAt ascending. */
  turns: AgentTurn[];
  artifacts: ResearchArtifactRef[];
  /** User bubbles only — assistant content is rendered via turns[].respond. */
  messages: ResearchMessage[];
  /** Reserved for future log-row field; null today. */
  wardId: string | null;
  wardName: string | null;
  /**
   * Only non-null when the hook can resubscribe with it. For snapshots of
   * pre-existing sessions we don't know the original conv_id — left null so
   * the WS subscription stays idle until the user sends a new message.
   * The live sendMessage path mints its own conv_id, so this is never the
   * blocker for live sessions.
   */
  conversationId: string | null;
}

/**
 * Build a snapshot for `sessionId` by fanning out to
 * `/api/logs/sessions`, `/api/sessions/:id/messages?scope=all`, and
 * `/api/sessions/:id/artifacts` in parallel. Returns null if any required
 * call fails or the root row can't be located — caller typically dispatches
 * ERROR on null.
 */
export async function snapshotSession(
  transport: Transport,
  sessionId: string,
): Promise<ResearchSnapshot | null> {
  const [logsRes, msgsRes, artifactsRes, stateRes] = await Promise.all([
    transport.listLogSessions(),
    transport.getSessionMessages(sessionId, { scope: "all" }),
    transport.listSessionArtifacts(sessionId).catch(() => ({ success: false } as const)),
    // /api/sessions/:id/state carries ward info so a reopened session
    // re-populates the header ward chip + clickable folder link. Soft
    // fail: older backends without the endpoint just leave ward null.
    transport.getSessionState(sessionId).catch(() => ({ success: false } as const)),
  ]);

  if (!logsRes.success || !logsRes.data) return null;
  if (!msgsRes.success || !msgsRes.data) return null;

  const sessionRows = logsRes.data.filter((r) => r.conversation_id === sessionId);
  const rootRow = sessionRows.find(isRootRow);
  if (!rootRow) return null;

  const messages = msgsRes.data;
  const turns = buildTurns(rootRow, sessionRows, messages);
  const artifacts = buildArtifacts(artifactsRes, messages);
  // Only the root execution's user rows are real prompts; subagents carry
  // system-injected "user" rows (ward_snapshot context, delegation preambles)
  // that look like prompts but shouldn't render.
  const userMessages = buildUserMessages(messages, rootRow.session_id);
  const title = pickTitle(sessionRows);
  // Session-level truth wins over per-execution status on reopen.
  //
  // `/api/logs/sessions` reports the *root execution's* status. That row
  // flips to "completed" as soon as the root's first pass ends — even
  // while subagents are still running and a continuation turn is pending.
  // Using it as the hydrate status was silencing the WS subscribe guard
  // (`state.status === "running"`) on reopened live sessions: we'd load
  // the history, then sit polling HTTP forever because the subscription
  // never fired.
  //
  // `/api/sessions/:id/state.isLive` is computed server-side each request
  // by checking for any running executions attached to the session, so
  // it's the freshest signal we have. Prefer it; fall back to the
  // session-level status on the same endpoint; fall back to the
  // per-execution row only when `/state` is unavailable (older backends).
  const status: ResearchStatus =
    stateRes.success && stateRes.data
      ? stateRes.data.isLive
        ? "running"
        : mapRootStatus(stateRes.data.session.status)
      : mapRootStatus(rootRow.status);
  const wardName = stateRes.success && stateRes.data?.ward?.name
    ? stateRes.data.ward.name
    : null;

  return {
    title,
    status,
    turns,
    artifacts,
    messages: userMessages,
    // The ward tool identifies a ward by its name; the gateway's
    // /api/wards/:id/open accepts that same name as the :id param.
    // There's no separate numeric ward id surfaced to the UI today.
    wardId: wardName,
    wardName,
    conversationId: null,
  };
}

// -----------------------------------------------------------------------------
// Log-row helpers
// -----------------------------------------------------------------------------

export function isRootRow(row: LogSession): boolean {
  const parent = row.parent_session_id;
  return parent == null || parent.length === 0;
}

export function turnFromLogRow(
  row: LogSession,
  parentId: string | null,
): AgentTurn {
  return {
    id: row.session_id,
    agentId: row.agent_id || DEFAULT_AGENT_ID,
    parentExecutionId: parentId,
    startedAt: parseTimestamp(row.started_at),
    completedAt: row.ended_at ? parseTimestamp(row.ended_at) : null,
    status: mapTurnStatus(row.status),
    wardId: null,
    request: null,
    timeline: [],
    tokenCount: row.token_count ?? 0,
    respond: null,
    respondStreaming: "",
    thinkingExpanded: false,
    errorMessage: null,
  };
}

function mapTurnStatus(raw: LogSession["status"] | string): AgentTurnStatus {
  switch (raw) {
    case "completed":
      return "completed";
    case "running":
      return "running";
    case "stopped":
    case "cancelled":
      return "stopped";
    case "error":
    case "crashed":
      return "error";
    default:
      return "error";
  }
}

function mapRootStatus(raw: LogSession["status"] | string): ResearchStatus {
  switch (raw) {
    case "completed":
      return "complete";
    case "running":
      return "running";
    case "stopped":
    case "cancelled":
      return "stopped";
    case "error":
    case "crashed":
      return "error";
    default:
      return "idle";
  }
}

function parseTimestamp(iso: string | undefined): number {
  if (!iso) return 0;
  const t = Date.parse(iso);
  return Number.isFinite(t) ? t : 0;
}

function pickTitle(rows: LogSession[]): string {
  for (const row of rows) {
    if (typeof row.title === "string" && row.title.length > 0) return row.title;
  }
  return "";
}

// -----------------------------------------------------------------------------
// tool_calls parsing (shared between respond-extraction and delegation)
// -----------------------------------------------------------------------------

interface ToolCall {
  tool_name?: string;
  args?: Record<string, unknown>;
}

/**
 * Accepts both camelCase `toolCalls` (current wire) and snake_case `tool_calls`
 * (legacy). Backend emits the parallel column as either a JSON string or an
 * already-decoded array — handle both.
 */
function parseToolCalls(m: SessionMessage): ToolCall[] {
  const camel = (m as unknown as { toolCalls?: unknown }).toolCalls;
  const candidate = camel ?? m.tool_calls;
  if (candidate == null) return [];
  try {
    const raw = typeof candidate === "string" ? candidate : JSON.stringify(candidate);
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as ToolCall[]) : [];
  } catch {
    return [];
  }
}

// -----------------------------------------------------------------------------
// Respond extraction — last respond() per execution_id wins.
// -----------------------------------------------------------------------------

export function extractRespondByExecId(
  messages: SessionMessage[],
): Map<string, string> {
  const out = new Map<string, string>();
  for (const m of messages) {
    if (m.role !== ASSISTANT_ROLE) continue;
    const calls = parseToolCalls(m);
    for (const call of calls) {
      if (call?.tool_name !== RESPOND_TOOL_NAME) continue;
      const message = call.args?.["message"];
      if (typeof message !== "string" || message.length === 0) continue;
      out.set(m.execution_id, message);
    }
  }
  return out;
}

// -----------------------------------------------------------------------------
// Delegation tasks — walk ROOT's messages in timestamp order.
//
// Order-matching is the trick: we don't need the child's convid from the
// tool_result row, because subagents are a flat list under root and children
// are sorted by `startedAt` ascending, matching the order root delegated them.
// -----------------------------------------------------------------------------

export function extractDelegationTasks(
  messages: SessionMessage[],
  rootExecId: string,
): string[] {
  const rootMessages = messages
    .filter((m) => m.execution_id === rootExecId && m.role === ASSISTANT_ROLE)
    .sort((a, b) => a.created_at.localeCompare(b.created_at));

  const out: string[] = [];
  for (const m of rootMessages) {
    for (const call of parseToolCalls(m)) {
      if (call?.tool_name !== DELEGATE_TOOL_NAME) continue;
      const task = call.args?.["task"];
      if (typeof task === "string") out.push(task);
    }
  }
  return out;
}

// -----------------------------------------------------------------------------
// Artifact refs from respond tool calls (used as fallback when /artifacts
// endpoint returned nothing — the file was written but not yet indexed).
// -----------------------------------------------------------------------------

interface ArtifactHint {
  path: string;
  label?: string;
}

function extractArtifactHints(messages: SessionMessage[]): ArtifactHint[] {
  const out: ArtifactHint[] = [];
  for (const m of messages) {
    if (m.role !== ASSISTANT_ROLE) continue;
    for (const call of parseToolCalls(m)) {
      if (call?.tool_name !== RESPOND_TOOL_NAME) continue;
      const artifacts = call.args?.["artifacts"];
      if (!Array.isArray(artifacts)) continue;
      for (const art of artifacts) {
        if (art && typeof art === "object") {
          const path = (art as Record<string, unknown>)["path"];
          const label = (art as Record<string, unknown>)["label"];
          if (typeof path === "string" && path.length > 0) {
            out.push({ path, label: typeof label === "string" ? label : undefined });
          }
        }
      }
    }
  }
  return out;
}

// -----------------------------------------------------------------------------
// buildTurns — root + flat children with delegation-task zipping.
// -----------------------------------------------------------------------------

function buildTurns(
  rootRow: LogSession,
  sessionRows: LogSession[],
  messages: SessionMessage[],
): AgentTurn[] {
  const respondByExec = extractRespondByExecId(messages);
  const rootTurn = applyRespond(turnFromLogRow(rootRow, null), respondByExec);

  const childRows = sessionRows
    .filter((r) => !isRootRow(r) && r.session_id !== rootRow.session_id)
    .sort((a, b) => parseTimestamp(a.started_at) - parseTimestamp(b.started_at));

  const tasks = extractDelegationTasks(messages, rootRow.session_id);

  const childTurns = childRows.map((row, idx) => {
    const base = turnFromLogRow(row, rootRow.session_id);
    const withRespond = applyRespond(base, respondByExec);
    const request = idx < tasks.length ? tasks[idx] : null;
    return { ...withRespond, request };
  });

  return [rootTurn, ...childTurns];
}

function applyRespond(
  turn: AgentTurn,
  respondByExec: Map<string, string>,
): AgentTurn {
  const message = respondByExec.get(turn.id);
  if (!message) return turn;
  return { ...turn, respond: message, respondStreaming: "" };
}

// -----------------------------------------------------------------------------
// Artifacts — /artifacts endpoint wins; respond hints fill in on empty.
// -----------------------------------------------------------------------------

type ArtifactsResult = { success: boolean; data?: Artifact[] };

function buildArtifacts(
  res: ArtifactsResult,
  messages: SessionMessage[],
): ResearchArtifactRef[] {
  if (res.success && res.data && res.data.length > 0) {
    return dedupeRefs(res.data.map(toArtifactRef));
  }
  // Fallback: synthesize refs from respond.args.artifacts. Path acts as the
  // stable id since these records don't have a DB id yet.
  const hints = extractArtifactHints(messages);
  const refs: ResearchArtifactRef[] = hints.map((h) => ({
    id: h.path,
    fileName: fileNameFromPath(h.path),
    label: h.label,
  }));
  return dedupeRefs(refs);
}

function fileNameFromPath(path: string): string {
  const slash = path.lastIndexOf("/");
  return slash >= 0 ? path.slice(slash + 1) : path;
}

function dedupeRefs(refs: ResearchArtifactRef[]): ResearchArtifactRef[] {
  const seen = new Set<string>();
  const out: ResearchArtifactRef[] = [];
  for (const r of refs) {
    if (seen.has(r.id)) continue;
    seen.add(r.id);
    out.push(r);
  }
  return out;
}

// -----------------------------------------------------------------------------
// User messages — role === "user" only; assistants render via turns.respond.
// -----------------------------------------------------------------------------

const SYSTEM_INJECTED_MARKERS = ["<ward_snapshot", "[Delegation "];

function isRealUserPrompt(m: SessionMessage, rootExecutionId: string): boolean {
  if (m.role !== USER_ROLE) return false;
  if (m.content === TOOL_CALLS_PLACEHOLDER) return false;
  // Subagent executions carry system-injected user-role messages (ward
  // snapshots, delegation context). Keep only rows from the root execution.
  if (m.execution_id !== rootExecutionId) return false;
  const trimmed = m.content.trimStart();
  return !SYSTEM_INJECTED_MARKERS.some((prefix) => trimmed.startsWith(prefix));
}

function buildUserMessages(messages: SessionMessage[], rootExecutionId: string): ResearchMessage[] {
  return messages
    .filter((m) => isRealUserPrompt(m, rootExecutionId))
    .map((m) => ({
      id: m.id,
      role: "user" as const,
      content: m.content,
      timestamp: parseTimestamp(m.created_at),
    }));
}
