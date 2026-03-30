// ============================================================================
// MISSION HOOKS
// Maps WebSocket events to Mission Control state (NarrativeBlock[])
// Modeled after WebChatPanel's event handling but outputs richer block types.
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { getTransport, type StreamEvent } from "@/services/transport";
import type { PlanStep, StepStatus } from "./PlanBlock";
import type { RecalledFact, SubagentInfo } from "./IntelligenceFeed";
import type { UploadedFile } from "./ChatInput";

// ============================================================================
// Types
// ============================================================================

export interface NarrativeBlock {
  id: string;
  type: "user" | "recall" | "tool" | "delegation" | "plan" | "response";
  timestamp: string;
  /** Shape depends on `type` — see block components for expected props */
  data: Record<string, unknown>;
  isStreaming?: boolean;
  isExpanded?: boolean;
}

export interface MissionState {
  blocks: NarrativeBlock[];
  sessionTitle: string;
  status: "idle" | "running" | "completed" | "error";
  tokenCount: number;
  durationMs: number;
  modelName: string;
  subagents: SubagentInfo[];
  plan: PlanStep[];
  recalledFacts: RecalledFact[];
  activeWard: { name: string; content: string } | null;
}

// ============================================================================
// Helpers
// ============================================================================

const ROOT_AGENT_ID = "root";
const WEB_CONV_ID_KEY = "agentzero_web_conv_id";
const WEB_SESSION_ID_KEY = "agentzero_web_session_id";

function getOrCreateConversationId(): string {
  // Check for ?new=1 param — start fresh session
  const params = new URLSearchParams(window.location.search);
  if (params.get("new") === "1") {
    // Clear the param from URL without reload
    params.delete("new");
    const newUrl = window.location.pathname + (params.toString() ? `?${params}` : "");
    window.history.replaceState({}, "", newUrl);
    return createNewConversationId();
  }

  let convId = localStorage.getItem(WEB_CONV_ID_KEY);
  if (!convId) {
    convId = `web-${crypto.randomUUID()}`;
    localStorage.setItem(WEB_CONV_ID_KEY, convId);
  }
  return convId;
}

/** Create a fresh conversation — clears session state so the next invoke creates a new session */
function createNewConversationId(): string {
  localStorage.removeItem(WEB_SESSION_ID_KEY);
  const convId = `web-${crypto.randomUUID()}`;
  localStorage.setItem(WEB_CONV_ID_KEY, convId);
  return convId;
}

function getSessionId(): string | null {
  return localStorage.getItem(WEB_SESSION_ID_KEY);
}

function setSessionId(sessionId: string): void {
  localStorage.setItem(WEB_SESSION_ID_KEY, sessionId);
}

function now(): string {
  return new Date().toISOString();
}

/** Try to parse JSON arguments from a tool call */
function parseArgs(args: unknown): Record<string, unknown> {
  if (!args) return {};
  if (typeof args === "object") return args as Record<string, unknown>;
  if (typeof args === "string") {
    try { return JSON.parse(args); } catch { return {}; }
  }
  return {};
}

// ============================================================================
// Hook: useMissionControl
// ============================================================================

export function useMissionControl() {
  // -- Core state --
  const [blocks, setBlocks] = useState<NarrativeBlock[]>([]);
  const [sessionTitle, setSessionTitle] = useState("");
  const [status, setStatus] = useState<MissionState["status"]>("idle");
  const [tokenCount, setTokenCount] = useState(0);
  const [modelName, setModelName] = useState("");

  // -- Sidebar data (extracted from blocks) --
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [plan, setPlan] = useState<PlanStep[]>([]);
  const [recalledFacts, setRecalledFacts] = useState<RecalledFact[]>([]);
  const [activeWard, setActiveWard] = useState<{ name: string; content: string } | null>(null);

  // -- Session/conversation IDs --
  const [conversationId, setConversationId] = useState<string>(() => getOrCreateConversationId());
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() => getSessionId());

  // -- Timing --
  const startTimeRef = useRef<number | null>(null);
  const [durationMs, setDurationMs] = useState(0);
  const durationIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // -- Token streaming buffer (same pattern as WebChatPanel) --
  const streamingBufferRef = useRef("");
  const rafIdRef = useRef<number | null>(null);

  // -- Sequence-based dedup --
  const lastSeqRef = useRef(0);

  // -- Guard against double submission --
  const isSubmittingRef = useRef(false);

  // -- Map of tool_call_id to block id for correlating ToolResult --
  const toolCallBlockMapRef = useRef<Map<string, string>>(new Map());

  // ========================================================================
  // Duration timer
  // ========================================================================

  const startDurationTimer = useCallback(() => {
    startTimeRef.current = Date.now();
    if (durationIntervalRef.current) clearInterval(durationIntervalRef.current);
    durationIntervalRef.current = setInterval(() => {
      if (startTimeRef.current) {
        setDurationMs(Date.now() - startTimeRef.current);
      }
    }, 500);
  }, []);

  const stopDurationTimer = useCallback(() => {
    if (durationIntervalRef.current) {
      clearInterval(durationIntervalRef.current);
      durationIntervalRef.current = null;
    }
    if (startTimeRef.current) {
      setDurationMs(Date.now() - startTimeRef.current);
    }
  }, []);

  // ========================================================================
  // Flush streaming buffer
  // ========================================================================

  const flushTokenBuffer = useCallback(() => {
    const buffered = streamingBufferRef.current;
    if (!buffered) return;
    streamingBufferRef.current = "";
    rafIdRef.current = null;

    setBlocks((prev) => {
      const last = prev[prev.length - 1];
      if (last && last.type === "response" && last.isStreaming) {
        return [
          ...prev.slice(0, -1),
          { ...last, data: { ...last.data, content: (last.data.content as string) + buffered } },
        ];
      }
      // Create new response block
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "response",
          timestamp: now(),
          data: { content: buffered, timestamp: now() },
          isStreaming: true,
        },
      ];
    });
  }, []);

  // ========================================================================
  // Event handler
  // ========================================================================

  const handleStreamEvent = useCallback((event: StreamEvent) => {
    // Sequence-based dedup
    const seq = event.seq as number | undefined;
    if (seq !== undefined && seq > 0) {
      if (seq <= lastSeqRef.current) return;
      lastSeqRef.current = seq;
    }

    switch (event.type) {
      // ------------------------------------------------------------------
      // Session lifecycle
      // ------------------------------------------------------------------
      case "invoke_accepted": {
        if (event.session_id && typeof event.session_id === "string") {
          setSessionId(event.session_id);
          setActiveSessionId(event.session_id);
        }
        lastSeqRef.current = 0;
        break;
      }

      case "agent_started": {
        setStatus("running");
        startDurationTimer();
        if (event.session_id && typeof event.session_id === "string") {
          setSessionId(event.session_id);
          setActiveSessionId(event.session_id);
        }
        if (event.model && typeof event.model === "string") {
          setModelName(event.model);
        }
        break;
      }

      // ------------------------------------------------------------------
      // Token streaming
      // ------------------------------------------------------------------
      case "token": {
        const delta = (event.delta ?? event.content ?? "") as string;
        if (delta) {
          streamingBufferRef.current += delta;
          if (rafIdRef.current === null) {
            rafIdRef.current = requestAnimationFrame(flushTokenBuffer);
          }
        }
        // Track tokens from any available field
        const totalTok = (event.total_tokens ?? event.tokens_in ?? event.token_count) as number | undefined;
        if (typeof totalTok === "number" && totalTok > 0) {
          setTokenCount(totalTok);
        } else {
          // Increment by 1 per token event as fallback
          setTokenCount((prev) => prev + 1);
        }
        break;
      }

      // ------------------------------------------------------------------
      // Tool calls
      // ------------------------------------------------------------------
      case "tool_call": {
        const toolName = (event.tool ?? event.tool_name ?? "") as string;
        const args = parseArgs(event.arguments ?? event.args);
        const toolCallId = (event.tool_call_id ?? event.id ?? "") as string;
        const inputSummary = (event.input ?? JSON.stringify(args).slice(0, 200)) as string;

        // Check for special tool types
        const action = (args.action ?? args.operation ?? "") as string;

        // set_session_title — update title, no block
        if (toolName === "set_session_title") {
          const title = (args.title ?? args.name ?? "") as string;
          if (title) setSessionTitle(title);
          break;
        }

        // memory recall — type: 'recall'
        if (toolName === "memory" && action === "recall") {
          const blockId = crypto.randomUUID();
          if (toolCallId) toolCallBlockMapRef.current.set(toolCallId, blockId);
          setBlocks((prev) => [
            ...prev,
            {
              id: blockId,
              type: "recall",
              timestamp: now(),
              data: { raw: "" }, // Will be filled by tool_result
            },
          ]);
          break;
        }

        // update_plan — type: 'plan'
        if (toolName === "update_plan") {
          const steps: PlanStep[] = [];
          // Try structured steps first, then parse from plan text
          const rawSteps = args.steps ?? args.plan ?? args.content ?? "";
          if (Array.isArray(rawSteps)) {
            for (const s of rawSteps) {
              if (typeof s === "string") {
                steps.push({ text: s, status: "pending" as StepStatus });
              } else if (typeof s === "object" && s) {
                steps.push({
                  text: ((s as Record<string, unknown>).text ?? (s as Record<string, unknown>).description ?? "") as string,
                  status: ((s as Record<string, unknown>).status ?? "pending") as StepStatus,
                });
              }
            }
          } else if (typeof rawSteps === "string" && rawSteps.trim()) {
            // Parse plan text: split by newlines, treat each line as a step
            const lines = rawSteps.split("\n").filter((l: string) => l.trim());
            for (const line of lines) {
              const trimmed = line.replace(/^[\s\-\*\d.]+/, "").trim();
              if (trimmed) {
                const isDone = line.includes("[x]") || line.includes("✓");
                steps.push({ text: trimmed, status: isDone ? "done" as StepStatus : "pending" as StepStatus });
              }
            }
          }
          if (steps.length > 0) {
            setPlan(steps);
          }
          const blockId = crypto.randomUUID();
          if (toolCallId) toolCallBlockMapRef.current.set(toolCallId, blockId);
          setBlocks((prev) => [
            ...prev,
            {
              id: blockId,
              type: "plan",
              timestamp: now(),
              data: { steps: steps.length > 0 ? steps : [{ text: "Planning...", status: "active" as StepStatus }] },
            },
          ]);
          break;
        }

        // delegate_to_agent — type: 'delegation'
        if (toolName === "delegate_to_agent") {
          const delegateAgentId = (args.agent_id ?? args.agentId ?? "") as string;
          const task = (args.task ?? args.message ?? inputSummary) as string;
          const blockId = crypto.randomUUID();
          if (toolCallId) toolCallBlockMapRef.current.set(toolCallId, blockId);
          setBlocks((prev) => [
            ...prev,
            {
              id: blockId,
              type: "delegation",
              timestamp: now(),
              data: { agentId: delegateAgentId, task, status: "active" },
            },
          ]);
          setSubagents((prev) => [
            ...prev.filter((s) => !(s.agentId === delegateAgentId && s.status === "active")),
            { agentId: delegateAgentId, task, status: "active" },
          ]);
          break;
        }

        // Generic tool call — type: 'tool'
        {
          const blockId = crypto.randomUUID();
          if (toolCallId) toolCallBlockMapRef.current.set(toolCallId, blockId);
          setBlocks((prev) => [
            ...prev,
            {
              id: blockId,
              type: "tool",
              timestamp: now(),
              data: {
                toolName,
                input: inputSummary,
                isExpanded: false,
              },
              isExpanded: false,
            },
          ]);
        }
        break;
      }

      // ------------------------------------------------------------------
      // Tool results
      // ------------------------------------------------------------------
      case "tool_result": {
        const toolCallId = (event.tool_call_id ?? "") as string;
        const result = (event.result ?? event.output ?? "") as string;
        const isError = event.is_error === true || event.error === true;
        const blockId = toolCallId ? toolCallBlockMapRef.current.get(toolCallId) : undefined;

        if (blockId) {
          setBlocks((prev) => {
            const idx = prev.findIndex((b) => b.id === blockId);
            if (idx < 0) return prev;
            const block = prev[idx];
            const updated = [...prev];

            if (block.type === "recall") {
              // Fill recall block with result JSON
              updated[idx] = { ...block, data: { raw: result } };
              // Extract facts for sidebar
              try {
                const parsed = JSON.parse(result);
                // The recall tool returns { results: [...], formatted: "..." }
                const facts = parsed.results ?? parsed.facts ?? [];
                if (Array.isArray(facts) && facts.length > 0) {
                  setRecalledFacts(
                    facts.map((f: Record<string, unknown>) => ({
                      key: (f.key ?? "") as string,
                      content: (f.content ?? "") as string,
                      category: (f.category ?? "") as string,
                      confidence: (f.confidence ?? 0) as number,
                    }))
                  );
                }
              } catch { /* ignore parse failure */ }
            } else if (block.type === "tool") {
              updated[idx] = {
                ...block,
                data: {
                  ...block.data,
                  output: result,
                  isError,
                },
              };
              // Check for ward data in tool result
              const toolName = block.data.toolName as string;
              if (toolName === "ward" || toolName === "set_ward" || toolName === "enter_ward") {
                try {
                  const parsed = JSON.parse(result);
                  if (parsed.__ward_changed__ || parsed.action === "switched") {
                    const wardName = (parsed.ward_name ?? parsed.name ?? "unknown") as string;
                    const wardContent = (parsed.agents_md ?? parsed.content ?? "") as string;
                    setActiveWard({ name: wardName, content: wardContent.slice(0, 300) });
                  }
                } catch { /* not ward JSON, ignore */ }
              }
            } else if (block.type === "delegation") {
              updated[idx] = {
                ...block,
                data: { ...block.data, result, status: isError ? "error" : "completed" },
              };
            } else if (block.type === "plan") {
              // Plan results — might have updated steps
              try {
                const parsed = JSON.parse(result);
                if (Array.isArray(parsed.steps)) {
                  const steps = parsed.steps.map((s: Record<string, unknown>) => ({
                    text: (s.text ?? "") as string,
                    status: (s.status ?? "pending") as StepStatus,
                  }));
                  updated[idx] = { ...block, data: { steps } };
                  setPlan(steps);
                }
              } catch {
                // leave block as-is
              }
            }

            return updated;
          });
          toolCallBlockMapRef.current.delete(toolCallId);
        }
        break;
      }

      // ------------------------------------------------------------------
      // Delegation lifecycle (from server-side routing)
      // ------------------------------------------------------------------
      case "delegation_started": {
        const childAgentId = (event.child_agent_id ?? "") as string;
        const task = (event.task ?? "") as string;

        // Update existing delegation block if we have one
        setBlocks((prev) => {
          const idx = prev.findIndex(
            (b) => b.type === "delegation" && b.data.agentId === childAgentId && b.data.status === "active",
          );
          if (idx >= 0) {
            const updated = [...prev];
            updated[idx] = {
              ...updated[idx],
              data: { ...updated[idx].data, task: task || updated[idx].data.task },
            };
            return updated;
          }
          // If no existing block, create one
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "delegation",
              timestamp: now(),
              data: { agentId: childAgentId, task, status: "active" },
            },
          ];
        });

        setSubagents((prev) => {
          const existing = prev.find((s) => s.agentId === childAgentId && s.status === "active");
          if (existing) return prev;
          return [...prev, { agentId: childAgentId, task, status: "active" }];
        });
        break;
      }

      case "delegation_completed": {
        const childAgentId = (event.child_agent_id ?? "") as string;
        const result = (event.result ?? "") as string;

        setBlocks((prev) => {
          const idx = prev.findIndex(
            (b) => b.type === "delegation" && b.data.agentId === childAgentId && b.data.status === "active",
          );
          if (idx >= 0) {
            const updated = [...prev];
            updated[idx] = {
              ...updated[idx],
              data: { ...updated[idx].data, status: "completed", result },
            };
            return updated;
          }
          return prev;
        });

        setSubagents((prev) =>
          prev.map((s) =>
            s.agentId === childAgentId && s.status === "active"
              ? { ...s, status: "completed" }
              : s,
          ),
        );
        break;
      }

      case "delegation_error": {
        const childAgentId = (event.child_agent_id ?? "") as string;
        const error = (event.error ?? "") as string;

        setBlocks((prev) => {
          const idx = prev.findIndex(
            (b) => b.type === "delegation" && b.data.agentId === childAgentId && b.data.status === "active",
          );
          if (idx >= 0) {
            const updated = [...prev];
            updated[idx] = {
              ...updated[idx],
              data: { ...updated[idx].data, status: "error", result: error },
            };
            return updated;
          }
          return prev;
        });

        setSubagents((prev) =>
          prev.map((s) =>
            s.agentId === childAgentId && s.status === "active"
              ? { ...s, status: "error" }
              : s,
          ),
        );
        break;
      }

      // ------------------------------------------------------------------
      // Session title
      // ------------------------------------------------------------------
      case "session_title_changed": {
        const title = (event.title ?? "") as string;
        if (title) setSessionTitle(title);
        break;
      }

      // ------------------------------------------------------------------
      // Turn complete (respond tool output)
      // ------------------------------------------------------------------
      case "turn_complete": {
        // Flush any buffered tokens
        if (rafIdRef.current !== null) {
          cancelAnimationFrame(rafIdRef.current);
          rafIdRef.current = null;
        }
        flushTokenBuffer();

        const finalMessage = event.final_message as string | undefined;
        if (finalMessage) {
          setBlocks((prev) => {
            const lastIdx = prev.length - 1;
            const last = prev[lastIdx];
            if (last && last.type === "response" && last.isStreaming) {
              return [
                ...prev.slice(0, lastIdx),
                { ...last, data: { ...last.data, content: finalMessage }, isStreaming: false },
              ];
            }
            return [
              ...prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)),
              {
                id: crypto.randomUUID(),
                type: "response",
                timestamp: now(),
                data: { content: finalMessage, timestamp: now() },
                isStreaming: false,
              },
            ];
          });
        }
        break;
      }

      // ------------------------------------------------------------------
      // Agent completed / error
      // ------------------------------------------------------------------
      case "agent_completed": {
        if (rafIdRef.current !== null) {
          cancelAnimationFrame(rafIdRef.current);
          rafIdRef.current = null;
        }
        flushTokenBuffer();
        setStatus("completed");
        stopDurationTimer();
        // Finalize any streaming blocks
        setBlocks((prev) => prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)));
        break;
      }

      case "error": {
        if (rafIdRef.current !== null) {
          cancelAnimationFrame(rafIdRef.current);
          rafIdRef.current = null;
        }
        flushTokenBuffer();
        setStatus("error");
        stopDurationTimer();
        setBlocks((prev) => prev.map((b) => (b.isStreaming ? { ...b, isStreaming: false } : b)));
        break;
      }

      // Handle system messages (delegation callbacks, continuation triggers)
      case "system_message":
      case "message": {
        const content = (event.content ?? event.message ?? "") as string;
        if (content) {
          setBlocks((prev) => [
            ...prev,
            {
              id: crypto.randomUUID(),
              type: "response",
              timestamp: now(),
              data: { content, timestamp: now() },
              isStreaming: false,
            },
          ]);
        }
        break;
      }

      default:
        // Unhandled event types — no-op
        break;
    }
  }, [flushTokenBuffer, startDurationTimer, stopDurationTimer]);

  // ========================================================================
  // WebSocket subscription
  // ========================================================================

  useEffect(() => {
    const subscriptionKey = activeSessionId || conversationId;
    if (!subscriptionKey) return;

    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;

      unsubscribe = transport.subscribeConversation(subscriptionKey, {
        onEvent: handleStreamEvent,
        scope: "session",
      });
    };

    setup();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
      streamingBufferRef.current = "";
      lastSeqRef.current = 0;
    };
  }, [activeSessionId, conversationId, handleStreamEvent]);

  // ========================================================================
  // Cleanup duration interval on unmount
  // ========================================================================

  useEffect(() => {
    return () => {
      if (durationIntervalRef.current) {
        clearInterval(durationIntervalRef.current);
      }
    };
  }, []);

  // ========================================================================
  // Send message
  // ========================================================================

  const sendMessage = useCallback(
    async (text: string, attachments: UploadedFile[] = []) => {
      if (!text.trim() || isSubmittingRef.current) return;
      isSubmittingRef.current = true;

      // Add user message block
      const attachmentNames = attachments.map((a) => a.name);
      setBlocks((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          type: "user",
          timestamp: now(),
          data: {
            content: text.trim(),
            timestamp: now(),
            attachments: attachmentNames.length > 0 ? attachmentNames : undefined,
          },
        },
      ]);

      setStatus("running");
      startDurationTimer();

      try {
        const transport = await getTransport();
        const currentSessionId = getSessionId() ?? undefined;

        // Build message — if attachments, include their references
        let message = text.trim();
        if (attachments.length > 0) {
          const refs = attachments.map((a) => `[file:${a.id}:${a.name}]`).join(" ");
          message = `${message}\n\n${refs}`;
        }

        await transport.executeAgent(ROOT_AGENT_ID, conversationId, message, currentSessionId);
      } catch (error) {
        console.error("[MissionControl] Failed to send message:", error);
        setStatus("error");
        stopDurationTimer();
      } finally {
        isSubmittingRef.current = false;
      }
    },
    [conversationId, startDurationTimer, stopDurationTimer],
  );

  // ========================================================================
  // Stop agent
  // ========================================================================

  const stopAgent = useCallback(async () => {
    try {
      const transport = await getTransport();
      await transport.stopAgent(conversationId);
    } catch (error) {
      console.error("[MissionControl] Failed to stop agent:", error);
    }
  }, [conversationId]);

  // ========================================================================
  // Return state
  // ========================================================================

  // ========================================================================
  // Start new session — clears all state and creates fresh conversation ID
  // ========================================================================

  const startNewSession = useCallback(() => {
    const newConvId = createNewConversationId();
    setConversationId(newConvId);
    setActiveSessionId(null);
    setBlocks([]);
    setSessionTitle("");
    setStatus("idle");
    setTokenCount(0);
    setModelName("");
    setSubagents([]);
    setPlan([]);
    setRecalledFacts([]);
    setActiveWard(null);
    stopDurationTimer();
    setDurationMs(0);
    lastSeqRef.current = 0;
    streamingBufferRef.current = "";
    toolCallBlockMapRef.current.clear();
  }, [stopDurationTimer]);

  const state: MissionState = {
    blocks,
    sessionTitle,
    status,
    tokenCount,
    durationMs,
    modelName,
    subagents,
    plan,
    recalledFacts,
    activeWard,
  };

  return { state, sendMessage, stopAgent, startNewSession };
}
