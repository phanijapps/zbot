// ============================================================================
// OUTBOUND TAB
// Transport config display, response schemas editor, toggle
// ============================================================================

import { useState } from "react";
import { Plus, Trash2, ChevronDown, ChevronRight, Globe, Terminal, FileJson } from "lucide-react";
import { getTransport } from "@/services/transport";
import type {
  ConnectorResponse,
  ConnectorResponseSchema,
  UpdateConnectorRequest,
} from "@/services/transport/types";
import { ResponseSchemaEditor } from "../editors/ResponseSchemaEditor";

interface OutboundTabProps {
  connector: ConnectorResponse;
  onUpdate: () => void;
}

export function OutboundTab({ connector, onUpdate }: OutboundTabProps) {
  const [expandedSchema, setExpandedSchema] = useState<string | null>(null);

  const handleToggleOutbound = async () => {
    try {
      const transport = await getTransport();
      const request: UpdateConnectorRequest = {
        outbound_enabled: !connector.outbound_enabled,
      };
      const result = await transport.updateConnector(connector.id, request);
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  const handleAddSchema = async () => {
    const newSchema: ConnectorResponseSchema = {
      name: `schema-${Date.now()}`,
      schema: { type: "object", properties: {} },
      description: "",
    };

    const updatedSchemas = [...(connector.metadata.response_schemas || []), newSchema];
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, response_schemas: updatedSchemas },
      });
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  const handleUpdateSchema = async (index: number, updated: ConnectorResponseSchema) => {
    const schemas = [...(connector.metadata.response_schemas || [])];
    schemas[index] = updated;
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, response_schemas: schemas },
      });
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  const handleRemoveSchema = async (index: number) => {
    const schemas = (connector.metadata.response_schemas || []).filter((_, i) => i !== index);
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, response_schemas: schemas },
      });
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  return (
    <div className="space-y-5">
      {/* Toggle */}
      <div className="card card--bordered card__padding">
        <div className="flex items-center justify-between">
          <div>
            <h4 className="text-sm font-medium text-[var(--foreground)]">Outbound Dispatch</h4>
            <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
              Send agent responses to this connector
            </p>
          </div>
          <button
            onClick={handleToggleOutbound}
            className={`btn btn--sm ${
              connector.outbound_enabled
                ? "btn--primary"
                : "btn--secondary"
            }`}
          >
            {connector.outbound_enabled ? "Enabled" : "Disabled"}
          </button>
        </div>
      </div>

      {/* Transport config */}
      <div className="card card--bordered card__padding">
        <div className="flex items-center gap-2 mb-3">
          <div className="card__icon card__icon--primary" style={{ width: 28, height: 28 }}>
            {connector.transport.type === "http" ? (
              <Globe className="w-3.5 h-3.5" />
            ) : (
              <Terminal className="w-3.5 h-3.5" />
            )}
          </div>
          <h4 className="text-sm font-medium text-[var(--foreground)]">Transport</h4>
        </div>
        {connector.transport.type === "http" ? (
          <div className="grid grid-cols-2 gap-4">
            <div>
              <span className="text-xs text-[var(--muted-foreground)]">URL</span>
              <p className="text-sm text-[var(--foreground)] font-mono mt-0.5 truncate">
                {connector.transport.callback_url}
              </p>
            </div>
            <div>
              <span className="text-xs text-[var(--muted-foreground)]">Method</span>
              <p className="text-sm text-[var(--foreground)] mt-0.5">
                {connector.transport.method}
              </p>
            </div>
            {Object.keys(connector.transport.headers || {}).length > 0 && (
              <div className="col-span-2">
                <span className="text-xs text-[var(--muted-foreground)]">Headers</span>
                <div className="mt-1 space-y-0.5">
                  {Object.entries(connector.transport.headers).map(([key, value]) => (
                    <p key={key} className="text-xs font-mono text-[var(--foreground)]">
                      <span className="text-[var(--muted-foreground)]">{key}:</span>{" "}
                      {value.length > 40 ? value.slice(0, 40) + "..." : value}
                    </p>
                  ))}
                </div>
              </div>
            )}
          </div>
        ) : connector.transport.type === "cli" ? (
          <div className="grid grid-cols-2 gap-4">
            <div>
              <span className="text-xs text-[var(--muted-foreground)]">Command</span>
              <p className="text-sm text-[var(--foreground)] font-mono mt-0.5">
                {connector.transport.command}
              </p>
            </div>
            {connector.transport.args?.length > 0 && (
              <div>
                <span className="text-xs text-[var(--muted-foreground)]">Args</span>
                <p className="text-sm text-[var(--foreground)] font-mono mt-0.5">
                  {connector.transport.args.join(" ")}
                </p>
              </div>
            )}
          </div>
        ) : (
          <p className="text-sm text-[var(--muted-foreground)]">
            {connector.transport.type} transport
          </p>
        )}
      </div>

      {/* Response schemas */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h4 className="text-sm font-medium text-[var(--foreground)]">Response Schemas</h4>
          <button
            onClick={handleAddSchema}
            className="btn btn--ghost btn--sm"
          >
            <Plus className="w-3.5 h-3.5" /> Add Schema
          </button>
        </div>

        {(!connector.metadata.response_schemas ||
          connector.metadata.response_schemas.length === 0) ? (
          <div className="empty-state" style={{ padding: "var(--spacing-8)" }}>
            <div className="empty-state__icon">
              <FileJson className="w-5 h-5" />
            </div>
            <p className="empty-state__description">No response schemas defined</p>
          </div>
        ) : (
          <div className="space-y-2">
            {connector.metadata.response_schemas.map((schema, index) => (
              <div key={`${schema.name}-${index}`} className="card card--bordered">
                <div
                  className="flex items-center justify-between px-4 py-3 cursor-pointer"
                  onClick={() =>
                    setExpandedSchema(expandedSchema === schema.name ? null : schema.name)
                  }
                >
                  <div className="flex items-center gap-2">
                    {expandedSchema === schema.name ? (
                      <ChevronDown className="w-4 h-4 text-[var(--muted-foreground)]" />
                    ) : (
                      <ChevronRight className="w-4 h-4 text-[var(--muted-foreground)]" />
                    )}
                    <span className="text-sm font-medium text-[var(--foreground)]">
                      {schema.name}
                    </span>
                    {schema.description && (
                      <span className="text-xs text-[var(--muted-foreground)]">
                        {schema.description}
                      </span>
                    )}
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemoveSchema(index);
                    }}
                    className="btn btn--icon-ghost btn--icon-danger"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
                {expandedSchema === schema.name && (
                  <div className="px-4 pb-4 border-t border-[var(--border)]">
                    <div className="pt-4">
                      <ResponseSchemaEditor
                        schema={schema}
                        onSave={(updated) => handleUpdateSchema(index, updated)}
                      />
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
