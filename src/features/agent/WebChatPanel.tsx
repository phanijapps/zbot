// ============================================================================
// WEB CHAT PANEL
// Chat interface for the web dashboard (uses transport layer instead of Tauri)
// ============================================================================

import { useState, useEffect, useRef } from "react";
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
  const [pendingFormId, setPendingFormId] = useState<string | null>(null);

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
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-800">
        <h1 className="text-lg font-semibold">Chat</h1>
        {isProcessing && (
          <div className="flex items-center gap-2 text-violet-400 text-sm">
            <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-violet-500" />
            Processing...
          </div>
        )}
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-500">
            <div className="text-center">
              <p className="text-lg mb-2">No messages yet</p>
              <p className="text-sm">Start a conversation</p>
            </div>
          </div>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={`flex ${message.role === "user" ? "justify-end" : "justify-start"}`}
            >
              <div
                className={`max-w-[80%] rounded-lg px-4 py-2 ${
                  message.role === "user"
                    ? "bg-violet-600 text-white"
                    : message.role === "tool"
                      ? "bg-yellow-900/30 text-yellow-200 border border-yellow-800"
                      : "bg-gray-800 text-gray-100"
                }`}
              >
                {message.role === "tool" && (
                  <div className="text-xs text-yellow-500 mb-1 font-medium">{message.toolName}</div>
                )}
                <div className="whitespace-pre-wrap">{message.content}</div>
                {message.isStreaming && (
                  <span className="inline-block w-2 h-4 bg-violet-500 animate-pulse ml-1" />
                )}
              </div>
            </div>
          ))
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="p-4 border-t border-gray-800">
        <div className="flex gap-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message..."
            disabled={isProcessing || canvasOpen}
            className="flex-1 bg-gray-800 border border-gray-700 rounded-lg px-4 py-2 resize-none focus:outline-none focus:border-violet-500 disabled:opacity-50"
            rows={1}
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || isProcessing || canvasOpen}
            className="bg-violet-600 hover:bg-violet-700 disabled:opacity-50 disabled:cursor-not-allowed text-white px-4 py-2 rounded-lg transition-colors"
          >
            Send
          </button>
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
