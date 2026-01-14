/**
 * ConversationView - Chat interface with integrated ThinkingPanel
 *
 * Layout:
 * Desktop: Chat (70%) + ThinkingPanel (30%) side by side
 * Tablet: Chat with collapsible bottom panel
 * Mobile: Chat with modal for thinking details
 *
 * Features:
 * - Message display with streaming support
 * - Thinking tab with animated indicator
 * - Auto-open/collapse thinking panel based on agent activity
 * - Message input with send functionality
 */

import { useState, useCallback, useEffect, useRef } from "react";
import { Send, Paperclip, MoreVertical, ArrowLeft, RefreshCw } from "lucide-react";
import { cn } from "@/shared/utils";
import { Button } from "@/shared/ui/button";
import { Textarea } from "@/shared/ui/textarea";
import { ThinkingTab } from "./ThinkingTab";
import { ThinkingPanel } from "./ThinkingPanel";
import { ThinkingPanelMobile } from "./ThinkingPanel";
import { useStreamEvents } from "./useStreamEvents";
import type { MessageWithThinking, ConversationWithAgent } from "./types";

interface ConversationViewProps {
  conversation: ConversationWithAgent | null;
  messages: MessageWithThinking[];
  onSendMessage: (content: string) => Promise<void>;
  onBack: () => void;
  onNewChat: () => void;
  isLoading?: boolean;
  className?: string;
}

export function ConversationView({
  conversation,
  messages,
  onSendMessage,
  onBack,
  onNewChat,
  isLoading = false,
  className,
}: ConversationViewProps) {
  const [input, setInput] = useState("");
  const [isSending, setIsSending] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Stream events handling
  const {
    state: thinkingState,
    togglePanel,
    openPanel,
    closePanel,
  } = useStreamEvents(true, true);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Handle sending message
  const handleSend = useCallback(async () => {
    if (!input.trim() || isSending) return;

    const messageContent = input.trim();
    setInput("");
    setIsSending(true);

    try {
      await onSendMessage(messageContent);
    } finally {
      setIsSending(false);
    }
  }, [input, isSending, onSendMessage]);

  // Handle keyboard
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend]
  );

  // Get tool count for current message
  const currentToolCount = messages[messages.length - 1]?.thinking?.toolCount || 0;

  return (
    <div className={cn("flex h-full", className)}>
      {/* Main Chat Area */}
      <div className="flex-1 flex flex-col min-w-0">
        {/* Header */}
        <ConversationHeader
          conversation={conversation}
          onBack={onBack}
          onNewChat={onNewChat}
          thinkingState={thinkingState}
          onThinkingClick={togglePanel}
          toolCount={currentToolCount}
        />

        {/* Messages Area */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {messages.length === 0 ? (
            <EmptyChatState agentName={conversation?.agentName} />
          ) : (
            <div className="max-w-3xl mx-auto space-y-6">
              {messages.map((message) => (
                <MessageBubble
                  key={message.id}
                  message={message}
                  onShowThinking={openPanel}
                />
              ))}
              {isLoading && <TypingIndicator />}
              <div ref={messagesEndRef} />
            </div>
          )}
        </div>

        {/* Input Area */}
        <div className="border-t border-white/10 p-4">
          <div className="max-w-3xl mx-auto">
            <div className="relative bg-white/5 rounded-2xl border border-white/10 focus-within:border-purple-500/50 transition-colors">
              <Textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Type your message... (Press Enter to send, Shift+Enter for new line)"
                disabled={isSending}
                className="min-h-[60px] max-h-[200px] bg-transparent border-0 text-white placeholder:text-gray-500 resize-none pr-24 focus-visible:ring-0"
              />
              <div className="absolute bottom-3 right-3 flex items-center gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-gray-400 hover:text-white h-8 w-8 p-0"
                  disabled
                >
                  <Paperclip className="size-4" />
                </Button>
                <Button
                  onClick={handleSend}
                  size="sm"
                  disabled={!input.trim() || isSending}
                  className="bg-gradient-to-br from-purple-600 to-pink-600 hover:from-purple-700 hover:to-pink-700 text-white h-8 px-3"
                >
                  {isSending ? (
                    <RefreshCw className="size-4 animate-spin" />
                  ) : (
                    <Send className="size-4" />
                  )}
                </Button>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Thinking Panel - Desktop */}
      <div className="hidden lg:block">
        <ThinkingPanel
          isOpen={thinkingState.isOpen}
          onClose={closePanel}
          state={thinkingState}
        />
      </div>

      {/* Thinking Panel - Mobile (Modal) */}
      <div className="lg:hidden">
        <ThinkingPanelMobile
          isOpen={thinkingState.isOpen}
          onClose={closePanel}
          state={thinkingState}
        />
      </div>
    </div>
  );
}

// ============================================================================
// HEADER COMPONENT
// ============================================================================

interface ConversationHeaderProps {
  conversation: ConversationWithAgent | null;
  onBack: () => void;
  onNewChat: () => void;
  thinkingState: {
    isOpen: boolean;
    isActive: boolean;
  };
  onThinkingClick: () => void;
  toolCount: number;
}

function ConversationHeader({
  conversation,
  onBack,
  onNewChat,
  thinkingState,
  onThinkingClick,
  toolCount,
}: ConversationHeaderProps) {
  return (
    <div className="h-14 border-b border-white/10 flex items-center justify-between px-4 shrink-0">
      {/* Left: Back + Agent Info */}
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="sm"
          onClick={onBack}
          className="text-gray-400 hover:text-white h-8 w-8 p-0 lg:hidden"
        >
          <ArrowLeft className="size-4" />
        </Button>

        {conversation && (
          <div className="flex items-center gap-2.5">
            <span className="text-xl" role="img">
              {conversation.agentIcon || "🤖"}
            </span>
            <div className="hidden sm:block">
              <div className="text-sm font-medium text-white">
                {conversation.title}
              </div>
              <div className="text-xs text-gray-500">
                {conversation.agentName} • {conversation.model || "AI Agent"}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-2">
        {/* Thinking Tab */}
        {(thinkingState.isActive || toolCount > 0) && (
          <ThinkingTab
            isActive={thinkingState.isActive}
            toolCount={toolCount}
            onClick={onThinkingClick}
          />
        )}

        {/* New Chat Button */}
        <Button
          variant="ghost"
          size="sm"
          onClick={onNewChat}
          className="text-gray-400 hover:text-white h-8 w-8 p-0"
          title="New chat"
        >
          <RefreshCw className="size-4" />
        </Button>

        {/* More Options */}
        <Button
          variant="ghost"
          size="sm"
          className="text-gray-400 hover:text-white h-8 w-8 p-0"
        >
          <MoreVertical className="size-4" />
        </Button>
      </div>
    </div>
  );
}

// ============================================================================
// MESSAGE BUBBLE
// ============================================================================

interface MessageBubbleProps {
  message: MessageWithThinking;
  onShowThinking: () => void;
}

function MessageBubble({ message, onShowThinking }: MessageBubbleProps) {
  const isUser = message.role === "user";

  return (
    <div className={cn("flex gap-3", isUser ? "flex-row-reverse" : "")}>
      {/* Avatar */}
      <div
        className={cn(
          "size-8 rounded-lg shrink-0 flex items-center justify-center text-sm font-medium",
          isUser
            ? "bg-gradient-to-br from-blue-500 to-purple-600 text-white"
            : "bg-gradient-to-br from-orange-500 to-pink-600 text-white"
        )}
      >
        {isUser ? "U" : message.role === "system" ? "S" : "🤖"}
      </div>

      {/* Content */}
      <div className={cn("flex-1 max-w-2xl", isUser ? "text-right" : "")}>
        {/* Message Content */}
        <div
          className={cn(
            "inline-block rounded-2xl px-4 py-3",
            isUser
              ? "bg-blue-600 text-white"
              : "bg-white/5 text-gray-100"
          )}
        >
          <p className="text-sm leading-relaxed whitespace-pre-wrap">
            {message.content}
          </p>
        </div>

        {/* Thinking Indicator (for assistant messages) */}
        {!isUser && message.thinking && message.thinking.toolCount > 0 && (
          <button
            onClick={onShowThinking}
            className="mt-2 text-xs text-gray-500 hover:text-purple-400 transition-colors flex items-center gap-1.5"
          >
            <span>🧠</span>
            <span>Used {message.thinking.toolCount} tools</span>
          </button>
        )}

        {/* Timestamp */}
        <p className="text-xs text-gray-600 mt-1 px-1">
          {new Date(message.timestamp).toLocaleTimeString()}
        </p>
      </div>
    </div>
  );
}

// ============================================================================
// EMPTY STATE
// ============================================================================

function EmptyChatState({ agentName }: { agentName?: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-full text-center px-8">
      <div className="text-5xl mb-4">💬</div>
      <h3 className="text-lg font-medium text-white mb-2">
        Start a conversation with {agentName || "the agent"}
      </h3>
      <p className="text-sm text-gray-500 max-w-md">
        Send a message to begin. The agent will use its tools to help you with
        your task.
      </p>
    </div>
  );
}

// ============================================================================
// TYPING INDICATOR
// ============================================================================

function TypingIndicator() {
  return (
    <div className="flex items-center gap-3">
      <div className="size-8 rounded-lg bg-gradient-to-br from-orange-500 to-pink-600 flex items-center justify-center">
        <span className="text-sm">🤖</span>
      </div>
      <div className="bg-white/5 rounded-2xl px-4 py-3">
        <div className="flex gap-1">
          <span className="size-2 bg-gray-500 rounded-full animate-bounce [animation-delay:-0.3s]" />
          <span className="size-2 bg-gray-500 rounded-full animate-bounce [animation-delay:-0.15s]" />
          <span className="size-2 bg-gray-500 rounded-full animate-bounce" />
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// RESPONSIVE COMPONENTS
// ============================================================================

/**
 * ConversationView for tablet devices
 * Includes bottom collapsible thinking panel
 */
export function ConversationViewTablet(props: ConversationViewProps) {
  // TODO: Implement tablet-specific layout with bottom panel
  return <ConversationView {...props} />;
}

/**
 * ConversationView for mobile devices
 * Full-width chat with modal thinking panel
 */
export function ConversationViewMobile(props: ConversationViewProps) {
  // TODO: Implement mobile-specific optimizations
  return <ConversationView {...props} />;
}
