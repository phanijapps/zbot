// ============================================================================
// FAST CHAT PAGE
// Minimal chat UI for fast mode — no intent analysis, no intelligence feed.
// ============================================================================

import { useRef, useEffect } from "react";
import { Square, Brain } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ChatInput } from "./ChatInput";
import { ThinkingBlock } from "./ThinkingBlock";
import { useFastChat, type FastMessage } from "./fast-chat-hooks";

// ============================================================================
// Prose classes (same as AgentResponse)
// ============================================================================

const PROSE_CLASSES =
  "prose prose-sm dark:prose-invert max-w-none text-sm " +
  "prose-headings:mt-3 prose-headings:mb-2 prose-p:my-1 " +
  "prose-pre:bg-[var(--muted)] prose-pre:border prose-pre:border-[var(--border)] " +
  "prose-code:text-[var(--foreground)] prose-code:bg-[var(--muted)] " +
  "prose-code:px-1 prose-code:py-0.5 prose-code:rounded " +
  "prose-code:before:content-none prose-code:after:content-none";

// ============================================================================
// Component
// ============================================================================

export function FastChat() {
  const { state, sendMessage, stopAgent, showThinking, setShowThinking, initializing } = useFastChat();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const isRunning = state.status === "running";

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [state.messages]);

  const handleSend = (text: string) => {
    sendMessage(text);
  };

  if (initializing) {
    return (
      <div className="fast-chat">
        <div className="fast-chat__empty">
          <span className="loading-spinner" />
        </div>
      </div>
    );
  }

  return (
    <div className="fast-chat">
      {/* Header */}
      <div className="fast-chat__header">
        <span className="fast-chat__title">Chat</span>
        <div className="fast-chat__actions">
          <button
            className={`btn btn--ghost btn--sm ${showThinking ? "btn--active" : ""}`}
            onClick={() => setShowThinking(!showThinking)}
            title={showThinking ? "Hide thinking" : "Show thinking"}
          >
            <Brain size={14} />
          </button>
          {isRunning && (
            <button
              className="btn btn--ghost btn--sm"
              onClick={stopAgent}
              title="Stop"
            >
              <Square style={{ width: 14, height: 14 }} />
            </button>
          )}
        </div>
      </div>

      {/* Messages */}
      <div className="fast-chat__messages">
        {state.messages.length === 0 && (
          <div className="fast-chat__empty">
            Send a message to start chatting
          </div>
        )}
        {state.messages.map((msg) => (
          <MessageBubble key={msg.id} message={msg} showThinking={showThinking} />
        ))}
        {isRunning && state.messages.length > 0 && (
          <div className="fast-chat__typing">
            <span className="loading-dots" />
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="fast-chat__input">
        <ChatInput onSend={handleSend} disabled={isRunning} />
      </div>
    </div>
  );
}

// ============================================================================
// Message Bubble
// ============================================================================

function MessageBubble({ message, showThinking }: { message: FastMessage; showThinking: boolean }) {
  if (message.role === "thinking") {
    if (!showThinking) return null;
    return (
      <div className="fast-chat__msg fast-chat__msg--thinking">
        <ThinkingBlock content={message.content} />
      </div>
    );
  }

  if (message.role === "user") {
    return (
      <div className="fast-chat__msg fast-chat__msg--user">
        <div className="fast-chat__bubble fast-chat__bubble--user">
          {message.content}
        </div>
      </div>
    );
  }

  if (message.role === "tool") {
    return (
      <div className="fast-chat__msg fast-chat__msg--tool">
        <div className="fast-chat__tool">
          <div className="fast-chat__tool-name">
            {message.toolName ?? "tool"}
          </div>
          {message.toolOutput && (
            <div className="fast-chat__tool-output">
              {message.toolOutput.length > 500
                ? message.toolOutput.slice(0, 500) + "..."
                : message.toolOutput}
            </div>
          )}
        </div>
      </div>
    );
  }

  // assistant
  return (
    <div className="fast-chat__msg fast-chat__msg--assistant">
      <div className="fast-chat__bubble fast-chat__bubble--agent">
        <div className={PROSE_CLASSES}>
          <ReactMarkdown remarkPlugins={[remarkGfm]}>
            {message.content}
          </ReactMarkdown>
        </div>
      </div>
    </div>
  );
}
