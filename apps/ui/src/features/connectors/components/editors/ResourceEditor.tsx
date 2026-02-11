// ============================================================================
// RESOURCE EDITOR
// Inline editor for a single ConnectorResource
// ============================================================================

import { useState } from "react";
import { Save, Plus, X } from "lucide-react";
import type { ConnectorResource } from "@/services/transport/types";

interface ResourceEditorProps {
  resource: ConnectorResource;
  onSave: (updated: ConnectorResource) => void;
}

export function ResourceEditor({ resource, onSave }: ResourceEditorProps) {
  const [name, setName] = useState(resource.name);
  const [uri, setUri] = useState(resource.uri);
  const [method, setMethod] = useState(resource.method);
  const [description, setDescription] = useState(resource.description || "");
  const [headers, setHeaders] = useState<[string, string][]>(
    Object.entries(resource.headers || {})
  );
  const [schemaText, setSchemaText] = useState(
    resource.response_schema ? JSON.stringify(resource.response_schema, null, 2) : ""
  );

  const handleSave = () => {
    let parsedSchema: Record<string, unknown> | undefined;
    if (schemaText.trim()) {
      try {
        parsedSchema = JSON.parse(schemaText);
      } catch {
        return; // Invalid JSON — don't save
      }
    }

    onSave({
      name,
      uri,
      method,
      description: description || undefined,
      headers: Object.fromEntries(headers.filter(([k]) => k)),
      response_schema: parsedSchema,
    });
  };

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div className="form-group">
          <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>Name</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="form-input"
          />
        </div>
        <div className="form-group">
          <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>Method</label>
          <select
            value={method}
            onChange={(e) => setMethod(e.target.value)}
            className="form-input form-select"
          >
            <option value="GET">GET</option>
            <option value="POST">POST</option>
          </select>
        </div>
      </div>

      <div className="form-group">
        <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>URI</label>
        <input
          type="text"
          value={uri}
          onChange={(e) => setUri(e.target.value)}
          placeholder="https://api.example.com/resource/{id}"
          className="form-input form-input--mono"
        />
      </div>

      <div className="form-group">
        <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>Description</label>
        <input
          type="text"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="What this resource provides"
          className="form-input"
        />
      </div>

      {/* Headers */}
      <div>
        <div className="flex items-center justify-between mb-1.5">
          <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>Headers</label>
          <button
            onClick={() => setHeaders([...headers, ["", ""]])}
            className="btn btn--ghost btn--sm"
            style={{ padding: "2px 8px" }}
          >
            <Plus className="w-3 h-3" /> Add
          </button>
        </div>
        {headers.map(([key, value], i) => (
          <div key={i} className="flex gap-2 mb-1.5">
            <input
              type="text"
              value={key}
              onChange={(e) => {
                const updated = [...headers];
                updated[i] = [e.target.value, value];
                setHeaders(updated);
              }}
              placeholder="Key"
              className="form-input"
              style={{ flex: 1, fontSize: "var(--text-xs)" }}
            />
            <input
              type="text"
              value={value}
              onChange={(e) => {
                const updated = [...headers];
                updated[i] = [key, e.target.value];
                setHeaders(updated);
              }}
              placeholder="Value"
              className="form-input"
              style={{ flex: 1, fontSize: "var(--text-xs)" }}
            />
            <button
              onClick={() => setHeaders(headers.filter((_, j) => j !== i))}
              className="btn btn--icon-ghost btn--icon-danger"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        ))}
      </div>

      {/* Response Schema */}
      <div className="form-group">
        <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>
          Response Schema (JSON)
        </label>
        <textarea
          value={schemaText}
          onChange={(e) => setSchemaText(e.target.value)}
          placeholder='{"type": "object", "properties": {...}}'
          className="form-input form-input--mono form-textarea"
          style={{ height: "96px", resize: "vertical", fontSize: "var(--text-xs)" }}
        />
      </div>

      <button
        onClick={handleSave}
        className="btn btn--primary btn--sm"
      >
        <Save className="w-3.5 h-3.5" /> Save
      </button>
    </div>
  );
}
