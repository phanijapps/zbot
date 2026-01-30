// ============================================================================
// WEB CHAT PANEL
// Chat interface for the web dashboard (uses transport layer instead of Tauri)
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { MessageSquare, Send, Loader2, Wrench, User, Bot } from "lucide-react";
import { getTransport, type StreamEvent } from "@/services/transport";
import type { ShowContentEvent, RequestInputEvent } from "@/shared/types";
import { GenerativeCanvas, type ContentState } from "./GenerativeCanvas";

// ============================================================================
// Types
// ============================================================================

interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  timestamp: Date;
  toolName?: string;
  isStreaming?: boolean;
}

// ============================================================================
// Component
// ============================================================================

const ROOT_AGENT_ID = "root";

export function WebChatPanel() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [conversationId, setConversationId] = useState<string>("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Generative Canvas state
  const [canvasOpen, setCanvasOpen] = useState(false);
  const [canvasContent, setCanvasContent] = useState<ContentState>(null);
  const [, setPendingFormId] = useState<string | null>(null);

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
        setIsProcessing(true);
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
      const newConversationId = conversationId || crypto.randomUUID();
      setConversationId(newConversationId);

      await transport.executeAgent(ROOT_AGENT_ID, newConversationId, userMessage.content);
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
        await transport.executeAgent(ROOT_AGENT_ID, conversationId, responseMessage);
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
        {isProcessing && (
          <div className="flex items-center gap-2 text-[var(--primary)] text-sm font-medium bg-[var(--accent)] px-3 py-1.5 rounded-lg">
            <Loader2 className="w-4 h-4 animate-spin" />
            Processing...
          </div>
        )}
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-6">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <div className="text-center">
              <div className="w-20 h-20 rounded-2xl bg-[var(--muted)] flex items-center justify-center mx-auto mb-4">
                <MessageSquare className="w-10 h-10 text-[var(--muted-foreground)]" />
              </div>
              <h2 className="text-lg font-semibold text-[var(--foreground)] mb-2">No messages yet</h2>
              <p className="text-[var(--muted-foreground)]">Start a conversation with your agent</p>
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
                      : "bg-gradient-to-br from-indigo-500 to-purple-600"
                }`}>
                  {message.role === "user" ? (
                    <User className="w-4 h-4 text-white" />
                  ) : message.role === "tool" ? (
                    <Wrench className="w-4 h-4 text-amber-600" />
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
                        : "bg-[var(--card)] border border-[var(--border)] text-[var(--foreground)]"
                  }`}
                >
                  {message.role === "tool" && (
                    <div className="text-xs font-medium text-amber-600 mb-1 flex items-center gap-1">
                      <Wrench className="w-3 h-3" />
                      {message.toolName}
                    </div>
                  )}
                  <div className="whitespace-pre-wrap text-sm">{message.content}</div>
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
