// ============================================================================
// FAST CHAT PAGE
// Minimal chat UI for fast mode — no intent analysis, no intelligence feed.
// ============================================================================

import { useRef, useEffect } from "react";
import {
  Square, Brain, Users, Loader2, CheckCircle2,
  FileText, FileCode, Table, Globe, Image, Film, Music,
  File, Presentation,
} from "lucide-react";
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
// Artifact icon helper (mirrors ArtifactsPanel)
// ============================================================================

function getArtifactIcon(fileType?: string) {
  const size = 12;
  switch (fileType) {
    case "md": case "txt": case "docx": return <FileText size={size} />;
    case "rs": case "py": case "js": case "ts": case "tsx": case "jsx": return <FileCode size={size} />;
    case "csv": case "json": case "xlsx": return <Table size={size} />;
    case "html": case "htm": return <Globe size={size} />;
    case "png": case "jpg": case "jpeg": case "gif": case "svg": return <Image size={size} />;
    case "mp4": case "webm": return <Film size={size} />;
    case "mp3": case "wav": return <Music size={size} />;
    case "pptx": return <Presentation size={size} />;
    case "pdf": return <FileText size={size} />;
    default: return <File size={size} />;
  }
}

// ============================================================================
// Component
// ============================================================================

export function FastChat() {
  const { state, artifacts, sendMessage, stopAgent, showThinking, setShowThinking, initializing } = useFastChat();
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
        {isRunning && (
          <div className="fast-chat__typing">
            <div className="thinking-indicator__dots">
              <div className="thinking-indicator__dot" />
              <div className="thinking-indicator__dot" />
              <div className="thinking-indicator__dot" />
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Artifact pills */}
      {artifacts.length > 0 && (
        <div className="fast-chat__artifacts">
          {artifacts.map((art) => (
            <span key={art.id} className="fast-chat__artifact-pill" title={art.filePath}>
              {getArtifactIcon(art.fileType)} {art.label || art.fileName}
            </span>
          ))}
        </div>
      )}

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

  if (message.role === "delegation") {
    return (
      <div className="fast-chat__msg fast-chat__msg--delegation">
        <div className="fast-chat__delegation">
          <div className="fast-chat__delegation-header">
            <Users size={14} />
            <span className="fast-chat__delegation-agent">{message.delegationAgent}</span>
            <span className="fast-chat__delegation-status">
              {message.delegationStatus === "running" && <Loader2 size={12} className="animate-spin" />}
              {message.delegationStatus === "completed" && <CheckCircle2 size={12} style={{ color: "var(--success)" }} />}
            </span>
          </div>
          {message.delegationTask && (
            <div className="fast-chat__delegation-task">{message.delegationTask}</div>
          )}
          {message.delegationResult && (
            <details className="fast-chat__delegation-result">
              <summary>Result</summary>
              <div>{message.delegationResult}</div>
            </details>
          )}
        </div>
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
