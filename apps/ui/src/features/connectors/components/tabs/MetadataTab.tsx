// ============================================================================
// METADATA TAB
// Resources list/editor, context textarea
// ============================================================================

import { useState } from "react";
import { Plus, Trash2, ChevronDown, ChevronRight, Save, Database, Zap } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ConnectorResponse, ConnectorResource } from "@/services/transport/types";
import { ResourceEditor } from "../editors/ResourceEditor";

interface MetadataTabProps {
  connector: ConnectorResponse;
  onUpdate: () => void;
}

export function MetadataTab({ connector, onUpdate }: MetadataTabProps) {
  const [expandedResource, setExpandedResource] = useState<string | null>(null);
  const [contextText, setContextText] = useState(connector.metadata.context || "");
  const [contextDirty, setContextDirty] = useState(false);

  const handleAddResource = async () => {
    const newResource: ConnectorResource = {
      name: `resource-${Date.now()}`,
      uri: "https://api.example.com/resource",
      method: "GET",
      headers: {},
    };

    const updatedResources = [...(connector.metadata.resources || []), newResource];
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, resources: updatedResources },
      });
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  const handleUpdateResource = async (index: number, updated: ConnectorResource) => {
    const resources = [...(connector.metadata.resources || [])];
    resources[index] = updated;
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, resources },
      });
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  const handleRemoveResource = async (index: number) => {
    const resources = (connector.metadata.resources || []).filter((_, i) => i !== index);
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, resources },
      });
      if (result.success) onUpdate();
    } catch {
      // Handled by parent
    }
  };

  const handleSaveContext = async () => {
    try {
      const transport = await getTransport();
      const result = await transport.updateConnector(connector.id, {
        metadata: { ...connector.metadata, context: contextText || undefined },
      });
      if (result.success) {
        setContextDirty(false);
        onUpdate();
      }
    } catch {
      // Handled by parent
    }
  };

  return (
    <div className="space-y-5">
      {/* Resources */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <div>
            <h4 className="text-sm font-medium text-[var(--foreground)]">Resources</h4>
            <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
              Queryable data endpoints for agents
            </p>
          </div>
          <button
            onClick={handleAddResource}
            className="btn btn--ghost btn--sm"
          >
            <Plus className="w-3.5 h-3.5" /> Add Resource
          </button>
        </div>

        {(!connector.metadata.resources || connector.metadata.resources.length === 0) ? (
          <div className="empty-state" style={{ padding: "var(--spacing-8)" }}>
            <div className="empty-state__icon">
              <Database className="w-5 h-5" />
            </div>
            <p className="empty-state__description">No resources defined</p>
          </div>
        ) : (
          <div className="space-y-2">
            {connector.metadata.resources.map((resource, index) => (
              <div key={`${resource.name}-${index}`} className="card card--bordered">
                <div
                  className="flex items-center justify-between px-4 py-3 cursor-pointer"
                  onClick={() =>
                    setExpandedResource(
                      expandedResource === resource.name ? null : resource.name
                    )
                  }
                >
                  <div className="flex items-center gap-2 min-w-0">
                    {expandedResource === resource.name ? (
                      <ChevronDown className="w-4 h-4 text-[var(--muted-foreground)] flex-shrink-0" />
                    ) : (
                      <ChevronRight className="w-4 h-4 text-[var(--muted-foreground)] flex-shrink-0" />
                    )}
                    <span className="text-sm font-medium text-[var(--foreground)]">
                      {resource.name}
                    </span>
                    <span className="badge text-[10px]">
                      {resource.method}
                    </span>
                    <span className="text-xs text-[var(--muted-foreground)] truncate">
                      {resource.uri}
                    </span>
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemoveResource(index);
                    }}
                    className="btn btn--icon-ghost btn--icon-danger flex-shrink-0"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
                {expandedResource === resource.name && (
                  <div className="px-4 pb-4 border-t border-[var(--border)]">
                    <div className="pt-4">
                      <ResourceEditor
                        resource={resource}
                        onSave={(updated) => handleUpdateResource(index, updated)}
                      />
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Context */}
      <div className="card card--bordered card__padding">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h4 className="text-sm font-medium text-[var(--foreground)]">Context</h4>
            <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
              Free-form text injected into agent prompts
            </p>
          </div>
          {contextDirty && (
            <button
              onClick={handleSaveContext}
              className="btn btn--primary btn--sm"
            >
              <Save className="w-3.5 h-3.5" /> Save
            </button>
          )}
        </div>
        <textarea
          value={contextText}
          onChange={(e) => {
            setContextText(e.target.value);
            setContextDirty(true);
          }}
          placeholder="Describe what this connector provides, how to use it, any important context for agents..."
          className="form-input form-textarea"
          style={{ height: "120px", resize: "vertical" }}
        />
      </div>

      {/* Capabilities (read-only view) */}
      {connector.metadata.capabilities?.length > 0 && (
        <div>
          <h4 className="text-sm font-medium text-[var(--foreground)] mb-3">Capabilities</h4>
          <div className="space-y-2">
            {connector.metadata.capabilities.map((cap) => (
              <div key={cap.name} className="card card--bordered card__padding">
                <div className="flex items-center gap-2">
                  <div className="card__icon card__icon--primary" style={{ width: 24, height: 24 }}>
                    <Zap className="w-3 h-3" />
                  </div>
                  <span className="text-sm font-medium text-[var(--foreground)]">{cap.name}</span>
                  {cap.description && (
                    <span className="text-xs text-[var(--muted-foreground)]">{cap.description}</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
