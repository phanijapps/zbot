/**
 * GenerativeCanvas - Sliding canvas panel for generative UI
 *
 * Displays:
 * - Content viewers (PDF, PPT, HTML, images, text, markdown)
 * - Input forms (JSON Schema based)
 *
 * Slides in from the right when the agent triggers generative UI events.
 */

import { useState, useCallback, useEffect } from "react";
import { X, FileText, FileImage, Code, Globe, Download, Loader2 } from "lucide-react";
import { Button } from "@/shared/ui/button";
import { invoke } from "@tauri-apps/api/core";
import type { ShowContentEvent, RequestInputEvent } from "@/shared/types/agent";

interface GenerativeCanvasProps {
  isOpen: boolean;
  onClose: () => void;
  content?: ContentState;
  onFormSubmit?: (data: Record<string, unknown>) => void;
  onCanvasCancel?: () => void;
  conversationId?: string;
}

// Content state for the canvas
export type ContentState =
  | { type: "show_content"; event: ShowContentEvent }
  | { type: "request_input"; event: RequestInputEvent }
  | null;

export function GenerativeCanvas({ isOpen, onClose, content: externalContent, onFormSubmit, onCanvasCancel, conversationId }: GenerativeCanvasProps) {
  const [internalContent, setInternalContent] = useState<ContentState>(null);
  const [loadedContent, setLoadedContent] = useState<string | null>(null);
  const [isLoadingContent, setIsLoadingContent] = useState(false);
  const content = externalContent ?? internalContent;

  // Clear content
  const clearContent = useCallback(() => {
    setInternalContent(null);
    setLoadedContent(null);
  }, []);

  // Update internal content when external content changes
  useEffect(() => {
    if (externalContent) {
      setInternalContent(externalContent as ContentState);
    }
  }, [externalContent]);

  // Load content from attachment file when needed
  useEffect(() => {
    if (content?.type === "show_content" && content.event.isAttachment && content.event.filePath) {
      const filePath = content.event.filePath;
      const loadAttachment = async () => {
        if (!conversationId) {
          console.error("[GenerativeCanvas] Cannot load attachment: missing conversation ID");
          return;
        }

        setIsLoadingContent(true);
        try {
          // Parse file path to get filename
          // Format: "conv_id/attachments/filename"
          const parts = filePath.split("/");
          const filename = parts[parts.length - 1];

          console.log("[GenerativeCanvas] Loading attachment:", {
            conversationId,
            filename,
            filePath,
          });

          const data = await invoke<string>("read_attachment_file", {
            conversationId,
            filename,
          });

          console.log("[GenerativeCanvas] Attachment loaded successfully, length:", data.length);
          setLoadedContent(data);
        } catch (error) {
          console.error("[GenerativeCanvas] Failed to load attachment:", error);
        } finally {
          setIsLoadingContent(false);
        }
      };

      loadAttachment();
    } else {
      // Reset loaded content when not an attachment
      setLoadedContent(null);
    }
  }, [content, conversationId]);

  // Handle download/export
  const handleDownload = useCallback(() => {
    if (content?.type === "show_content") {
      const { contentType, title, content: data, isAttachment } = content.event;
      // Use loaded content if it's an attachment, otherwise use the event content
      const actualContent = isAttachment ? (loadedContent || data) : data;

      // Determine MIME type and file extension
      let mimeType = "text/plain";
      let extension = "txt";

      switch (contentType) {
        case "pdf":
          mimeType = "application/pdf";
          extension = "pdf";
          break;
        case "ppt":
          mimeType = "application/vnd.ms-powerpoint";
          extension = "ppt";
          break;
        case "html":
          mimeType = "text/html";
          extension = "html";
          break;
        case "image":
          mimeType = "image/png";
          extension = "png";
          break;
        case "markdown":
          mimeType = "text/markdown";
          extension = "md";
          break;
        case "text":
        default:
          mimeType = "text/plain";
          extension = "txt";
          break;
      }

      // Decode base64 if needed
      let finalData = actualContent;
      if (contentType === "image" || contentType === "pdf" || contentType === "ppt") {
        // Base64 encoded content
        const byteCharacters = atob(actualContent);
        const byteNumbers = new Array(byteCharacters.length);
        for (let i = 0; i < byteCharacters.length; i++) {
          byteNumbers[i] = byteCharacters.charCodeAt(i);
        }
        const byteArray = new Uint8Array(byteNumbers);
        finalData = byteArray as any;
      }

      // Create blob and download
      const blob = new Blob([finalData], { type: mimeType });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${title}.${extension}`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
    }
  }, [content, loadedContent]);

  // Get icon based on content type
  const getContentTypeIcon = (contentType: ShowContentEvent["contentType"]) => {
    switch (contentType) {
      case "pdf":
      case "ppt":
        return <FileText className="size-5" />;
      case "html":
      case "markdown":
        return <Code className="size-5" />;
      case "image":
        return <FileImage className="size-5" />;
      case "text":
        return <FileText className="size-5" />;
      default:
        return <FileText className="size-5" />;
    }
  };

  if (!isOpen) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-end">
      {/* Solid Backdrop */}
      <div
        className="absolute inset-0 bg-black"
        onClick={onClose}
      />

      {/* Canvas Panel - slides from bottom */}
      <div className="relative w-full max-h-[85vh] bg-gradient-to-br from-[#0a0a0a] to-[#111] border-t border-white/10 rounded-t-2xl shadow-2xl flex flex-col animate-in slide-in-from-bottom duration-300">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10 shrink-0">
          <div className="flex items-center gap-2.5">
            {content?.type === "show_content" && (
              <>
                <div className="bg-gradient-to-br from-purple-500 to-pink-600 p-1.5 rounded-lg">
                  {getContentTypeIcon(content.event.contentType)}
                </div>
                <div>
                  <h2 className="text-sm font-medium text-white">{content.event.title}</h2>
                  <p className="text-xs text-gray-500 capitalize">{content.event.contentType}</p>
                </div>
              </>
            )}
            {content?.type === "request_input" && (
              <>
                <div className="bg-gradient-to-br from-blue-500 to-cyan-600 p-1.5 rounded-lg">
                  <Globe className="size-5" />
                </div>
                <div>
                  <h2 className="text-sm font-medium text-white">{content.event.title}</h2>
                  <p className="text-xs text-gray-500">Input Required</p>
                </div>
              </>
            )}
            {!content && (
              <>
                <div className="bg-gradient-to-br from-gray-600 to-gray-700 p-1.5 rounded-lg">
                  <FileText className="size-5" />
                </div>
                <div>
                  <h2 className="text-sm font-medium text-white">Canvas</h2>
                  <p className="text-xs text-gray-500">Generative UI</p>
                </div>
              </>
            )}
          </div>
          <div className="flex items-center gap-1">
            {content?.type === "show_content" && (
              <Button
                variant="ghost"
                size="sm"
                onClick={handleDownload}
                className="text-gray-400 hover:text-white h-8 w-8 p-0"
                title="Export"
              >
                <Download className="size-4" />
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={onClose}
              className="text-gray-400 hover:text-white h-8 w-8 p-0"
            >
              <X className="size-4" />
            </Button>
          </div>
        </div>

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-4">
          {!content && (
            <div className="flex flex-col items-center justify-center h-full text-center">
              <div className="text-4xl mb-3">🎨</div>
              <p className="text-gray-500 text-sm">Canvas is ready</p>
              <p className="text-gray-600 text-xs mt-1">Content will appear here when the agent generates it</p>
            </div>
          )}

          {content?.type === "show_content" && (
            <ContentViewer event={content.event} loadedContent={loadedContent} isLoadingContent={isLoadingContent} />
          )}

          {content?.type === "request_input" && (
            <FormViewer event={content.event} onSubmit={(data) => {
              console.log("Form submitted:", data);
              clearContent();
              onClose();
              // Call the parent's onFormSubmit if provided
              onFormSubmit?.(data);
            }} onCancel={() => {
              clearContent();
              onClose();
              // Call the parent's onCanvasCancel if provided (for focusing text input)
              onCanvasCancel?.();
            }} />
          )}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// CONTENT VIEWER
// ============================================================================

interface ContentViewerProps {
  event: ShowContentEvent;
  loadedContent: string | null;
  isLoadingContent: boolean;
}

function ContentViewer({ event, loadedContent, isLoadingContent }: ContentViewerProps) {
  // Use loaded content if it's an attachment, otherwise use the event content
  const displayContent = event.isAttachment ? (loadedContent || event.content) : event.content;

  const renderContent = () => {
    // Show loading indicator if loading attachment content
    if (event.isAttachment && isLoadingContent) {
      return (
        <div className="flex items-center justify-center bg-white/5 rounded-lg p-8">
          <div className="flex flex-col items-center gap-3">
            <Loader2 className="size-6 text-purple-500 animate-spin" />
            <p className="text-sm text-gray-400">Loading content...</p>
          </div>
        </div>
      );
    }

    switch (event.contentType) {
      case "pdf":
        return (
          <div className="bg-white rounded-lg p-4 min-h-[600px] flex items-center justify-center">
            <p className="text-gray-800 text-sm">PDF Viewer: {event.title}</p>
          </div>
        );

      case "ppt":
        return (
          <div className="bg-white rounded-lg p-4 min-h-[600px] flex items-center justify-center">
            <p className="text-gray-800 text-sm">PPT Viewer: {event.title}</p>
          </div>
        );

      case "html":
        return (
          <div className="bg-white rounded-lg overflow-hidden min-h-[600px]">
            <iframe
              srcDoc={displayContent}
              className="w-full h-full min-h-[600px] border-0"
              sandbox="allow-scripts"
              title={event.title}
            />
          </div>
        );

      case "image":
        return (
          <div className="flex items-center justify-center bg-white/5 rounded-lg p-4">
            <img
              src={`data:image/png;base64,${displayContent}`}
              alt={event.title}
              className="max-w-full max-h-[600px] rounded"
            />
          </div>
        );

      case "markdown":
        return (
          <div className="prose prose-invert prose-sm max-w-none">
            <pre className="whitespace-pre-wrap text-gray-300 text-sm">{displayContent}</pre>
          </div>
        );

      case "text":
      default:
        return (
          <div className="prose prose-invert prose-sm max-w-none">
            <pre className="whitespace-pre-wrap text-gray-300 text-sm">{displayContent}</pre>
          </div>
        );
    }
  };

  return (
    <div className="space-y-3">
      {event.metadata && Object.keys(event.metadata).length > 0 && (
        <div className="bg-white/5 rounded-lg p-3">
          <p className="text-xs text-gray-500 mb-1">Metadata</p>
          <pre className="text-xs text-gray-400 overflow-x-auto">
            {JSON.stringify(event.metadata, null, 2)}
          </pre>
        </div>
      )}
      {renderContent()}
    </div>
  );
}

// ============================================================================
// FORM VIEWER
// ============================================================================

interface FormViewerProps {
  event: RequestInputEvent;
  onSubmit: (data: Record<string, unknown>) => void;
  onCancel: () => void;
}

function FormViewer({ event, onSubmit, onCancel }: FormViewerProps) {
  const [formData, setFormData] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  const handleSubmit = () => {
    // Validate form
    const newErrors: Record<string, string> = {};

    // Basic validation based on JSON Schema
    const schema = event.schema as any;
    if (schema.properties) {
      for (const [key, propSchema] of Object.entries(schema.properties) as any) {
        const value = formData[key];
        if ((propSchema as any).required && !value) {
          newErrors[key] = `${key} is required`;
        }
      }
    }

    if (Object.keys(newErrors).length > 0) {
      setErrors(newErrors);
      return;
    }

    onSubmit(formData);
  };

  const renderField = (name: string, propSchema: any) => {
    const value = formData[name];
    // const error = errors[name];

    switch (propSchema.type) {
      case "string":
        if (propSchema.enum) {
          return (
            <select
              value={value as string || ""}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
              className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500"
            >
              <option value="">Select...</option>
              {propSchema.enum.map((option: string) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          );
        }
        if (propSchema.format === "textarea") {
          return (
            <textarea
              value={value as string || ""}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
              placeholder={propSchema.description || ""}
              className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500 resize-none"
              rows={4}
            />
          );
        }
        return (
          <input
            type="text"
            value={value as string || ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
            placeholder={propSchema.description || ""}
            className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500"
          />
        );

      case "number":
        return (
          <input
            type="number"
            value={value as number || ""}
            onChange={(e) => setFormData({ ...formData, [name]: parseFloat(e.target.value) })}
            placeholder={propSchema.description || ""}
            className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500"
          />
        );

      case "boolean":
        return (
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={value as boolean || false}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.checked })}
              className="rounded bg-white/5 border-white/10"
            />
            <span className="text-sm text-gray-300">{propSchema.description || name}</span>
          </label>
        );

      case "array":
        return (
          <input
            type="text"
            value={Array.isArray(value) ? value.join(", ") : ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value.split(", ").filter(Boolean) })}
            placeholder={propSchema.description || "Comma-separated values"}
            className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500"
          />
        );

      case "object":
        return (
          <textarea
            value={typeof value === "string" ? value : JSON.stringify(value || {}, null, 2)}
            onChange={(e) => {
              try {
                setFormData({ ...formData, [name]: JSON.parse(e.target.value) });
                setErrors({ ...errors, [name]: "" });
              } catch {
                setErrors({ ...errors, [name]: "Invalid JSON" });
              }
            }}
            placeholder={propSchema.description || "JSON object"}
            className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500 resize-none font-mono"
            rows={4}
          />
        );

      default:
        return (
          <input
            type="text"
            value={value as string || ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
            className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-purple-500"
          />
        );
    }
  };

  const schema = event.schema as any;
  const properties = schema.properties || {};

  return (
    <div className="space-y-4">
      {/* Description */}
      {event.description && (
        <p className="text-sm text-gray-400">{event.description}</p>
      )}

      {/* Form Fields */}
      <div className="space-y-3">
        {Object.entries(properties).map(([name, propSchema]: [string, any]) => (
          <div key={name} className="space-y-1.5">
            <label className="text-sm font-medium text-white flex items-center gap-1">
              {propSchema.title || name}
              {(schema.required?.includes(name)) && <span className="text-red-400">*</span>}
            </label>
            {renderField(name, propSchema)}
            {errors[name] && (
              <p className="text-xs text-red-400">{errors[name]}</p>
            )}
            {propSchema.description && (
              <p className="text-xs text-gray-500">{propSchema.description}</p>
            )}
          </div>
        ))}
      </div>

      {/* Action Buttons */}
      <div className="flex gap-2 pt-2">
        <Button
          variant="outline"
          className="flex-1 border-white/20 text-white hover:bg-white/5"
          onClick={onCancel}
        >
          Cancel
        </Button>
        <Button
          className="flex-1 bg-gradient-to-r from-blue-600 to-cyan-600 hover:from-blue-700 hover:to-cyan-700 text-white"
          onClick={handleSubmit}
        >
          {event.submitButton || "Submit"}
        </Button>
      </div>
    </div>
  );
}

// ============================================================================
// EXPORT HELPERS
// ============================================================================

/**
 * Helper hook to get canvas controller
 */
export function useGenerativeCanvas() {
  return {
    setShowContent: (_event: ShowContentEvent) => {
      // Find the canvas element and trigger show content
      // This will be connected to the actual canvas instance
    },
    setRequestInput: (_event: RequestInputEvent) => {
      // Find the canvas element and trigger request input
    },
  };
}
