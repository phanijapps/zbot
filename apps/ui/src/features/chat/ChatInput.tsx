import { useState, useRef, useCallback } from "react";
import { ArrowUp } from "lucide-react";

// ============================================================================
// Types
// ============================================================================

export interface UploadedFile {
  /** Server-assigned file ID */
  id: string;
  /** Original filename */
  name: string;
  /** MIME type */
  mimeType: string;
  /** File size in bytes */
  size: number;
}

export interface ChatInputProps {
  onSend: (message: string, attachments: UploadedFile[]) => void;
  disabled: boolean;
}

// ============================================================================
// Component
// ============================================================================

/**
 * ChatInput - textarea with Enter-to-send, Shift+Enter for newlines,
 * attachment buttons, and pending file chips.
 */
export function ChatInput({ onSend, disabled }: ChatInputProps) {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const canSend = text.trim().length > 0;
  const isDisabled = disabled;

  const handleSend = useCallback(() => {
    if (!canSend || isDisabled) return;
    onSend(text.trim(), []);
    setText("");
    textareaRef.current?.focus();
  }, [canSend, isDisabled, text, onSend]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.repeat) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div style={{ width: "100%" }}>
      <div className="chat-input__container">
        <textarea
          ref={textareaRef}
          className="chat-input__field"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          disabled={isDisabled}
          rows={2}
        />

        <div className="chat-input__actions">
          <button
            className="chat-input__send"
            onClick={handleSend}
            disabled={!canSend || isDisabled}
            title="Send message"
          >
            <ArrowUp style={{ width: 18, height: 18 }} />
          </button>
        </div>
      </div>
    </div>
  );
}
