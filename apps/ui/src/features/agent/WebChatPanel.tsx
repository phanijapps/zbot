// ============================================================================
// WEB CHAT PANEL
// Chat interface for the web dashboard (uses transport layer instead of Tauri)
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { MessageSquare, Send, Loader2, Wrench, User, Bot, GitBranch, CheckCircle2, Info } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { getTransport, type StreamEvent, type MessageResponse } from "@/services/transport";
import type { ShowContentEvent, RequestInputEvent } from "@/shared/types";
import { GenerativeCanvas, type ContentState } from "./GenerativeCanvas";
import { SubagentActivityPanel, type SubagentActivity } from "./SubagentActivityPanel";
import { TruncatedContent } from "./TruncatedContent";

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
  console.log("[SESSION_DEBUG] /new command - clearing session_id, new conversation_id:", convId);
  localStorage.removeItem(WEB_SESSION_ID_KEY);
  return convId;
}

// Get the current session ID (if any)
function getSessionId(): string | null {
  const sessionId = localStorage.getItem(WEB_SESSION_ID_KEY);
  console.log("[SESSION_DEBUG] getSessionId() =>", sessionId);
  return sessionId;
}

// Store the session ID from backend
function setSessionId(sessionId: string): void {
  console.log("[SESSION_DEBUG] setSessionId() storing:", sessionId);
  localStorage.setItem(WEB_SESSION_ID_KEY, sessionId);
}

export function WebChatPanel() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [conversationId, setConversationId] = useState<string>(() => getOrCreateConversationId());
  const [isLoadingHistory, setIsLoadingHistory] = useState(true);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [canvasContent, setCanvasContent] = useState<ContentState>(null);
  const [, setPendingFormId] = useState<string | null>(null);

  // Delegation tracking - uses SubagentActivity for detailed tracking
  const [subagentActivities, setSubagentActivities] = useState<Map<string, SubagentActivity>>(new Map());

  // Load conversation history on mount and when conversationId changes
  useEffect(() => {
    const loadHistory = async () => {
      if (!conversationId) return;

      setIsLoadingHistory(true);
      try {
        const transport = await getTransport();
        const result = await transport.getMessages(conversationId);

        if (result.success && result.data && result.data.length > 0) {
          const loadedMessages: ChatMessage[] = result.data.map((m: MessageResponse) => ({
            id: m.id,
            role: m.role as "user" | "assistant" | "tool" | "delegation",
            content: m.content,
            timestamp: new Date(m.timestamp),
            isStreaming: false,
          }));
          setMessages(loadedMessages);
        }
      } catch (error) {
        console.error("Failed to load conversation history:", error);
      } finally {
        setIsLoadingHistory(false);
      }
    };

    loadHistory();
  }, [conversationId]);

  // Subscribe to events when conversation changes
  useEffect(() => {
    if (!conversationId) return;

    let unsubscribe: (() => void) | null = null;

    const subscribe = async () => {
      const transport = await getTransport();
      unsubscribe = transport.subscribe(conversationId, handleStreamEvent);
    };

    subscribe();

    return () => {
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, [conversationId]);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleStreamEvent = (event: StreamEvent) => {
    switch (event.type) {
      case "agent_started":
        console.log("[SESSION_DEBUG] agent_started event received:", {
          session_id: event.session_id,
          agent_id: event.agent_id,
          conversation_id: event.conversation_id,
        });
        setIsProcessing(true);
        // Capture session_id from the backend for session continuity
        if (event.session_id && typeof event.session_id === "string") {
          setSessionId(event.session_id);
        } else {
          console.warn("[SESSION_DEBUG] agent_started event missing session_id!", event);
        }
        break;

      case "token":
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last && last.role === "assistant" && last.isStreaming) {
            return [
              ...prev.slice(0, -1),
              { ...last, content: last.content + (event.delta as string) },
            ];
          }
          return [
            ...prev,
            {
              id: crypto.randomUUID(),
              role: "assistant",
              content: event.delta as string,
              timestamp: new Date(),
              isStreaming: true,
            },
          ];
        });
        break;

      case "tool_call":
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: "tool",
            content: `Calling ${event.tool}...`,
            timestamp: new Date(),
            toolName: event.tool as string,
          },
        ]);
        break;

      case "tool_result":
        setMessages((prev) => {
          const toolCallIndex = prev.findIndex(
            (m) => m.role === "tool" && m.content.includes("...")
          );
          if (toolCallIndex >= 0) {
            const updated = [...prev];
            const result = event.result as string;
            updated[toolCallIndex] = {
              ...updated[toolCallIndex],
              content: `${updated[toolCallIndex].toolName}: ${result.substring(0, 200)}${result.length > 200 ? "..." : ""}`,
            };
            return updated;
          }
          return prev;
        });
        break;

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
        const childConvId = event.child_conversation_id as string;
        const task = event.task as string;

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
        const childConvId = event.child_conversation_id as string;
        const childAgentId = event.child_agent_id as string;
        const result = event.result as string | undefined;

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

      case "agent_completed":
      case "turn_complete":
      case "error":
        setIsProcessing(false);
        setMessages((prev) => {
          const last = prev[prev.length - 1];
          if (last && last.isStreaming) {
            return [...prev.slice(0, -1), { ...last, isStreaming: false }];
          }
          return prev;
        });
        break;
    }
  };

  const handleSend = async () => {
    if (!input.trim() || isProcessing) return;

    const trimmedInput = input.trim();

    // Handle /new command to start a new conversation
    if (trimmedInput === "/new") {
      const newConvId = createNewConversationId();
      setConversationId(newConvId);
      setMessages([]);
      setSubagentActivities(new Map());
      setInput("");
      return;
    }

    const userMessage: ChatMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: trimmedInput,
      timestamp: new Date(),
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput("");
    setIsProcessing(true);

    try {
      const transport = await getTransport();
      // Pass session_id to continue the same session (or undefined for new session)
      const currentSessionId = getSessionId() ?? undefined;
      console.log("[SESSION_DEBUG] Sending message with session_id:", currentSessionId, "conversation_id:", conversationId);
      await transport.executeAgent(ROOT_AGENT_ID, conversationId, userMessage.content, currentSessionId);
    } catch (error) {
      console.error("Failed to send message:", error);
      setIsProcessing(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
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
          <h1 className="text-lg font-semibold text-[var(--foreground)]">Chat</h1>
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
          {isProcessing && (
            <div className="flex items-center gap-2 text-[var(--primary)] text-sm font-medium bg-[var(--accent)] px-3 py-1.5 rounded-lg">
              <Loader2 className="w-4 h-4 animate-spin" />
              Processing...
            </div>
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
            {messages.map((message) => (
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
