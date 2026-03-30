import { useState, useRef, useCallback } from "react";

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
// File Upload Helper
// ============================================================================

async function uploadFile(file: File): Promise<UploadedFile> {
  const form = new FormData();
  form.append("file", file);
  const base = "http://localhost:18791";
  const res = await fetch(`${base}/api/upload`, { method: "POST", body: form });
  if (!res.ok) {
    throw new Error(`Upload failed: ${res.statusText}`);
  }
  return res.json();
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
  const [attachments, setAttachments] = useState<UploadedFile[]>([]);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const canSend = text.trim().length > 0 || attachments.length > 0;
  const isDisabled = disabled || uploading;

  const handleSend = useCallback(() => {
    if (!canSend || isDisabled) return;
    onSend(text.trim(), attachments);
    setText("");
    setAttachments([]);
    // Refocus textarea after send
    textareaRef.current?.focus();
  }, [canSend, isDisabled, text, attachments, onSend]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.repeat) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleFileSelect = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setUploading(true);
    try {
      const uploaded = await Promise.all(
        Array.from(files).map((f) => uploadFile(f)),
      );
      setAttachments((prev) => [...prev, ...uploaded]);
    } catch (err) {
      console.error("File upload failed:", err);
    } finally {
      setUploading(false);
    }
  };

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  };

  return (
    <div>
      {/* Pending attachment chips */}
      {attachments.length > 0 && (
        <div className="chat-input__chips">
          {attachments.map((a) => (
            <span key={a.id} className="chat-input__chip">
              {a.name}
              <span
                className="chat-input__chip-remove"
                onClick={() => removeAttachment(a.id)}
              >
                x
              </span>
            </span>
          ))}
        </div>
      )}

      {/* Input row: attach buttons + textarea + send */}
      <div className="chat-input__row">
        {/* File attach */}
        <button
          className="chat-input__attach"
          title="Attach file"
          onClick={() => fileInputRef.current?.click()}
          disabled={isDisabled}
        >
          {"\uD83D\uDCCE"}
        </button>
        <input
          ref={fileInputRef}
          type="file"
          hidden
          multiple
          onChange={(e) => {
            handleFileSelect(e.target.files);
            e.target.value = "";
          }}
        />

        {/* Image attach */}
        <button
          className="chat-input__attach"
          title="Attach image"
          onClick={() => imageInputRef.current?.click()}
          disabled={isDisabled}
        >
          {"\uD83D\uDDBC\uFE0F"}
        </button>
        <input
          ref={imageInputRef}
          type="file"
          hidden
          multiple
          accept="image/*"
          onChange={(e) => {
            handleFileSelect(e.target.files);
            e.target.value = "";
          }}
        />

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          className="chat-input__field"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          disabled={isDisabled}
          rows={1}
        />

        {/* Send button */}
        <button
          className="chat-input__send"
          onClick={handleSend}
          disabled={!canSend || isDisabled}
        >
          Send
        </button>
      </div>

      {uploading && (
        <div className="chat-input__uploading">
          Uploading...
        </div>
      )}
    </div>
  );
}
