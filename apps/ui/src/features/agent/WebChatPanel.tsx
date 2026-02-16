// ============================================================================
// WEB CHAT PANEL
// Chat interface for the web dashboard (uses transport layer instead of Tauri)
// ============================================================================

import { useState, useEffect, useRef, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import { MessageSquare, Send, Loader2, Wrench, User, Bot, GitBranch, CheckCircle2, Info, RotateCcw, StopCircle } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { getTransport, type StreamEvent, type MessageResponse, type SessionMessage } from "@/services/transport";
import type { ShowContentEvent, RequestInputEvent } from "@/shared/types";
import { GenerativeCanvas, type ContentState } from "./GenerativeCanvas";
import { SubagentActivityPanel, type SubagentActivity } from "./SubagentActivityPanel";
import { TruncatedContent } from "./TruncatedContent";
import { ConnectionStatus } from "@/components/ConnectionStatus";

// ============================================================================
// Types
// ============================================================================

interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "tool" | "delegation" | "system";
  content: string;
  timestamp: Date;
  toolName?: string;
  isStreaming?: boolean;
  delegationStatus?: "started" | "completed";
  childAgentId?: string;
}

// ActiveDelegation is now SubagentActivity from the panel component

// ============================================================================
// Component
// ============================================================================

const ROOT_AGENT_ID = "root";
const WEB_CONV_ID_KEY = "agentzero_web_conv_id";
const WEB_SESSION_ID_KEY = "agentzero_web_session_id";

// Get or create a stable conversation ID
function getOrCreateConversationId(): string {
  let convId = localStorage.getItem(WEB_CONV_ID_KEY);
  if (!convId) {
    convId = `web-${crypto.randomUUID()}`;
    localStorage.setItem(WEB_CONV_ID_KEY, convId);
  }
  return convId;
}

// Create a new conversation ID and clear session
function createNewConversationId(): string {
  const convId = `web-${crypto.randomUUID()}`;
  localStorage.setItem(WEB_CONV_ID_KEY, convId);
  // Clear session_id when starting a new conversation
  localStorage.removeItem(WEB_SESSION_ID_KEY);
  return convId;
}

// Get the current session ID (if any)
function getSessionId(): string | null {
  return localStorage.getItem(WEB_SESSION_ID_KEY);
}

// Store the session ID from backend
function setSessionId(sessionId: string): void {
  localStorage.setItem(WEB_SESSION_ID_KEY, sessionId);
}

export function WebChatPanel() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [conversationId, setConversationId] = useState<string>(() => getOrCreateConversationId());
  const [isLoadingHistory, setIsLoadingHistory] = useState(true);
  const [reloadTrigger, setReloadTrigger] = useState(0);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Session ID - used for subscription routing (prefer sess-xxx over web-xxx)
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() => getSessionId());

  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [canvasContent, setCanvasContent] = useState<ContentState>(null);
  const [, setPendingFormId] = useState<string | null>(null);

  // Delegation tracking - uses SubagentActivity for detailed tracking
  const [subagentActivities, setSubagentActivities] = useState<Map<string, SubagentActivity>>(new Map());

  // Token streaming buffer - prevents garbled text from rapid/duplicate events
  const streamingBufferRef = useRef<string>("");
  const rafIdRef = useRef<number | null>(null);
  const lastSeqRef = useRef<number>(0);

  // Tool call collapse — single rolling message instead of one per call
  const toolCallCountRef = useRef(0);
  const toolActivityIdRef = useRef<string | null>(null);

  // Track whether respond content was already displayed this execution (dedup turn_complete vs agent_completed)
  const respondDisplayedRef = useRef(false);

  // Synchronous guard to prevent double-submission (React state is async)
  const isSubmittingRef = useRef(false);

  // Handle ?new=1 param to start a fresh session
  useEffect(() => {
    if (searchParams.get("new") === "1") {
      // Clear the param to avoid re-triggering on re-renders
      setSearchParams({}, { replace: true });
      // Create a new conversation (clears session and creates new conv ID)
      const newConvId = createNewConversationId();
      setConversationId(newConvId);
      setActiveSessionId(null);
      setMessages([]);
      setSubagentActivities(new Map());
    }
  }, [searchParams, setSearchParams]);

  // Load conversation history on mount, when conversationId changes, or on reloadTrigger
  useEffect(() => {
    const loadHistory = async () => {
      setIsLoadingHistory(true);
      try {
        const transport = await getTransport();

        // Prefer session-based query (returns actual data) over conversation-based
        // (web-xxx IDs don't match any DB records — known gap)
        if (activeSessionId) {
          const result = await transport.getSessionMessages(activeSessionId, { scope: "root" });
          if (result.success && result.data && result.data.length > 0) {
            const loadedMessages: ChatMessage[] = result.data
              .filter((m: SessionMessage) => m.role !== "tool" && !m.tool_calls)
              .map((m: SessionMessage) => ({
                id: m.id,
                role: m.role as "user" | "assistant" | "delegation",
                content: m.content,
                timestamp: new Date(m.created_at),
                isStreaming: false,
              }));
            setMessages(loadedMessages);
          }
        } else if (conversationId) {
          // Fallback for first load before session is established
          const result = await transport.getMessages(conversationId);
          if (result.success && result.data && result.data.length > 0) {
            const loadedMessages: ChatMessage[] = result.data
              .filter((m: MessageResponse) => m.role !== "tool")
              .map((m: MessageResponse) => ({
                id: m.id,
                role: m.role as "user" | "assistant" | "delegation",
                content: m.content,
                timestamp: new Date(m.timestamp),
                isStreaming: false,
              }));
            setMessages(loadedMessages);
          }
        }
      } catch (error) {
        console.error("Failed to load conversation history:", error);
      } finally {
        setIsLoadingHistory(false);
      }
    };

    loadHistory();
  }, [conversationId, activeSessionId, reloadTrigger]);

  // Flush buffered token deltas to state (called via requestAnimationFrame)
  const flushTokenBuffer = useCallback(() => {
    const buffered = streamingBufferRef.current;
    if (!buffered) return;
    streamingBufferRef.current = "";
    rafIdRef.current = null;

    setMessages((prev) => {
      const last = prev[prev.length - 1];
      if (last && last.role === "assistant" && last.isStreaming) {
        return [
          ...prev.slice(0, -1),
          { ...last, content: last.content + buffered },
        ];
      }
      return [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content: buffered,
          timestamp: new Date(),
          isStreaming: true,
        },
      ];
    });
  }, []);

  // Event handler for stream events - defined before subscription so it's in scope
  const handleStreamEvent = useCallback((event: StreamEvent) => {
    // Deduplicate events by sequence number (prevents double delivery from
    // dual-path routing or reconnection races)
    const seq = event.seq as number | undefined;
    if (seq !== undefined && seq > 0) {
      if (seq <= lastSeqRef.current) {
        return; // Skip duplicate or out-of-order event
      }
      lastSeqRef.current = seq;
    }

    switch (event.type) {
      case "invoke_accepted":
        // Learn session_id early (before AgentStarted) to reduce subscription transition window
        if (event.session_id && typeof event.session_id === "string") {
          setSessionId(event.session_id);
          setActiveSessionId(event.session_id);
        }
        // Reset seq tracking for new session
        lastSeqRef.current = 0;
        break;

      case "agent_started":
        setIsProcessing(true);
        // Reset per-execution state
        toolCallCountRef.current = 0;
        toolActivityIdRef.current = null;
        respondDisplayedRef.current = false;
        // Capture session_id from the backend for session continuity AND subscription
        if (event.session_id && typeof event.session_id === "string") {
          setSessionId(event.session_id);
          // Also update state to trigger session-based subscription for subagent events
          setActiveSessionId(event.session_id);
        }
        break;

      case "token":
        // Buffer tokens and flush on next animation frame to prevent
        // garbled text from rapid state updates or duplicate events
        streamingBufferRef.current += event.delta as string;
        if (rafIdRef.current === null) {
          rafIdRef.current = requestAnimationFrame(flushTokenBuffer);
        }
        break;

      case "tool_call": {
        toolCallCountRef.current += 1;
        const toolName = event.tool as string;
        const count = toolCallCountRef.current;

        if (!toolActivityIdRef.current) {
          // First tool call — create the rolling message
          const id = crypto.randomUUID();
          toolActivityIdRef.current = id;
          setMessages((prev) => [
            ...prev,
            {
              id,
              role: "tool",
              content: `Calling ${toolName}...`,
              timestamp: new Date(),
              toolName,
            },
          ]);
        } else {
          // Subsequent calls — update existing message in-place
          const activityId = toolActivityIdRef.current;
          setMessages((prev) => {
            const idx = prev.findIndex((m) => m.id === activityId);
            if (idx >= 0) {
              const updated = [...prev];
              updated[idx] = {
                ...updated[idx],
                content: `Calling ${toolName}... (${count} tool calls)`,
                toolName,
              };
              return updated;
            }
            return prev;
          });
        }
        break;
      }

      case "tool_result": {
        const activityId = toolActivityIdRef.current;
        if (activityId) {
          const result = event.result as string;
          const count = toolCallCountRef.current;
          setMessages((prev) => {
            const idx = prev.findIndex((m) => m.id === activityId);
            if (idx >= 0) {
              const updated = [...prev];
              const toolName = updated[idx].toolName || "tool";
              const suffix = count > 1 ? ` (${count} tools)` : "";
              updated[idx] = {
                ...updated[idx],
                content: `${toolName}: ${result.substring(0, 200)}${result.length > 200 ? "..." : ""}${suffix}`,
              };
              return updated;
            }
            return prev;
          });
        }
        break;
      }

      case "show_content":
        // Show content in generative canvas
        setCanvasContent({
          type: "show_content",
          event: event as unknown as ShowContentEvent,
        });
        setCanvasOpen(true);
        break;

      case "request_input":
        // Show form in generative canvas
        const inputEvent = event as unknown as RequestInputEvent;
        setCanvasContent({
          type: "request_input",
          event: inputEvent,
        });
        setPendingFormId(inputEvent.formId);
        setCanvasOpen(true);
        break;

      case "delegation_started": {
        const childAgentId = event.child_agent_id as string;
        // Use child_conversation_id if available, otherwise fall back to child_execution_id
        const childConvId = (event.child_conversation_id ?? event.child_execution_id) as string;
        const task = event.task as string;

        if (!childConvId) break; // Skip if no identifier available

        // Track the active delegation with full activity data
        setSubagentActivities((prev) => {
          const updated = new Map(prev);
          updated.set(childConvId, {
            childAgentId,
            childConversationId: childConvId,
            task,
            startedAt: new Date(),
            status: "running",
            tokens: 0,
            toolCalls: [],
          });
          return updated;
        });

        // Add delegation message to chat
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: "delegation",
            content: `Delegating to ${childAgentId}: "${task.substring(0, 100)}${task.length > 100 ? "..." : ""}"`,
            timestamp: new Date(),
            delegationStatus: "started",
            childAgentId,
          },
        ]);
        break;
      }

      case "delegation_completed": {
        const childConvId = (event.child_conversation_id ?? event.child_execution_id) as string;
        const childAgentId = event.child_agent_id as string;
        const result = event.result as string | undefined;

        if (!childConvId) break;

        // Update subagent activity to completed status
        setSubagentActivities((prev) => {
          const updated = new Map(prev);
          const activity = updated.get(childConvId);
          if (activity) {
            updated.set(childConvId, {
              ...activity,
              status: "completed",
              completedAt: new Date(),
              result: result,
            });
          }
          return updated;
        });

        // Update delegation message or add completion message
        setMessages((prev) => {
          // Find the corresponding started message
          const startedIndex = prev.findIndex(
            (m) => m.role === "delegation" && m.childAgentId === childAgentId && m.delegationStatus === "started"
          );

          if (startedIndex >= 0) {
            const updated = [...prev];
            updated[startedIndex] = {
              ...updated[startedIndex],
              delegationStatus: "completed",
              content: `${childAgentId} completed: ${result?.substring(0, 150) || "(no result)"}${(result?.length || 0) > 150 ? "..." : ""}`,
            };
            return updated;
          }

          // If no started message found, add a completion message
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              role: "delegation",
              content: `${childAgentId} completed: ${result?.substring(0, 150) || "(no result)"}`,
              timestamp: new Date(),
              delegationStatus: "completed",
              childAgentId,
            },
          ];
        });
        break;
      }

      case "delegation_error": {
        const childConvId = event.child_conversation_id as string;
        const childAgentId = event.child_agent_id as string;
        const error = event.error as string | undefined;

        // Update subagent activity to error status
        setSubagentActivities((prev) => {
          const updated = new Map(prev);
          const activity = updated.get(childConvId);
          if (activity) {
            updated.set(childConvId, {
              ...activity,
              status: "error",
              completedAt: new Date(),
              error: error,
            });
          }
          return updated;
        });

        // Update delegation message
        setMessages((prev) => {
          const startedIndex = prev.findIndex(
            (m) => m.role === "delegation" && m.childAgentId === childAgentId && m.delegationStatus === "started"
          );

          if (startedIndex >= 0) {
            const updated = [...prev];
            updated[startedIndex] = {
              ...updated[startedIndex],
              delegationStatus: "completed",
              content: `${childAgentId} failed: ${error || "Unknown error"}`,
            };
            return updated;
          }
          return prev;
        });
        break;
      }

      case "message_added": {
        // A message was added to the conversation (e.g., delegation callback)
        // Add it directly to the messages array
        const role = event.role as string;
        const content = event.content as string;

        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: role as "user" | "assistant" | "tool" | "delegation" | "system",
            content,
            timestamp: new Date(),
            isStreaming: false,
          },
        ]);
        break;
      }

      case "turn_complete": {
        // Respond tool output arrives as turn_complete with final_message field
        // (GatewayEvent::Respond → ServerMessage::TurnComplete { final_message })
        if (rafIdRef.current !== null) {
          cancelAnimationFrame(rafIdRef.current);
          rafIdRef.current = null;
        }
        flushTokenBuffer();

        const finalMessage = event.final_message as string | undefined;
        if (finalMessage && !respondDisplayedRef.current) {
          respondDisplayedRef.current = true;
          // Respond tool — strip tool noise, replace streaming duplicate or append
          setMessages((prev) => {
            const cleaned = prev.filter((m) => m.role !== "tool");
            const lastIdx = cleaned.length - 1;
            const last = cleaned[lastIdx];
            if (last && last.role === "assistant" && last.isStreaming) {
              return [
                ...cleaned.slice(0, lastIdx),
                { ...last, content: finalMessage, isStreaming: false },
              ];
            }
            return [
              ...cleaned.map((m) => (m.isStreaming ? { ...m, isStreaming: false } : m)),
              {
                id: crypto.randomUUID(),
                role: "assistant",
                content: finalMessage,
                timestamp: new Date(),
                isStreaming: false,
              },
            ];
          });
          toolActivityIdRef.current = null;
          setIsProcessing(false);
        }
        // turn_complete without final_message is a mid-execution turn boundary — ignore it.
        // Cleanup happens on agent_completed.
        break;
      }

      case "agent_completed":
      case "error": {
        // Flush any remaining buffered tokens before finalizing
        if (rafIdRef.current !== null) {
          cancelAnimationFrame(rafIdRef.current);
          rafIdRef.current = null;
        }
        flushTokenBuffer();

        setIsProcessing(false);
        toolActivityIdRef.current = null;
        // Reload messages from DB — authoritative source for the respond tool output
        setReloadTrigger((c) => c + 1);
        break;
      }

      case "session_ended":
        break;
    }
  }, [flushTokenBuffer]);

  // Subscribe to events via server-side routing
  // Uses "session" scope for main chat view - filters subagent internal events server-side
  // while still showing delegation lifecycle markers (DelegationStarted/DelegationCompleted)
  //
  // SUBSCRIPTION STRATEGY:
  // - If we have an activeSessionId (sess-xxx), subscribe ONLY by session_id
  // - Otherwise, subscribe by conversationId (web-xxx) until session is established
  // This avoids duplicate events from dual-path routing (session_id + conversation_id)
  useEffect(() => {
    // Prefer session_id subscription when available
    const subscriptionKey = activeSessionId || conversationId;
    if (!subscriptionKey) return;

    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const setupSubscription = async () => {
      const transport = await getTransport();

      if (cancelled) return;

      // Use "session" scope to filter subagent internal events server-side
      // Server will only send: root execution events + delegation lifecycle markers
      unsubscribe = transport.subscribeConversation(subscriptionKey, {
        onEvent: handleStreamEvent,
        scope: "session",
        onConfirmed: (seq, rootExecutionIds) => {
          console.log(`[WebChatPanel] Subscription confirmed for ${subscriptionKey} at seq ${seq}, roots: ${rootExecutionIds?.length ?? 0}`);
        },
      });
    };

    setupSubscription();

    return () => {
      cancelled = true;
      if (unsubscribe) {
        unsubscribe();
      }
      // Cancel any pending token buffer flush
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }
      streamingBufferRef.current = "";
      lastSeqRef.current = 0;
    };
  }, [activeSessionId, conversationId, handleStreamEvent]);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // End the current session and optionally start a new one
  const handleEndSession = async (startNew: boolean) => {
    const currentSessionId = getSessionId();
    if (currentSessionId) {
      try {
        const transport = await getTransport();
        await transport.endSession(currentSessionId);
      } catch (error) {
        console.error("Failed to end session:", error);
      }
      // Always clear the session_id after ending
      localStorage.removeItem(WEB_SESSION_ID_KEY);
      setActiveSessionId(null); // Clear session subscription
    }

    if (startNew) {
      const newConvId = createNewConversationId();
      setConversationId(newConvId);
      setMessages([]);
      setSubagentActivities(new Map());
    }
  };

  const handleSend = async () => {
    // Use ref as synchronous guard — React state (isProcessing) is batched
    // and won't update between rapid calls (e.g., Enter key repeat).
    // Without this, multiple Invoke messages can be sent, creating parallel
    // agent executions whose token streams interleave → garbled text.
    if (!input.trim() || isProcessing || isSubmittingRef.current) return;
    isSubmittingRef.current = true;

    const userMessage: ChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: input.trim(),
      timestamp: new Date(),
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput("");
    setIsProcessing(true);

    try {
      const transport = await getTransport();
      // Pass session_id to continue the same session (or undefined for new session)
      const currentSessionId = getSessionId() ?? undefined;
      await transport.executeAgent(ROOT_AGENT_ID, conversationId, userMessage.content, currentSessionId);
    } catch (error) {
      console.error("Failed to send message:", error);
      setIsProcessing(false);
    } finally {
      isSubmittingRef.current = false;
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey && !e.repeat) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleCanvasClose = () => {
    setCanvasOpen(false);
    setCanvasContent(null);
    setPendingFormId(null);
    // Focus back to input
    inputRef.current?.focus();
  };

  const handleFormSubmit = async (formId: string, data: Record<string, unknown>) => {
    console.log("[WebChatPanel] Form submitted:", { formId, data });

    // Send the form response back to the agent via a special message
    // The agent should be waiting for this input
    try {
      const transport = await getTransport();
      if (conversationId) {
        // Send as a structured response
        const responseMessage = JSON.stringify({
          type: "form_response",
          formId,
          data,
        });
        const currentSessionId = getSessionId() ?? undefined;
        await transport.executeAgent(ROOT_AGENT_ID, conversationId, responseMessage, currentSessionId);
      }
    } catch (error) {
      console.error("Failed to send form response:", error);
    }
  };

  const handleFormCancel = (formId: string) => {
    console.log("[WebChatPanel] Form cancelled:", formId);
    // Optionally notify the agent that the form was cancelled
  };

  return (
    <div className="flex flex-col h-full bg-[var(--background)]">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-[var(--border)] bg-[var(--card)]">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center">
            <MessageSquare className="w-5 h-5 text-white" />
          </div>
          <h1 className="text-lg font-semibold text-[var(--foreground)]">
            Chat
          </h1>
        </div>
        <div className="flex items-center gap-2">
          {(() => {
            const runningCount = Array.from(subagentActivities.values()).filter(a => a.status === "running").length;
            return runningCount > 0 && (
              <div className="flex items-center gap-2 text-violet-600 text-sm font-medium bg-violet-50 px-3 py-1.5 rounded-lg border border-violet-200">
                <GitBranch className="w-4 h-4" />
                {runningCount} subagent{runningCount > 1 ? "s" : ""} working
              </div>
            );
          })()}
          <ConnectionStatus />
          {isProcessing && (
            <div className="flex items-center gap-2 text-[var(--primary)] text-sm font-medium bg-[var(--accent)] px-3 py-1.5 rounded-lg">
              <Loader2 className="w-4 h-4 animate-spin" />
              Processing...
            </div>
          )}
          {/* Session controls */}
          {messages.length > 0 && !isProcessing && (
            <>
              <button
                onClick={() => handleEndSession(false)}
                className="flex items-center gap-1.5 text-sm text-[var(--muted-foreground)] hover:text-red-600 px-2 py-1.5 rounded-lg hover:bg-red-50 transition-colors"
                title="End current session"
              >
                <StopCircle className="w-4 h-4" />
                End
              </button>
              <button
                onClick={() => handleEndSession(true)}
                className="flex items-center gap-1.5 text-sm text-[var(--muted-foreground)] hover:text-[var(--primary)] px-2 py-1.5 rounded-lg hover:bg-[var(--accent)] transition-colors"
                title="Start new conversation"
              >
                <RotateCcw className="w-4 h-4" />
                New
              </button>
            </>
          )}
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-6">
            {isLoadingHistory ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <Loader2 className="w-8 h-8 text-[var(--primary)] animate-spin mx-auto mb-4" />
              <p className="text-[var(--muted-foreground)]">Loading conversation...</p>
            </div>
          </div>
        ) : messages.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="w-20 h-20 rounded-2xl bg-[var(--muted)] flex items-center justify-center mx-auto mb-4">
                <MessageSquare className="w-10 h-10 text-[var(--muted-foreground)]" />
              </div>
              <h2 className="text-lg font-semibold text-[var(--foreground)] mb-2">No messages yet</h2>
              <p className="text-[var(--muted-foreground)]">Start a conversation with your agent</p>
              <p className="text-xs text-[var(--muted-foreground)] mt-2">Type <code className="bg-[var(--muted)] px-1.5 py-0.5 rounded">/new</code> to start a fresh session</p>
            </div>
          </div>
        ) : (
          <div className="max-w-3xl mx-auto space-y-4">
            {messages.filter((m) => isProcessing || m.role !== "tool").map((message) => (
              <div
                key={message.id}
                className={`flex gap-3 ${message.role === "user" ? "flex-row-reverse" : ""}`}
              >
                {/* Avatar */}
                <div className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${
                  message.role === "user"
                    ? "bg-[var(--primary)]"
                    : message.role === "tool"
                      ? "bg-amber-100"
                      : message.role === "delegation"
                        ? message.delegationStatus === "completed"
                          ? "bg-emerald-100"
                          : "bg-violet-100"
                        : message.role === "system"
                          ? "bg-blue-100"
                          : "bg-gradient-to-br from-indigo-500 to-purple-600"
                }`}>
                  {message.role === "user" ? (
                    <User className="w-4 h-4 text-white" />
                  ) : message.role === "tool" ? (
                    <Wrench className="w-4 h-4 text-amber-600" />
                  ) : message.role === "delegation" ? (
                    message.delegationStatus === "completed" ? (
                      <CheckCircle2 className="w-4 h-4 text-emerald-600" />
                    ) : (
                      <GitBranch className="w-4 h-4 text-violet-600" />
                    )
                  ) : message.role === "system" ? (
                    <Info className="w-4 h-4 text-blue-600" />
                  ) : (
                    <Bot className="w-4 h-4 text-white" />
                  )}
                </div>

                {/* Message */}
                <div
                  className={`max-w-[75%] rounded-2xl px-4 py-3 ${
                    message.role === "user"
                      ? "bg-[var(--primary)] text-white"
                      : message.role === "tool"
                        ? "bg-amber-50 border border-amber-200 text-amber-900"
                        : message.role === "delegation"
                          ? message.delegationStatus === "completed"
                            ? "bg-emerald-50 border border-emerald-200 text-emerald-900"
                            : "bg-violet-50 border border-violet-200 text-violet-900"
                          : message.role === "system"
                            ? "bg-blue-50 border border-blue-200 text-blue-900"
                            : "bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)]"
                  }`}
                >
                  {message.role === "tool" && (
                    <div className="text-xs font-medium text-amber-600 mb-1 flex items-center gap-1">
                      <Wrench className="w-3 h-3" />
                      {message.toolName}
                    </div>
                  )}
                  {message.role === "system" && (
                    <div className="text-xs font-medium text-blue-600 mb-1 flex items-center gap-1">
                      <Info className="w-3 h-3" />
                      System
                    </div>
                  )}
                  {message.role === "delegation" && (
                    <div className={`text-xs font-medium mb-1 flex items-center gap-1 ${
                      message.delegationStatus === "completed" ? "text-emerald-600" : "text-violet-600"
                    }`}>
                      {message.delegationStatus === "completed" ? (
                        <>
                          <CheckCircle2 className="w-3 h-3" />
                          Subagent Completed
                        </>
                      ) : (
                        <>
                          <GitBranch className="w-3 h-3" />
                          Delegating to Subagent
                          <Loader2 className="w-3 h-3 animate-spin ml-1" />
                        </>
                      )}
                    </div>
                  )}
                  {/* Use TruncatedContent for long messages, regular markdown for streaming/short */}
                  {message.isStreaming || message.role === "user" || message.role === "tool" ? (
                    <div className="prose prose-sm dark:prose-invert max-w-none text-sm prose-headings:mt-3 prose-headings:mb-2 prose-p:my-1 prose-pre:bg-[var(--muted)] prose-pre:border prose-pre:border-[var(--border)] prose-code:text-[var(--primary)] prose-code:bg-[var(--muted)] prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-code:before:content-none prose-code:after:content-none">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {message.content}
                      </ReactMarkdown>
                    </div>
                  ) : (
                    <TruncatedContent
                      id={message.id}
                      content={message.content}
                      maxWords={400}
                      className="text-sm"
                    />
                  )}
                  {message.isStreaming && (
                    <span className="inline-block w-2 h-4 bg-[var(--primary)] animate-pulse ml-1 rounded-sm" />
                  )}
                </div>
              </div>
            ))}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* Subagent Activity Panel */}
      <SubagentActivityPanel
        activities={subagentActivities}
        onClose={(conversationId) => {
          setSubagentActivities((prev) => {
            const updated = new Map(prev);
            updated.delete(conversationId);
            return updated;
          });
        }}
      />

      {/* Input */}
      <div className="p-4 border-t border-[var(--border)] bg-[var(--card)]">
        <div className="max-w-3xl mx-auto">
          <div className="flex gap-3">
            <textarea
              ref={inputRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a message..."
              disabled={isProcessing || canvasOpen}
              className="flex-1 bg-[var(--muted)] border border-[var(--border)] rounded-xl px-4 py-3 resize-none focus:outline-none focus:ring-2 focus:ring-[var(--primary)] disabled:opacity-50 text-[var(--foreground)] placeholder:text-[var(--muted-foreground)]"
              rows={1}
            />
            <button
              onClick={handleSend}
              disabled={!input.trim() || isProcessing || canvasOpen}
              className="bg-[var(--primary)] hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed text-white px-5 py-3 rounded-xl transition-all flex items-center gap-2 font-medium"
            >
              <Send className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>

      {/* Generative Canvas */}
      <GenerativeCanvas
        isOpen={canvasOpen}
        content={canvasContent}
        onClose={handleCanvasClose}
        onFormSubmit={handleFormSubmit}
        onFormCancel={handleFormCancel}
      />
    </div>
  );
}
