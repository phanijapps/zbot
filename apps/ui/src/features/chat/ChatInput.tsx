import { useState, useRef, useCallback } from "react";
import { Paperclip, ArrowUp } from "lucide-react";

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
  /** Absolute path on server filesystem */
  path: string;
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
  const res = await fetch("/api/upload", { method: "POST", body: form });
  if (!res.ok) {
    throw new Error(`Upload failed: ${res.statusText}`);
  }
  const data = await res.json();
  return {
    id: data.id,
    name: data.filename,
    mimeType: data.mime_type,
    size: data.size,
    path: data.path,
  };
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
  const [uploadError, setUploadError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
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
    setUploadError(null);
    try {
      const uploaded = await Promise.all(
        Array.from(files).map((f) => uploadFile(f)),
      );
      setAttachments((prev) => [...prev, ...uploaded]);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Upload failed";
      console.error("File upload failed:", err);
      setUploadError(msg);
      setTimeout(() => setUploadError(null), 5000);
    } finally {
      setUploading(false);
    }
  };

  const removeAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
  };

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    handleFileSelect(e.dataTransfer.files);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  return (
    <div style={{ width: "100%" }} onDrop={handleDrop} onDragOver={handleDragOver} role="region" aria-label="Drop files here">
      {/* Pending attachment chips */}
      {attachments.length > 0 && (
        <div className="chat-input__chips">
          {attachments.map((a) => (
            <span key={a.id} className="chat-input__chip">
              {a.name}
              <span
                className="chat-input__chip-remove"
                onClick={() => removeAttachment(a.id)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") removeAttachment(a.id); }}
              >
                x
              </span>
            </span>
          ))}
        </div>
      )}

      {/* Input container with buttons inside */}
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

        {/* Actions positioned inside the field */}
        <div className="chat-input__actions">
          <button
            className="chat-input__action-btn"
            title="Attach file"
            onClick={() => fileInputRef.current?.click()}
            disabled={isDisabled}
          >
            <Paperclip style={{ width: 18, height: 18 }} />
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

      {uploading && (
        <div className="chat-input__uploading">
          Uploading...
        </div>
      )}
      {uploadError && (
        <div style={{ fontSize: "var(--text-xs)", color: "var(--destructive)", marginTop: 4 }}>
          {uploadError}
        </div>
      )}
    </div>
  );
}
