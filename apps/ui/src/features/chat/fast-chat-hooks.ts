// ============================================================================
// FAST CHAT HOOKS
// Simplified version of mission-hooks for fast mode (no intent analysis).
// Flat message list, streaming tokens, tool call tracking.
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { getTransport, type StreamEvent } from "@/services/transport";

// ============================================================================
// Types
// ============================================================================

export interface FastMessage {
  id: string;
  role: "user" | "assistant" | "tool";
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

// ============================================================================
// Helpers
// ============================================================================

const ROOT_AGENT_ID = "root";
const FAST_CONV_ID_KEY = "zbot_fast_conv_id";
const FAST_SESSION_ID_KEY = "zbot_fast_session_id";

function getOrCreateConversationId(): string {
  let convId = localStorage.getItem(FAST_CONV_ID_KEY);
  if (!convId) {
    convId = `fast-${crypto.randomUUID()}`;
    localStorage.setItem(FAST_CONV_ID_KEY, convId);
  }
  return convId;
}

function createNewConversationId(): string {
  localStorage.removeItem(FAST_SESSION_ID_KEY);
  const convId = `fast-${crypto.randomUUID()}`;
  localStorage.setItem(FAST_CONV_ID_KEY, convId);
  return convId;
}

function getSessionId(): string | null {
  return localStorage.getItem(FAST_SESSION_ID_KEY);
}

function setSessionId(id: string): void {
  localStorage.setItem(FAST_SESSION_ID_KEY, id);
}

function now(): string {
  return new Date().toISOString();
}

// ============================================================================
// Hook: useFastChat
// ============================================================================

export function useFastChat() {
  const [messages, setMessages] = useState<FastMessage[]>([]);
  const [status, setStatus] = useState<FastChatState["status"]>("idle");

  const [conversationId, setConversationId] = useState<string>(() => {
    return getOrCreateConversationId();
  });
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() => getSessionId());

  // Streaming buffer
  const streamingBufferRef = useRef("");
  const rafIdRef = useRef<number | null>(null);

  // Sequence dedup
  const lastSeqRef = useRef(0);

  // Guard against double submission
  const isSubmittingRef = useRef(false);

  // Map tool_call_id -> message id
  const toolCallMsgMapRef = useRef<Map<string, string>>(new Map());

  // ========================================================================
  // Flush streaming buffer
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
            setActiveSessionId(event.session_id);
          }
          lastSeqRef.current = 0;
          break;
        }

        case "agent_started": {
          setStatus("running");
          if (event.session_id && typeof event.session_id === "string") {
            setSessionId(event.session_id);
            setActiveSessionId(event.session_id);
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
        // Tool calls
        // ----------------------------------------------------------------
        case "tool_call": {
          const toolName = (event.tool ?? event.tool_name ?? "") as string;
          const toolCallId = (event.tool_call_id ?? event.id ?? "") as string;

          // Skip internal tools that don't need display
          if (toolName === "set_session_title" || toolName === "respond") break;

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
    [flushTokenBuffer]
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

      if (activeSessionId && activeSessionId !== conversationId) {
        unsubs.push(
          transport.subscribeConversation(activeSessionId, {
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
      streamingBufferRef.current = "";
      lastSeqRef.current = 0;
    };
  }, [activeSessionId, conversationId, handleStreamEvent]);

  // ========================================================================
  // Send message
  // ========================================================================

  const sendMessage = useCallback(
    async (text: string) => {
      if (!text.trim() || isSubmittingRef.current) return;
      isSubmittingRef.current = true;

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
        const currentSessionId = getSessionId() ?? undefined;
        await transport.executeAgent(
          ROOT_AGENT_ID,
          conversationId,
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
    [conversationId]
  );

  // ========================================================================
  // Stop agent
  // ========================================================================

  const stopAgent = useCallback(async () => {
    try {
      const transport = await getTransport();
      await transport.stopAgent(conversationId);
    } catch (error) {
      console.error("[FastChat] Failed to stop agent:", error);
    }
  }, [conversationId]);

  // ========================================================================
  // Start new session
  // ========================================================================

  const startNewSession = useCallback(() => {
    const newConvId = createNewConversationId();
    setConversationId(newConvId);
    setActiveSessionId(null);
    setMessages([]);
    setStatus("idle");
    lastSeqRef.current = 0;
    streamingBufferRef.current = "";
    toolCallMsgMapRef.current.clear();
  }, []);

  const state: FastChatState = { messages, status };

  return { state, sendMessage, stopAgent, startNewSession };
}
