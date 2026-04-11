// ============================================================================
// GENERATIVE CANVAS
// Modal for generative UI - shows content or input forms
// Cannot be dismissed by clicking outside - only via cancel or submit buttons
// ============================================================================

import { useState } from "react";
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
    <div className="canvas-backdrop">
      <div className="canvas-backdrop__overlay" />

      <div className="canvas-panel">
        <div className="canvas-panel__header">
          <div className="canvas-panel__header-info">
            {content.type === "show_content" && (
              <>
                <div className="canvas-panel__icon">
                  <ContentIcon contentType={content.event.contentType} />
                </div>
                <div>
                  <h2 className="canvas-panel__title">{content.event.title}</h2>
                  <p className="canvas-panel__subtitle">{content.event.contentType}</p>
                </div>
              </>
            )}
            {content.type === "request_input" && (
              <>
                <div className="canvas-panel__icon canvas-panel__icon--input">
                  <InputIcon />
                </div>
                <div>
                  <h2 className="canvas-panel__title">{content.event.title}</h2>
                  <p className="canvas-panel__subtitle">Input Required</p>
                </div>
              </>
            )}
          </div>

          {content.type === "show_content" && (
            <button onClick={onClose} className="canvas-panel__close">
              <XIcon />
            </button>
          )}
        </div>

        <div className="canvas-panel__body">
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
        <div className="canvas-content-viewer">
          <p className="canvas-panel__subtitle">PDF Viewer: {event.title}</p>
        </div>
      );

    case "ppt":
      return (
        <div className="canvas-content-viewer">
          <p className="canvas-panel__subtitle">PPT Viewer: {event.title}</p>
        </div>
      );

    case "html":
      return (
        <div className="canvas-content-viewer canvas-content-viewer--html">
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
        <div className="canvas-content-viewer canvas-content-viewer--image">
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
        <div className="canvas-content-viewer">
          <pre className="canvas-content-viewer__text">
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
              className="form-input form-select"
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
              className="form-input form-textarea"
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
            className="form-input"
          />
        );

      case "number":
        return (
          <input
            type="number"
            value={(value as number) || ""}
            onChange={(e) => setFormData({ ...formData, [name]: Number.parseFloat(e.target.value) })}
            placeholder={propSchema.description || ""}
            className="form-input"
          />
        );

      case "boolean":
        return (
          <label className="canvas-form__checkbox-label">
            <input
              type="checkbox"
              checked={(value as boolean) || false}
              onChange={(e) => setFormData({ ...formData, [name]: e.target.checked })}
            />
            <span>{propSchema.description || name}</span>
          </label>
        );

      case "array":
        return (
          <input
            type="text"
            value={Array.isArray(value) ? value.join(", ") : ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value.split(", ").filter(Boolean) })}
            placeholder={propSchema.description || "Comma-separated values"}
            className="form-input"
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
            className="form-input form-textarea form-input--mono"
            rows={4}
          />
        );

      default:
        return (
          <input
            type="text"
            value={(value as string) || ""}
            onChange={(e) => setFormData({ ...formData, [name]: e.target.value })}
            className="form-input"
          />
        );
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {event.description && (
        <p className="canvas-panel__subtitle">{event.description}</p>
      )}

      <div className="flex flex-col gap-3">
        {Object.entries(properties).map(([name, propSchema]) => (
          <div key={name} className="canvas-form__field">
            <label className="canvas-form__label">
              {propSchema.title || name}
              {schema.required?.includes(name) && <span className="canvas-form__required">*</span>}
            </label>
            {renderField(name, propSchema)}
            {errors[name] && (
              <p className="canvas-form__error">{errors[name]}</p>
            )}
            {propSchema.description && propSchema.type !== "boolean" && (
              <p className="canvas-form__hint">{propSchema.description}</p>
            )}
          </div>
        ))}
      </div>

      <div className="canvas-form__actions">
        <button onClick={onCancel} className="btn btn--secondary btn--md" style={{ flex: 1 }}>
          Cancel
        </button>
        <button onClick={handleSubmit} className="btn btn--primary btn--md" style={{ flex: 1 }}>
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
    <svg style={{ width: 20, height: 20 }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
    <svg style={{ width: 20, height: 20 }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
    </svg>
  );
}

function XIcon() {
  return (
    <svg style={{ width: 16, height: 16 }} fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}
