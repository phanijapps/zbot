import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ArtifactsPanel } from "./ArtifactsPanel";

export interface AgentResponseProps {
  /** Markdown content from the agent */
  content: string;
  /** ISO timestamp */
  timestamp: string;
  /** Session ID for loading artifacts */
  sessionId?: string;
}

/** Formats an ISO timestamp to a short time string (HH:MM) */
function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  } catch {
    return "";
  }
}

/**
 * Prose classes matching the existing WebChatPanel pattern.
 * Includes the inline code fix (foreground color, no quote marks).
 */
const PROSE_CLASSES =
  "prose prose-sm dark:prose-invert max-w-none text-sm " +
  "prose-headings:mt-3 prose-headings:mb-2 prose-p:my-1 " +
  "prose-pre:bg-[var(--muted)] prose-pre:border prose-pre:border-[var(--border)] " +
  "prose-code:text-[var(--foreground)] prose-code:bg-[var(--muted)] " +
  "prose-code:px-1 prose-code:py-0.5 prose-code:rounded " +
  "prose-code:before:content-none prose-code:after:content-none";

/**
 * Agent response block — avatar (Z, muted), timestamp, markdown-rendered text.
 */
export function AgentResponse({ content, timestamp, sessionId }: AgentResponseProps) {
  return (
    <div className="msg-block">
      <div className="msg-block__avatar msg-block__avatar--agent">Z</div>
      <div>
        <div className="msg-block__time">{formatTime(timestamp)}</div>
        <div className="msg-block__content">
          <div className={PROSE_CLASSES}>
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
          </div>
          {sessionId && <ArtifactsPanel sessionId={sessionId} />}
        </div>
      </div>
    </div>
  );
}
