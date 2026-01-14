// ============================================================================
// CONVERSATIONS FEATURE
// Chat interface with agent
// ============================================================================

import { useState } from "react";
import { Send, Paperclip, Minimize2, Maximize2 } from "lucide-react";
import { Button } from "@/shared/ui/button";
import { Textarea } from "@/shared/ui/textarea";
import { cn } from "@/shared/utils";

interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: string;
}

export function ConversationsPanel() {
  const [input, setInput] = useState("");
  const [isExpanded, setIsExpanded] = useState(false);
  const [messages, setMessages] = useState<Message[]>([
    {
      id: "1",
      role: "assistant",
      content: "Hello! I'm your AI assistant. How can I help you today?",
      timestamp: new Date().toLocaleTimeString(),
    },
  ]);

  const handleSend = () => {
    if (input.trim()) {
      setMessages([
        ...messages,
        {
          id: Date.now().toString(),
          role: "user",
          content: input.trim(),
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
      setInput("");
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="h-14 border-b border-white/5 flex items-center justify-between px-6">
        <div className="flex items-center gap-3">
          <div className="size-2 rounded-full bg-green-500 animate-pulse" />
          <h2 className="text-white font-medium">AI Assistant</h2>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="text-gray-400 hover:text-white h-8 w-8 p-0"
          onClick={() => setIsExpanded(!isExpanded)}
        >
          {isExpanded ? (
            <Minimize2 className="size-4" />
          ) : (
            <Maximize2 className="size-4" />
          )}
        </Button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {messages.map((message) => (
          <div
            key={message.id}
            className={cn(
              "flex gap-4",
              message.role === "user" ? "flex-row-reverse" : ""
            )}
          >
            <div
              className={cn(
                "size-8 rounded-lg shrink-0 flex items-center justify-center text-sm font-medium",
                message.role === "user"
                  ? "bg-gradient-to-br from-blue-500 to-purple-600 text-white"
                  : "bg-gradient-to-br from-orange-500 to-pink-600 text-white"
              )}
            >
              {message.role === "user" ? "U" : "AI"}
            </div>
            <div
              className={cn(
                "flex-1 max-w-2xl",
                message.role === "user" ? "text-right" : ""
              )}
            >
              <div
                className={cn(
                  "inline-block rounded-2xl px-4 py-3",
                  message.role === "user"
                    ? "bg-blue-600 text-white"
                    : "bg-white/5 text-gray-100"
                )}
              >
                <p className="text-sm leading-relaxed whitespace-pre-wrap">
                  {message.content}
                </p>
              </div>
              <p className="text-xs text-gray-500 mt-1 px-1">
                {message.timestamp}
              </p>
            </div>
          </div>
        ))}
      </div>

      {/* Input Area */}
      <div className="border-t border-white/5 p-4">
        <div className="max-w-4xl mx-auto">
          <div className="relative bg-white/5 rounded-2xl border border-white/10 focus-within:border-blue-500/50 transition-colors">
            <Textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type your message... (Press Enter to send, Shift+Enter for new line)"
              className="min-h-[60px] max-h-[200px] bg-transparent border-0 text-white placeholder:text-gray-500 resize-none pr-24 focus-visible:ring-0"
            />
            <div className="absolute bottom-3 right-3 flex items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                className="text-gray-400 hover:text-white h-8 w-8 p-0"
              >
                <Paperclip className="size-4" />
              </Button>
              <Button
                onClick={handleSend}
                size="sm"
                className="bg-gradient-to-br from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white h-8 px-3"
                disabled={!input.trim()}
              >
                <Send className="size-4" />
              </Button>
            </div>
          </div>
          <p className="text-xs text-gray-500 mt-2 text-center">
            Press <kbd className="px-1.5 py-0.5 bg-white/5 rounded">Enter</kbd>{" "}
            to send,{" "}
            <kbd className="px-1.5 py-0.5 bg-white/5 rounded">
              Shift + Enter
            </kbd>{" "}
            for new line
          </p>
        </div>
      </div>
    </div>
  );
}
