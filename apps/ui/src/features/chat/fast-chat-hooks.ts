// ============================================================================
// FAST CHAT HOOKS
// Simplified version of mission-hooks for fast mode (no intent analysis).
// Flat message list, streaming tokens, tool call tracking.
// Persistent session via POST /api/chat/init.
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { getTransport, type StreamEvent } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";

// ============================================================================
// Types
// ============================================================================

export interface FastMessage {
  id: string;
  role: "user" | "assistant" | "tool" | "thinking" | "delegation";
  content: string;
  timestamp: string;
  /** For tool messages */
  toolName?: string;
  toolOutput?: string;
  isError?: boolean;
  /** Delegation fields */
  delegationAgent?: string;
  delegationTask?: string;
  delegationStatus?: "running" | "completed" | "error";
  delegationResult?: string;
  /** Whether this message is still being streamed */
  isStreaming?: boolean;
}

export interface FastChatState {
  messages: FastMessage[];
  status: "idle" | "running" | "completed" | "error";
}

export interface UseFastChatResult {
  state: FastChatState;
  artifacts: Artifact[];
  sendMessage: (text: string) => Promise<void>;
  stopAgent: () => Promise<void>;
  showThinking: boolean;
  setShowThinking: (v: boolean) => void;
  initializing: boolean;
}

// ============================================================================
// Helpers
// ============================================================================

const ROOT_AGENT_ID = "root";

function now(): string {
  return new Date().toISOString();
}

/**
 * Extract a display-friendly tool name from tool_calls data.
 */
function extractToolName(toolCalls: unknown): string | undefined {
  if (Array.isArray(toolCalls) && toolCalls.length > 0) {
    const first = toolCalls[0];
    if (typeof first === "object" && first !== null) {
      const tc = first as Record<string, unknown>;
      return (tc.name ?? tc.tool ?? tc.function ?? "") as string || undefined;
    }
  }
  if (typeof toolCalls === "string") return toolCalls;
  return undefined;
}

// ============================================================================
// Event handler context
// ============================================================================

interface FastChatEventCtx {
  setMessages: React.Dispatch<React.SetStateAction<FastMessage[]>>;
  setStatus: React.Dispatch<React.SetStateAction<FastChatState["status"]>>;
  setArtifacts: React.Dispatch<React.SetStateAction<Artifact[]>>;
  sessionIdRef: React.MutableRefObject<string | null>;
  toolCallMsgMapRef: React.MutableRefObject<Map<string, string>>;
  streamingBufferRef: React.MutableRefObject<string>;
  rafIdRef: React.MutableRefObject<number | null>;
  thinkingBufferRef: React.MutableRefObject<string>;
  thinkingRafIdRef: React.MutableRefObject<number | null>;
  flushTokenBuffer: () => void;
  flushThinkingBuffer: () => void;
  setSessionId: (id: string) => void;
}

// ============================================================================
// Extracted event handlers
// ============================================================================

function fastHandleInvokeAccepted(event: StreamEvent, ctx: FastChatEventCtx): void {
  if (event.session_id && typeof event.session_id === "string") {
    ctx.setSessionId(event.session_id);
  }
}

function fastHandleAgentStarted(event: StreamEvent, ctx: FastChatEventCtx): void {
  ctx.setStatus("running");
  if (event.session_id && typeof event.session_id === "string") {
    ctx.setSessionId(event.session_id);
  }
}

function fastHandleTokenEvent(event: StreamEvent, ctx: FastChatEventCtx): void {
  const delta = (event.delta ?? event.content ?? "") as string;
  if (delta) {
    ctx.streamingBufferRef.current += delta;
    if (ctx.rafIdRef.current === null) {
      ctx.rafIdRef.current = requestAnimationFrame(ctx.flushTokenBuffer);
    }
  }
}

function fastHandleThinkingEvent(event: StreamEvent, ctx: FastChatEventCtx): void {
  const delta = (event.delta ?? event.content ?? "") as string;
  if (delta) {
    ctx.thinkingBufferRef.current += delta;
    if (ctx.thinkingRafIdRef.current === null) {
      ctx.thinkingRafIdRef.current = requestAnimationFrame(ctx.flushThinkingBuffer);
    }
  }
}

function fastHandleToolCallEvent(event: StreamEvent, ctx: FastChatEventCtx): void {
  const toolName = (event.tool ?? event.tool_name ?? "") as string;
  const toolCallId = (event.tool_call_id ?? event.id ?? "") as string;

  if (toolName === "set_session_title" || toolName === "respond") return;

  ctx.setMessages((prev) =>
    prev.map((m) =>
      m.role === "thinking" && m.isStreaming ? { ...m, isStreaming: false } : m
    )
  );

  const msgId = crypto.randomUUID();
  if (toolCallId) ctx.toolCallMsgMapRef.current.set(toolCallId, msgId);

  ctx.setMessages((prev) => [
    ...prev,
    {
      id: msgId,
      role: "tool",
      content: "",
      timestamp: now(),
      toolName,
      toolOutput: "",
    },
  ]);
}

function fastHandleToolResultEvent(event: StreamEvent, ctx: FastChatEventCtx): void {
  const toolCallId = (event.tool_call_id ?? "") as string;
  const result = (event.result ?? event.output ?? "") as string;
  const isError = event.is_error === true || event.error === true;
  const msgId = toolCallId ? ctx.toolCallMsgMapRef.current.get(toolCallId) : undefined;

  if (msgId) {
    ctx.setMessages((prev) => {
      const idx = prev.findIndex((m) => m.id === msgId);
      if (idx < 0) return prev;
      const updated = [...prev];
      updated[idx] = { ...updated[idx], toolOutput: result, isError };
      return updated;
    });
    ctx.toolCallMsgMapRef.current.delete(toolCallId);
  }
}

function fastFlushAllBuffers(ctx: FastChatEventCtx): void {
  if (ctx.rafIdRef.current !== null) {
    cancelAnimationFrame(ctx.rafIdRef.current);
    ctx.rafIdRef.current = null;
  }
  ctx.flushTokenBuffer();

  if (ctx.thinkingRafIdRef.current !== null) {
    cancelAnimationFrame(ctx.thinkingRafIdRef.current);
    ctx.thinkingRafIdRef.current = null;
  }
  ctx.flushThinkingBuffer();
}

function fastHandleTurnComplete(event: StreamEvent, ctx: FastChatEventCtx): void {
  fastFlushAllBuffers(ctx);

  const finalMessage = event.final_message as string | undefined;
  if (finalMessage) {
    ctx.setMessages((prev) => {
      const lastIdx = prev.length - 1;
      const last = prev[lastIdx];
      if (last && last.role === "assistant" && last.isStreaming) {
        return [
          ...prev.slice(0, lastIdx),
          { ...last, content: finalMessage, isStreaming: false },
        ];
      }
      // Check if this message already exists (e.g., loaded from history)
      const alreadyExists = prev.some(
        (m) => m.role === "assistant" && m.content === finalMessage
      );
      if (alreadyExists) {
        return prev.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m));
      }
      return [
        ...prev.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m)),
        {
          id: crypto.randomUUID(),
          role: "assistant" as const,
          content: finalMessage,
          timestamp: now(),
          isStreaming: false,
        },
      ];
    });
  }
}

function fastHandleAgentCompleted(event: StreamEvent, ctx: FastChatEventCtx): void {
  fastFlushAllBuffers(ctx);

  const result = event.result as string | undefined;
  if (result) {
    ctx.setMessages((prev) => {
      const hasResponse = prev.some((m) => m.role === "assistant");
      if (hasResponse) return prev;
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "assistant" as const,
          content: result,
          timestamp: now(),
          isStreaming: false,
        },
      ];
    });
  }

  ctx.setStatus("completed");
  ctx.setMessages((prev) => prev.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m)));

  if (ctx.sessionIdRef.current) {
    getTransport().then((t) =>
      t.listSessionArtifacts(ctx.sessionIdRef.current!).then((r) => {
        if (r.success && r.data) ctx.setArtifacts(r.data);
      })
    );
  }
}

function fastHandleErrorEvent(ctx: FastChatEventCtx): void {
  fastFlushAllBuffers(ctx);
  ctx.setStatus("error");
  ctx.setMessages((prev) => prev.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m)));
}

function fastHandleDelegationStarted(event: StreamEvent, ctx: FastChatEventCtx): void {
  const childAgent = (event.child_agent_id ?? "") as string;
  const task = (event.task ?? "") as string;
  const childExecId = (event.child_execution_id ?? "") as string;

  const msgId = crypto.randomUUID();
  if (childExecId) ctx.toolCallMsgMapRef.current.set(`delegation:${childExecId}`, msgId);

  ctx.setMessages((prev) => [
    ...prev,
    {
      id: msgId,
      role: "delegation",
      content: "",
      timestamp: now(),
      delegationAgent: childAgent,
      delegationTask: task,
      delegationStatus: "running",
    },
  ]);
}

function fastHandleDelegationCompleted(event: StreamEvent, ctx: FastChatEventCtx): void {
  const childExecId = (event.child_execution_id ?? "") as string;
  const result = (event.result ?? "") as string;
  const msgId = ctx.toolCallMsgMapRef.current.get(`delegation:${childExecId}`);

  if (msgId) {
    ctx.setMessages((prev) => {
      const idx = prev.findIndex((m) => m.id === msgId);
      if (idx < 0) return prev;
      const updated = [...prev];
      updated[idx] = { ...updated[idx], delegationStatus: "completed", delegationResult: result };
      return updated;
    });
    ctx.toolCallMsgMapRef.current.delete(`delegation:${childExecId}`);
  }
}

// ============================================================================
// Hook: useFastChat
// ============================================================================

export function useFastChat(): UseFastChatResult {
  const [messages, setMessages] = useState<FastMessage[]>([]);
  const [status, setStatus] = useState<FastChatState["status"]>("idle");
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [showThinking, setShowThinking] = useState(false);
  const [initializing, setInitializing] = useState(true);
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);

  // Streaming buffer
  const streamingBufferRef = useRef("");
  const rafIdRef = useRef<number | null>(null);

  // Thinking streaming buffer
  const thinkingBufferRef = useRef("");
  const thinkingRafIdRef = useRef<number | null>(null);

  // Sequence dedup
  const lastSeqRef = useRef(0);

  // Guard against double submission
  const isSubmittingRef = useRef(false);

  // Map tool_call_id -> message id
  const toolCallMsgMapRef = useRef<Map<string, string>>(new Map());

  // Stable refs for session/conversation IDs (avoid stale closures)
  const sessionIdRef = useRef<string | null>(null);
  const conversationIdRef = useRef<string | null>(null);

  // Keep refs in sync
  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);
  useEffect(() => {
    conversationIdRef.current = conversationId;
  }, [conversationId]);

  // ========================================================================
  // On mount: init session + load history
  // ========================================================================

  useEffect(() => {
    let cancelled = false;

    async function init() {
      try {
        // Ensure transport is ready (initializes WebSocket)
        await getTransport();

        // Get or create persistent chat session
        const initResult = await fetch("/api/chat/init", { method: "POST" });
        if (!initResult.ok) {
          console.error("[FastChat] Failed to init session:", initResult.statusText);
          setInitializing(false);
          return;
        }
        const { sessionId: sid, conversationId: cid } = await initResult.json();
        if (cancelled) return;

        setSessionId(sid);
        setConversationId(cid);
        sessionIdRef.current = sid;
        conversationIdRef.current = cid;

        // Load existing messages from DB — this is the ONLY source of truth on mount.
        // Clear any stale streaming state before setting history.
        streamingBufferRef.current = "";
        thinkingBufferRef.current = "";
        lastSeqRef.current = 0;
        toolCallMsgMapRef.current.clear();

        const msgResult = await fetch(
          `/api/sessions/${encodeURIComponent(sid)}/messages?limit=100`
        );
        if (msgResult.ok) {
          const msgs = await msgResult.json();
          if (!cancelled && Array.isArray(msgs)) {
            // Use message IDs from DB to prevent duplicates with streaming
            const mapped: FastMessage[] = msgs.map((m: Record<string, unknown>) => ({
              id: (m.id as string) || crypto.randomUUID(),
              role: (m.role as FastMessage["role"]) || "assistant",
              content: (m.content as string) || "",
              timestamp: (m.createdAt as string) || now(),
              toolName: m.toolCalls ? extractToolName(m.toolCalls) : undefined,
              toolOutput: m.toolResults ? String(m.toolResults) : undefined,
              isStreaming: false, // History messages are never streaming
            }));
            setMessages(mapped);
          }
        } else {
          // No history — start with empty
          setMessages([]);
        }
      } catch (error) {
        console.error("[FastChat] Init error:", error);
      } finally {
        if (!cancelled) {
          setInitializing(false);
        }
      }
    }

    init();
    return () => {
      cancelled = true;
    };
  }, []);

  // ========================================================================
  // Flush streaming buffer (assistant tokens)
  // ========================================================================

  const flushTokenBuffer = useCallback(() => {
    const buffered = streamingBufferRef.current;
    if (!buffered) return;
    streamingBufferRef.current = "";
    rafIdRef.current = null;

    setMessages((prev) => {
      // Find last streaming assistant message
      let targetIdx = -1;
      for (let i = prev.length - 1; i >= 0; i--) {
        if (prev[i].role === "assistant" && prev[i].isStreaming) {
          targetIdx = i;
          break;
        }
      }

      if (targetIdx >= 0) {
        const updated = [...prev];
        updated[targetIdx] = {
          ...updated[targetIdx],
          content: updated[targetIdx].content + buffered,
        };
        return updated;
      }

      // No streaming message found — create one
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content: buffered,
          timestamp: now(),
          isStreaming: true,
        },
      ];
    });
  }, []);

  // ========================================================================
  // Flush thinking buffer
  // ========================================================================

  const flushThinkingBuffer = useCallback(() => {
    const buffered = thinkingBufferRef.current;
    if (!buffered) return;
    thinkingBufferRef.current = "";
    thinkingRafIdRef.current = null;

    setMessages((prev) => {
      // Find last streaming thinking message
      let targetIdx = -1;
      for (let i = prev.length - 1; i >= 0; i--) {
        if (prev[i].role === "thinking" && prev[i].isStreaming) {
          targetIdx = i;
          break;
        }
      }

      if (targetIdx >= 0) {
        const updated = [...prev];
        updated[targetIdx] = {
          ...updated[targetIdx],
          content: updated[targetIdx].content + buffered,
        };
        return updated;
      }

      // No streaming thinking message found — create one
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "thinking",
          content: buffered,
          timestamp: now(),
          isStreaming: true,
        },
      ];
    });
  }, []);

  // ========================================================================
  // Event handler
  // ========================================================================

  const handleStreamEvent = useCallback(
    (event: StreamEvent) => {
      const seq = event.seq as number | undefined;
      if (seq !== undefined && seq > 0) {
        if (seq <= lastSeqRef.current) return;
        lastSeqRef.current = seq;
      }

      const ctx: FastChatEventCtx = {
        setMessages,
        setStatus,
        setArtifacts,
        sessionIdRef,
        toolCallMsgMapRef,
        streamingBufferRef,
        rafIdRef,
        thinkingBufferRef,
        thinkingRafIdRef,
        flushTokenBuffer,
        flushThinkingBuffer,
        setSessionId: (id: string) => {
          setSessionId(id);
          sessionIdRef.current = id;
        },
      };

      switch (event.type) {
        case "invoke_accepted":
          fastHandleInvokeAccepted(event, ctx);
          lastSeqRef.current = 0;
          break;
        case "agent_started":
          fastHandleAgentStarted(event, ctx);
          break;
        case "token":
          fastHandleTokenEvent(event, ctx);
          break;
        case "thinking":
        case "reasoning":
          fastHandleThinkingEvent(event, ctx);
          break;
        case "tool_call":
          fastHandleToolCallEvent(event, ctx);
          break;
        case "tool_result":
          fastHandleToolResultEvent(event, ctx);
          break;
        case "turn_complete":
          fastHandleTurnComplete(event, ctx);
          break;
        case "agent_completed":
          fastHandleAgentCompleted(event, ctx);
          break;
        case "error":
          fastHandleErrorEvent(ctx);
          break;
        case "delegation_started":
          fastHandleDelegationStarted(event, ctx);
          break;
        case "delegation_completed":
          fastHandleDelegationCompleted(event, ctx);
          break;
        default:
          break;
      }
    },
    [flushTokenBuffer, flushThinkingBuffer]
  );

  // ========================================================================
  // WebSocket subscription
  // ========================================================================

  useEffect(() => {
    if (!conversationId) return;

    let unsub: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;

      // Subscribe to conversationId only — events route through this single subscription.
      // Do NOT also subscribe to sessionId — that causes duplicate events.
      unsub = transport.subscribeConversation(conversationId, {
        onEvent: handleStreamEvent,
        scope: "session",
      });
    };

    setup();

    return () => {
      cancelled = true;
      unsub?.();
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
      if (thinkingRafIdRef.current !== null) {
        cancelAnimationFrame(thinkingRafIdRef.current);
        thinkingRafIdRef.current = null;
      }
      streamingBufferRef.current = "";
      thinkingBufferRef.current = "";
      lastSeqRef.current = 0;
    };
  }, [conversationId, handleStreamEvent]);

  // ========================================================================
  // Send message
  // ========================================================================

  const sendMessage = useCallback(
    async (text: string) => {
      if (!text.trim() || isSubmittingRef.current) return;
      isSubmittingRef.current = true;

      const cid = conversationIdRef.current;
      if (!cid) {
        console.error("[FastChat] No conversationId, cannot send");
        isSubmittingRef.current = false;
        return;
      }

      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "user",
          content: text.trim(),
          timestamp: now(),
        },
      ]);

      setStatus("running");

      try {
        const transport = await getTransport();
        const currentSessionId = sessionIdRef.current ?? undefined;
        await transport.executeAgent(
          ROOT_AGENT_ID,
          cid,
          text.trim(),
          currentSessionId,
          "fast"
        );
      } catch (error) {
        console.error("[FastChat] Failed to send message:", error);
        setStatus("error");
      } finally {
        isSubmittingRef.current = false;
      }
    },
    []
  );

  // ========================================================================
  // Stop agent
  // ========================================================================

  const stopAgent = useCallback(async () => {
    const cid = conversationIdRef.current;
    if (!cid) return;
    try {
      const transport = await getTransport();
      await transport.stopAgent(cid);
    } catch (error) {
      console.error("[FastChat] Failed to stop agent:", error);
    }
  }, []);

  const state: FastChatState = { messages, status };

  return { state, artifacts, sendMessage, stopAgent, showThinking, setShowThinking, initializing };
}
