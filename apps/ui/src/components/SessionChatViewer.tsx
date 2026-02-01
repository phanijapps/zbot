// ============================================================================
// SESSION CHAT VIEWER
// Read-only or interactive view of a session's conversation
// Used in the chat slider for viewing session history
// ============================================================================

import { useState, useEffect, useRef } from "react";
import {
  MessageSquare,
  Send,
  Loader2,
  Wrench,
  User,
  Bot,
  GitBranch,
  CheckCircle2,
  Info,
  Eye,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { getTransport, type MessageResponse, type SessionMessage, type MessageScope } from "@/services/transport";

// ============================================================================
// Types
// ============================================================================

interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "tool" | "delegation" | "system";
  content: string;
  timestamp: Date;
  toolName?: string;
  delegationStatus?: "started" | "completed";
  childAgentId?: string;
  /** Agent ID for session messages (to show which agent sent the message) */
  agentId?: string;
  /** Delegation type for session messages */
  delegationType?: string;
}

interface SessionChatViewerProps {
  /** Session ID for fetching messages (new API) */
  sessionId?: string;
  /** Execution ID for execution-scoped messages (optional) */
  executionId?: string;
  /** Legacy: Conversation ID (for backward compatibility, falls back to old API) */
  conversationId?: string;
  agentId: string;
  readOnly?: boolean;
  onClose?: () => void;
}

// ============================================================================
// Component
// ============================================================================

export function SessionChatViewer({
  sessionId,
  executionId,
  conversationId,
  agentId,
  readOnly = false,
}: SessionChatViewerProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(true);
  const [isProcessing, setIsProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Determine the message scope based on props
  const scope: MessageScope = executionId ? 'execution' : 'root';

  // Load conversation history
  useEffect(() => {
    const loadHistory = async () => {
      setIsLoading(true);
      setError(null);

      try {
        const transport = await getTransport();

        // Use new session messages API if sessionId is provided
        if (sessionId) {
          const result = await transport.getSessionMessages(sessionId, {
            scope,
            execution_id: executionId,
          });

          if (result.success && result.data) {
            const loadedMessages: ChatMessage[] = result.data.map((m: SessionMessage) => ({
              id: m.id,
              role: m.role as ChatMessage["role"],
              content: m.content,
              timestamp: new Date(m.created_at),
              agentId: m.agent_id,
              delegationType: m.delegation_type,
            }));
            setMessages(loadedMessages);
          } else {
            setError(result.error || "Failed to load messages");
          }
        } else if (conversationId) {
          // Fall back to legacy API for backward compatibility
          const result = await transport.getMessages(conversationId);

          if (result.success && result.data) {
            const loadedMessages: ChatMessage[] = result.data.map((m: MessageResponse) => ({
              id: m.id,
              role: m.role as ChatMessage["role"],
              content: m.content,
              timestamp: new Date(m.timestamp),
            }));
            setMessages(loadedMessages);
          } else {
            setError(result.error || "Failed to load messages");
          }
        } else {
          setError("No session or conversation ID provided");
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setIsLoading(false);
      }
    };

    loadHistory();
  }, [sessionId, executionId, conversationId, scope]);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSend = async () => {
    if (!input.trim() || isProcessing || readOnly) return;

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
      // Use sessionId if available, otherwise fall back to conversationId
      const convId = conversationId || sessionId || "";
      const sessId = sessionId || conversationId;
      await transport.executeAgent(agentId, convId, userMessage.content, sessId);
    } catch (err) {
      console.error("Failed to send message:", err);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex flex-col h-full bg-background">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border bg-card">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center">
            <MessageSquare className="w-5 h-5 text-white" />
          </div>
          <div>
            <h1 className="text-lg font-semibold">{agentId}</h1>
            <p className="text-xs text-muted-foreground font-mono">
              {executionId ? (
                <span title={`Session: ${sessionId}`}>
                  {executionId.slice(0, 20)}...
                </span>
              ) : (
                sessionId || conversationId
              )}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {executionId && (
            <div className="flex items-center gap-1 text-xs text-violet-600 bg-violet-100 px-2 py-1 rounded-md">
              <GitBranch className="w-3 h-3" />
              Subagent View
            </div>
          )}
          {readOnly && (
            <div className="flex items-center gap-2 text-muted-foreground text-sm bg-muted px-3 py-1.5 rounded-lg">
              <Eye className="w-4 h-4" />
              Read-only
            </div>
          )}
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-6">
        {isLoading ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <Loader2 className="w-8 h-8 text-primary animate-spin mx-auto mb-4" />
              <p className="text-muted-foreground">Loading conversation...</p>
            </div>
          </div>
        ) : error ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center text-destructive">
              <p>{error}</p>
            </div>
          </div>
        ) : messages.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="w-20 h-20 rounded-2xl bg-muted flex items-center justify-center mx-auto mb-4">
                <MessageSquare className="w-10 h-10 text-muted-foreground" />
              </div>
              <h2 className="text-lg font-semibold mb-2">No messages</h2>
              <p className="text-muted-foreground">This conversation is empty</p>
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
                <div
                  className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${
                    message.role === "user"
                      ? "bg-primary"
                      : message.role === "tool"
                        ? "bg-amber-100"
                        : message.role === "delegation"
                          ? message.delegationStatus === "completed"
                            ? "bg-emerald-100"
                            : "bg-violet-100"
                          : message.role === "system"
                            ? "bg-blue-100"
                            : "bg-gradient-to-br from-indigo-500 to-purple-600"
                  }`}
                >
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
                      ? "bg-primary text-primary-foreground"
                      : message.role === "tool"
                        ? "bg-amber-50 border border-amber-200 text-amber-900"
                        : message.role === "delegation"
                          ? message.delegationStatus === "completed"
                            ? "bg-emerald-50 border border-emerald-200 text-emerald-900"
                            : "bg-violet-50 border border-violet-200 text-violet-900"
                          : message.role === "system"
                            ? "bg-blue-50 border border-blue-200 text-blue-900"
                            : "bg-card border border-border"
                  }`}
                >
                  {message.role === "tool" && message.toolName && (
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
                    <div
                      className={`text-xs font-medium mb-1 flex items-center gap-1 ${
                        message.delegationStatus === "completed" ? "text-emerald-600" : "text-violet-600"
                      }`}
                    >
                      {message.delegationStatus === "completed" ? (
                        <>
                          <CheckCircle2 className="w-3 h-3" />
                          Subagent Completed
                        </>
                      ) : (
                        <>
                          <GitBranch className="w-3 h-3" />
                          Delegating to Subagent
                        </>
                      )}
                    </div>
                  )}
                  <div className="prose prose-sm dark:prose-invert max-w-none text-sm">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.content}</ReactMarkdown>
                  </div>
                </div>
              </div>
            ))}
            <div ref={messagesEndRef} />
          </div>
        )}
      </div>

      {/* Input (hidden in read-only mode) */}
      {!readOnly && (
        <div className="p-4 border-t border-border bg-card">
          <div className="max-w-3xl mx-auto">
            <div className="flex gap-3">
              <textarea
                ref={inputRef}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Type a message..."
                disabled={isProcessing}
                className="flex-1 bg-muted border border-border rounded-xl px-4 py-3 resize-none focus:outline-none focus:ring-2 focus:ring-primary disabled:opacity-50"
                rows={1}
              />
              <button
                onClick={handleSend}
                disabled={!input.trim() || isProcessing}
                className="bg-primary hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed text-primary-foreground px-5 py-3 rounded-xl transition-all flex items-center gap-2 font-medium"
              >
                <Send className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default SessionChatViewer;
