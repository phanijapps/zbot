// ============================================================================
// RESPONSE SCHEMA EDITOR
// Inline editor for a single ResponseSchema
// ============================================================================

import { useState } from "react";
import { Save } from "lucide-react";
import type { ConnectorResponseSchema } from "@/services/transport/types";

interface ResponseSchemaEditorProps {
  schema: ConnectorResponseSchema;
  onSave: (updated: ConnectorResponseSchema) => void;
}

export function ResponseSchemaEditor({ schema, onSave }: ResponseSchemaEditorProps) {
  const [name, setName] = useState(schema.name);
  const [description, setDescription] = useState(schema.description || "");
  const [schemaText, setSchemaText] = useState(JSON.stringify(schema.schema, null, 2));

  const handleSave = () => {
    let parsed: Record<string, unknown>;
    try {
      parsed = JSON.parse(schemaText);
    } catch {
      return; // Invalid JSON — don't save
    }

    onSave({
      name,
      schema: parsed,
      description: description || undefined,
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
          <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>Description</label>
          <input
            type="text"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What this schema defines"
            className="form-input"
          />
        </div>
      </div>

      <div className="form-group">
        <label className="form-label" style={{ fontSize: "var(--text-xs)" }}>
          Schema (JSON)
        </label>
        <textarea
          value={schemaText}
          onChange={(e) => setSchemaText(e.target.value)}
          className="form-input form-input--mono form-textarea"
          style={{ height: "128px", resize: "vertical", fontSize: "var(--text-xs)" }}
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
