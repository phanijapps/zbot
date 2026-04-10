// ============================================================================
// FAST CHAT HOOKS
// Simplified version of mission-hooks for fast mode (no intent analysis).
// Flat message list, streaming tokens, tool call tracking.
// Persistent session via POST /api/chat/init.
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { getTransport, type StreamEvent } from "@/services/transport";

// ============================================================================
// Types
// ============================================================================

export interface FastMessage {
  id: string;
  role: "user" | "assistant" | "tool" | "thinking";
  content: string;
  timestamp: string;
  /** For tool messages */
  toolName?: string;
  toolOutput?: string;
  isError?: boolean;
  /** Whether this message is still being streamed */
  isStreaming?: boolean;
}

export interface FastChatState {
  messages: FastMessage[];
  status: "idle" | "running" | "completed" | "error";
}

export interface UseFastChatResult {
  state: FastChatState;
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
// Hook: useFastChat
// ============================================================================

export function useFastChat(): UseFastChatResult {
  const [messages, setMessages] = useState<FastMessage[]>([]);
  const [status, setStatus] = useState<FastChatState["status"]>("idle");
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [showThinking, setShowThinking] = useState(false);
  const [initializing, setInitializing] = useState(true);

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

        // Load existing messages
        const msgResult = await fetch(
          `/api/sessions/${encodeURIComponent(sid)}/messages?limit=100`
        );
        if (msgResult.ok) {
          const msgs = await msgResult.json();
          if (!cancelled && Array.isArray(msgs)) {
            const mapped: FastMessage[] = msgs.map((m: Record<string, unknown>) => ({
              id: (m.id as string) || crypto.randomUUID(),
              role: (m.role as FastMessage["role"]) || "assistant",
              content: (m.content as string) || "",
              timestamp: (m.createdAt as string) || now(),
              toolName: m.toolCalls ? extractToolName(m.toolCalls) : undefined,
              toolOutput: m.toolResults ? String(m.toolResults) : undefined,
            }));
            setMessages(mapped);
          }
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

      switch (event.type) {
        case "invoke_accepted": {
          if (event.session_id && typeof event.session_id === "string") {
            setSessionId(event.session_id);
            sessionIdRef.current = event.session_id;
          }
          lastSeqRef.current = 0;
          break;
        }

        case "agent_started": {
          setStatus("running");
          if (event.session_id && typeof event.session_id === "string") {
            setSessionId(event.session_id);
            sessionIdRef.current = event.session_id;
          }
          break;
        }

        // ----------------------------------------------------------------
        // Token streaming
        // ----------------------------------------------------------------
        case "token": {
          const delta = (event.delta ?? event.content ?? "") as string;
          if (delta) {
            streamingBufferRef.current += delta;
            if (rafIdRef.current === null) {
              rafIdRef.current = requestAnimationFrame(flushTokenBuffer);
            }
          }
          break;
        }

        // ----------------------------------------------------------------
        // Thinking / reasoning streaming
        // ----------------------------------------------------------------
        case "thinking":
        case "reasoning": {
          const delta = (event.delta ?? event.content ?? "") as string;
          if (delta) {
            thinkingBufferRef.current += delta;
            if (thinkingRafIdRef.current === null) {
              thinkingRafIdRef.current = requestAnimationFrame(flushThinkingBuffer);
            }
          }
          break;
        }

        // ----------------------------------------------------------------
        // Tool calls
        // ----------------------------------------------------------------
        case "tool_call": {
          const toolName = (event.tool ?? event.tool_name ?? "") as string;
          const toolCallId = (event.tool_call_id ?? event.id ?? "") as string;

          // Skip internal tools that don't need display
          if (toolName === "set_session_title" || toolName === "respond") break;

          // Finalize any streaming thinking message before tool call
          setMessages((prev) =>
            prev.map((m) =>
              m.role === "thinking" && m.isStreaming
                ? { ...m, isStreaming: false }
                : m
            )
          );

          const msgId = crypto.randomUUID();
          if (toolCallId) toolCallMsgMapRef.current.set(toolCallId, msgId);

          setMessages((prev) => [
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
          break;
        }

        // ----------------------------------------------------------------
        // Tool results
        // ----------------------------------------------------------------
        case "tool_result": {
          const toolCallId = (event.tool_call_id ?? "") as string;
          const result = (event.result ?? event.output ?? "") as string;
          const isError = event.is_error === true || event.error === true;
          const msgId = toolCallId
            ? toolCallMsgMapRef.current.get(toolCallId)
            : undefined;

          if (msgId) {
            setMessages((prev) => {
              const idx = prev.findIndex((m) => m.id === msgId);
              if (idx < 0) return prev;
              const updated = [...prev];
              updated[idx] = {
                ...updated[idx],
                toolOutput: result,
                isError,
              };
              return updated;
            });
            toolCallMsgMapRef.current.delete(toolCallId);
          }
          break;
        }

        // ----------------------------------------------------------------
        // Turn complete
        // ----------------------------------------------------------------
        case "turn_complete": {
          if (rafIdRef.current !== null) {
            cancelAnimationFrame(rafIdRef.current);
            rafIdRef.current = null;
          }
          flushTokenBuffer();

          if (thinkingRafIdRef.current !== null) {
            cancelAnimationFrame(thinkingRafIdRef.current);
            thinkingRafIdRef.current = null;
          }
          flushThinkingBuffer();

          const finalMessage = event.final_message as string | undefined;
          if (finalMessage) {
            setMessages((prev) => {
              const lastIdx = prev.length - 1;
              const last = prev[lastIdx];
              if (last && last.role === "assistant" && last.isStreaming) {
                return [
                  ...prev.slice(0, lastIdx),
                  { ...last, content: finalMessage, isStreaming: false },
                ];
              }
              return [
                ...prev.map((m) =>
                  m.isStreaming ? { ...m, isStreaming: false } : m
                ),
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
          break;
        }

        // ----------------------------------------------------------------
        // Agent completed / error
        // ----------------------------------------------------------------
        case "agent_completed": {
          if (rafIdRef.current !== null) {
            cancelAnimationFrame(rafIdRef.current);
            rafIdRef.current = null;
          }
          flushTokenBuffer();

          if (thinkingRafIdRef.current !== null) {
            cancelAnimationFrame(thinkingRafIdRef.current);
            thinkingRafIdRef.current = null;
          }
          flushThinkingBuffer();

          const result = event.result as string | undefined;
          if (result) {
            setMessages((prev) => {
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

          setStatus("completed");
          setMessages((prev) =>
            prev.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m))
          );
          break;
        }

        case "error": {
          if (rafIdRef.current !== null) {
            cancelAnimationFrame(rafIdRef.current);
            rafIdRef.current = null;
          }
          flushTokenBuffer();

          if (thinkingRafIdRef.current !== null) {
            cancelAnimationFrame(thinkingRafIdRef.current);
            thinkingRafIdRef.current = null;
          }
          flushThinkingBuffer();

          setStatus("error");
          setMessages((prev) =>
            prev.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m))
          );
          break;
        }

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

    const unsubs: (() => void)[] = [];
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;

      unsubs.push(
        transport.subscribeConversation(conversationId, {
          onEvent: handleStreamEvent,
          scope: "session",
        })
      );

      if (sessionId && sessionId !== conversationId) {
        unsubs.push(
          transport.subscribeConversation(sessionId, {
            onEvent: handleStreamEvent,
            scope: "session",
          })
        );
      }
    };

    setup();

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
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
  }, [sessionId, conversationId, handleStreamEvent]);

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

  return { state, sendMessage, stopAgent, showThinking, setShowThinking, initializing };
}
