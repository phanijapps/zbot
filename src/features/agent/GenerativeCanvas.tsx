// ============================================================================
// GENERATIVE CANVAS
// Modal for generative UI - shows content or input forms
// Cannot be dismissed by clicking outside - only via cancel or submit buttons
// ============================================================================

import { useState, useCallback, useEffect } from "react";
import type { ShowContentEvent, RequestInputEvent } from "@/shared/types";

// ============================================================================
// Types
// ============================================================================

export type ContentState =
  | { type: "show_content"; event: ShowContentEvent }
  | { type: "request_input"; event: RequestInputEvent }
  | null;

interface GenerativeCanvasProps {
  isOpen: boolean;
  content: ContentState;
  onClose: () => void;
  onFormSubmit?: (formId: string, data: Record<string, unknown>) => void;
  onFormCancel?: (formId: string) => void;
}

// ============================================================================
// Main Component
// ============================================================================

export function GenerativeCanvas({
  isOpen,
  content,
  onClose,
  onFormSubmit,
  onFormCancel,
}: GenerativeCanvasProps) {
  if (!isOpen || !content) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center">
      {/* Backdrop - does NOT close on click (user requested) */}
      <div className="absolute inset-0 bg-black/80 backdrop-blur-sm" />

      {/* Canvas Panel - slides from bottom */}
      <div className="relative w-full max-w-4xl max-h-[85vh] bg-gradient-to-br from-gray-900 to-gray-950 border-t border-gray-700 rounded-t-2xl shadow-2xl flex flex-col animate-in slide-in-from-bottom duration-300">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-700 shrink-0">
          <div className="flex items-center gap-2.5">
            {content.type === "show_content" && (
              <>
                <div className="bg-gradient-to-br from-violet-500 to-pink-600 p-1.5 rounded-lg">
                  <ContentIcon contentType={content.event.contentType} />
                </div>
                <div>
                  <h2 className="text-sm font-medium text-white">{content.event.title}</h2>
                  <p className="text-xs text-gray-500 capitalize">{content.event.contentType}</p>
                </div>
              </>
            )}
            {content.type === "request_input" && (
              <>
                <div className="bg-gradient-to-br from-blue-500 to-cyan-600 p-1.5 rounded-lg">
                  <InputIcon />
                </div>
                <div>
                  <h2 className="text-sm font-medium text-white">{content.event.title}</h2>
                  <p className="text-xs text-gray-500">Input Required</p>
                </div>
              </>
            )}
          </div>

          {/* Close button only for show_content */}
          {content.type === "show_content" && (
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-white h-8 w-8 flex items-center justify-center rounded-lg hover:bg-gray-800 transition-colors"
            >
              <XIcon />
            </button>
          )}
        </div>

        {/* Content Area */}
        <div className="flex-1 overflow-y-auto p-4">
          {content.type === "show_content" && (
            <ContentViewer event={content.event} />
          )}

          {content.type === "request_input" && (
            <FormViewer
              event={content.event}
              onSubmit={(data) => {
                onFormSubmit?.(content.event.formId, data);
                onClose();
              }}
              onCancel={() => {
                onFormCancel?.(content.event.formId);
                onClose();
              }}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Content Viewer
// ============================================================================

function ContentViewer({ event }: { event: ShowContentEvent }) {
  const content = event.content;

  switch (event.contentType) {
    case "pdf":
      return (
        <div className="bg-white rounded-lg p-4 min-h-[400px] flex items-center justify-center">
          <p className="text-gray-800 text-sm">PDF Viewer: {event.title}</p>
        </div>
      );

    case "ppt":
      return (
        <div className="bg-white rounded-lg p-4 min-h-[400px] flex items-center justify-center">
          <p className="text-gray-800 text-sm">PPT Viewer: {event.title}</p>
        </div>
      );

    case "html":
      return (
        <div className="bg-white rounded-lg overflow-hidden min-h-[400px]">
          <iframe
            srcDoc={content}
            className="w-full h-full min-h-[400px] border-0"
            sandbox="allow-scripts"
            title={event.title}
          />
        </div>
      );

    case "image":
      return (
        <div className="flex items-center justify-center bg-gray-800/50 rounded-lg p-4">
          <img
            src={event.base64 ? `data:image/png;base64,${content}` : content}
            alt={event.title}
            className="max-w-full max-h-[500px] rounded"
          />
        </div>
      );

    case "markdown":
    case "text":
    default:
      return (
        <div className="prose prose-invert prose-sm max-w-none">
          <pre className="whitespace-pre-wrap text-gray-300 text-sm bg-gray-800/50 rounded-lg p-4">
            {content}
          </pre>
        </div>
      );
  }
}

// ============================================================================
// Form Viewer
// ============================================================================

interface FormViewerProps {
  event: RequestInputEvent;
  onSubmit: (data: Record<string, unknown>) => void;
  onCancel: () => void;
}

function FormViewer({ event, onSubmit, onCancel }: FormViewerProps) {
  const [formData, setFormData] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  const schema = event.schema as {
    properties?: Record<string, {
      type?: string;
      title?: string;
      description?: string;
      enum?: string[];
      format?: string;
      required?: boolean;
    }>;
    required?: string[];
  };

  const properties = schema.properties || {};

  const handleSubmit = () => {
    // Basic validation
    const newErrors: Record<string, string> = {};

    if (schema.required) {
      for (const key of schema.required) {
        if (!formData[key]) {
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

  const renderField = (name: string, propSchema: {
    type?: string;
    title?: string;
    description?: string;
    enum?: string[];
    format?: string;
  }) => {
    const value = formData[name];

    switch (propSchema.type) {
      case "string":
        if (propSchema.enum) {
          return (
            <select
              value={(value as string) || ""}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
              className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500"
            >
              <option value="">Select...</option>
              {propSchema.enum.map((option) => (
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
              value={(value as string) || ""}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
              placeholder={propSchema.description || ""}
              className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500 resize-none"
              rows={4}
            />
          );
        }
        return (
          <input
            type="text"
            value={(value as string) || ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
            placeholder={propSchema.description || ""}
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500"
          />
        );

      case "number":
        return (
          <input
            type="number"
            value={(value as number) || ""}
            onChange={(e) => setFormData({ ...formData, [name]: parseFloat(e.target.value) })}
            placeholder={propSchema.description || ""}
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500"
          />
        );

      case "boolean":
        return (
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={(value as boolean) || false}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.checked })}
              className="rounded bg-gray-800 border-gray-700"
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
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500"
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
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500 resize-none font-mono"
            rows={4}
          />
        );

      default:
        return (
          <input
            type="text"
            value={(value as string) || ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
            className="w-full bg-gray-800 border border-gray-700 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-violet-500"
          />
        );
    }
  };

  return (
    <div className="space-y-4">
      {/* Description */}
      {event.description && (
        <p className="text-sm text-gray-400">{event.description}</p>
      )}

      {/* Form Fields */}
      <div className="space-y-3">
        {Object.entries(properties).map(([name, propSchema]) => (
          <div key={name} className="space-y-1.5">
            <label className="text-sm font-medium text-white flex items-center gap-1">
              {propSchema.title || name}
              {schema.required?.includes(name) && <span className="text-red-400">*</span>}
            </label>
            {renderField(name, propSchema)}
            {errors[name] && (
              <p className="text-xs text-red-400">{errors[name]}</p>
            )}
            {propSchema.description && propSchema.type !== "boolean" && (
              <p className="text-xs text-gray-500">{propSchema.description}</p>
            )}
          </div>
        ))}
      </div>

      {/* Action Buttons */}
      <div className="flex gap-2 pt-2">
        <button
          onClick={onCancel}
          className="flex-1 border border-gray-600 text-white hover:bg-gray-800 px-4 py-2 rounded-lg transition-colors"
        >
          Cancel
        </button>
        <button
          onClick={handleSubmit}
          className="flex-1 bg-gradient-to-r from-violet-600 to-blue-600 hover:from-violet-700 hover:to-blue-700 text-white px-4 py-2 rounded-lg transition-colors"
        >
          {event.submitButton || "Submit"}
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// Icons
// ============================================================================

function ContentIcon({ contentType }: { contentType: ShowContentEvent["contentType"] }) {
  return (
    <svg className="size-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      {contentType === "image" ? (
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
      ) : (
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
      )}
    </svg>
  );
}

function InputIcon() {
  return (
    <svg className="size-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
    </svg>
  );
}

function XIcon() {
  return (
    <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}
