export interface UserMessageProps {
  /** Message text content */
  content: string;
  /** ISO timestamp */
  timestamp: string;
  /** Optional file attachment names */
  attachments?: string[];
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
 * User message block — avatar (U), timestamp, text, optional attachment chips.
 */
export function UserMessage({ content, timestamp, attachments }: UserMessageProps) {
  return (
    <div className="msg-block">
      <div className="msg-block__avatar msg-block__avatar--user">U</div>
      <div>
        <div className="msg-block__time">{formatTime(timestamp)}</div>
        <div className="msg-block__content">{content}</div>
        {attachments && attachments.length > 0 && (
          <div className="msg-block__attachments">
            {attachments.map((name) => (
              <span key={name} className="msg-block__attachment">
                {name}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
