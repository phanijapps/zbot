// ============================================================================
// CONNECTORS PANEL
// Management interface for external connectors
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import {
  Cable,
  Plus,
  Trash2,
  Pencil,
  X,
  Loader2,
  Globe,
  Terminal,
  ToggleLeft,
  ToggleRight,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/shared/ui/dialog";
import { getTransport } from "@/services/transport";
import type {
  ConnectorResponse,
  CreateConnectorRequest,
  ConnectorTransport,
} from "@/services/transport/types";
import { ConnectorDetail } from "./components/ConnectorDetail";

// ============================================================================
// Types
// ============================================================================

type DialogMode = "create" | "edit";
type TransportType = "http" | "cli";

interface HeaderEntry {
  id: string;
  key: string;
  value: string;
}

// Default API base — same origin during development
const API_BASE = typeof window !== "undefined"
  ? `${window.location.protocol}//${window.location.hostname}:18791`
  : "http://localhost:18791";

// ============================================================================
// Component
// ============================================================================

export function WebConnectorsPanel() {
  // State
  const [connectors, setConnectors] = useState<ConnectorResponse[]>([]);
  const [selectedConnector, setSelectedConnector] = useState<ConnectorResponse | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Dialog state
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<DialogMode>("create");
  const [editingId, setEditingId] = useState<string | null>(null);

  // Form state
  const [formId, setFormId] = useState("");
  const [formName, setFormName] = useState("");
  const [formTransportType, setFormTransportType] = useState<TransportType>("http");
  const [formCallbackUrl, setFormCallbackUrl] = useState("");
  const [formMethod, setFormMethod] = useState("POST");
  const [formHeaders, setFormHeaders] = useState<HeaderEntry[]>([]);
  const [formCommand, setFormCommand] = useState("");
  const [formArgs, setFormArgs] = useState("");
  const [formEnabled, setFormEnabled] = useState(true);
  const [formOutboundEnabled, setFormOutboundEnabled] = useState(true);
  const [formInboundEnabled, setFormInboundEnabled] = useState(true);

  // ─────────────────────────────────────────────────────────────────────────
  // Data Loading
  // ─────────────────────────────────────────────────────────────────────────

  const loadConnectors = useCallback(async () => {
    try {
      const transport = await getTransport();
      const result = await transport.listConnectors();
      if (result.success && result.data) {
        setConnectors(result.data);
      } else {
        setError(result.error || "Failed to load connectors");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadConnectors();
  }, [loadConnectors]);

  // Update selected connector when selection changes
  useEffect(() => {
    if (selectedId) {
      const connector = connectors.find((c) => c.id === selectedId);
      setSelectedConnector(connector || null);
    } else {
      setSelectedConnector(null);
    }
  }, [selectedId, connectors]);

  // ─────────────────────────────────────────────────────────────────────────
  // Dialog Handlers
  // ─────────────────────────────────────────────────────────────────────────

  const openCreateDialog = () => {
    setDialogMode("create");
    setEditingId(null);
    setFormId("");
    setFormName("");
    setFormTransportType("http");
    setFormCallbackUrl("");
    setFormMethod("POST");
    setFormHeaders([]);
    setFormCommand("");
    setFormArgs("");
    setFormEnabled(true);
    setFormOutboundEnabled(true);
    setFormInboundEnabled(true);
    setDialogOpen(true);
  };

  const openEditDialog = () => {
    if (!selectedConnector) return;

    setDialogMode("edit");
    setEditingId(selectedConnector.id);
    setFormId(selectedConnector.id);
    setFormName(selectedConnector.name);
    setFormEnabled(selectedConnector.enabled);
    setFormOutboundEnabled(selectedConnector.outbound_enabled);
    setFormInboundEnabled(selectedConnector.inbound_enabled);

    if (selectedConnector.transport.type === "http") {
      setFormTransportType("http");
      setFormCallbackUrl(selectedConnector.transport.callback_url);
      setFormMethod(selectedConnector.transport.method);
      setFormHeaders(
        Object.entries(selectedConnector.transport.headers || {}).map(([key, value]) => ({
          id: crypto.randomUUID(),
          key,
          value,
        }))
      );
    } else if (selectedConnector.transport.type === "cli") {
      setFormTransportType("cli");
      setFormCommand(selectedConnector.transport.command);
      setFormArgs(selectedConnector.transport.args?.join(", ") || "");
    }

    setDialogOpen(true);
  };

  const handleSave = async () => {
    try {
      const transport = await getTransport();

      // Build transport config based on type
      let transportConfig: ConnectorTransport;
      if (formTransportType === "http") {
        transportConfig = {
          type: "http",
          callback_url: formCallbackUrl,
          method: formMethod,
          headers: formHeaders.reduce(
            (acc, h) => (h.key ? { ...acc, [h.key]: h.value } : acc),
            {} as Record<string, string>
          ),
        };
      } else {
        transportConfig = {
          type: "cli",
          command: formCommand,
          args: formArgs
            .split(",")
            .map((a) => a.trim())
            .filter(Boolean),
          env: {},
        };
      }

      if (dialogMode === "edit" && editingId) {
        const result = await transport.updateConnector(editingId, {
          name: formName,
          transport: transportConfig,
          enabled: formEnabled,
          outbound_enabled: formOutboundEnabled,
          inbound_enabled: formInboundEnabled,
        });

        if (result.success) {
          await loadConnectors();
          setDialogOpen(false);
        } else {
          setError(result.error || "Failed to update connector");
        }
      } else {
        const request: CreateConnectorRequest = {
          id: formId,
          name: formName,
          transport: transportConfig,
          enabled: formEnabled,
          outbound_enabled: formOutboundEnabled,
          inbound_enabled: formInboundEnabled,
        };

        const result = await transport.createConnector(request);

        if (result.success) {
          await loadConnectors();
          setDialogOpen(false);
          if (result.data) {
            setSelectedId(result.data.id);
          }
        } else {
          setError(result.error || "Failed to create connector");
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  // ─────────────────────────────────────────────────────────────────────────
  // Actions
  // ─────────────────────────────────────────────────────────────────────────

  const handleDelete = async () => {
    if (!selectedId) return;
    if (!confirm("Are you sure you want to delete this connector?")) return;

    try {
      const transport = await getTransport();
      const result = await transport.deleteConnector(selectedId);

      if (result.success) {
        setSelectedId(null);
        setSelectedConnector(null);
        await loadConnectors();
      } else {
        setError(result.error || "Failed to delete connector");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  const handleToggleEnabled = async () => {
    if (!selectedId || !selectedConnector) return;

    try {
      const transport = await getTransport();
      const result = selectedConnector.enabled
        ? await transport.disableConnector(selectedId)
        : await transport.enableConnector(selectedId);

      if (result.success) {
        await loadConnectors();
      } else {
        setError(result.error || "Failed to toggle connector");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    }
  };

  // ─────────────────────────────────────────────────────────────────────────
  // Header Helpers
  // ─────────────────────────────────────────────────────────────────────────

  const addHeader = () => {
    setFormHeaders([...formHeaders, { id: crypto.randomUUID(), key: "", value: "" }]);
  };

  const removeHeader = (id: string) => {
    setFormHeaders(formHeaders.filter((h) => h.id !== id));
  };

  const updateHeader = (id: string, field: "key" | "value", value: string) => {
    setFormHeaders(formHeaders.map((h) => (h.id === id ? { ...h, [field]: value } : h)));
  };

  // ─────────────────────────────────────────────────────────────────────────
  // Render
  // ─────────────────────────────────────────────────────────────────────────

  if (isLoading) {
    return (
      <div className="loading-spinner">
        <Loader2 className="loading-spinner__icon animate-spin" />
      </div>
    );
  }

  return (
    <div className="page">
      <div className="split-panel">
        {/* Left sidebar - Connector list */}
        <div className="split-panel__sidebar">
          <div className="page-header" style={{ padding: "var(--spacing-4)", marginBottom: 0 }}>
            <div>
              <h2 className="page-title" style={{ fontSize: "var(--text-lg)" }}>Connectors</h2>
              <p className="page-subtitle">External bridges for messaging</p>
            </div>
            <button
              onClick={openCreateDialog}
              className="btn btn--primary btn--icon"
              title="Add connector"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>

          {error && (
            <div className="alert alert--error" style={{ margin: "var(--spacing-2) var(--spacing-3)", borderRadius: "var(--radius-md)" }}>
              <span className="flex-1 text-xs">{error}</span>
              <button onClick={() => setError(null)} className="alert__dismiss">
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          )}

          <div className="flex-1 overflow-auto" style={{ padding: "var(--spacing-2) var(--spacing-3)" }}>
            {connectors.length === 0 ? (
              <div className="empty-state">
                <div className="empty-state__icon">
                  <Cable className="w-5 h-5" />
                </div>
                <p className="empty-state__title">No connectors</p>
                <p className="empty-state__description">
                  Create a connector to bridge external systems
                </p>
              </div>
            ) : (
              <div className="space-y-1.5">
                {connectors.map((connector) => (
                  <button
                    key={connector.id}
                    onClick={() => setSelectedId(connector.id)}
                    className={`w-full text-left p-3 rounded-lg transition-all ${
                      selectedId === connector.id
                        ? "bg-[var(--primary)]/10 border border-[var(--primary)]/30"
                        : "bg-[var(--card)] hover:bg-[var(--muted)] border border-transparent"
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <div
                        className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${
                          connector.enabled
                            ? "bg-[var(--primary-muted)]"
                            : "bg-[var(--muted)]"
                        }`}
                      >
                        {connector.transport.type === "http" ? (
                          <Globe className={`w-4 h-4 ${connector.enabled ? "text-[var(--primary)]" : "text-[var(--muted-foreground)]"}`} />
                        ) : (
                          <Terminal className={`w-4 h-4 ${connector.enabled ? "text-[var(--primary)]" : "text-[var(--muted-foreground)]"}`} />
                        )}
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="list-item__title truncate">
                          {connector.name}
                        </div>
                        <div className="list-item__subtitle">
                          {connector.transport.type === "http"
                            ? connector.transport.callback_url
                            : connector.transport.type === "cli"
                              ? connector.transport.command
                              : connector.transport.type}
                        </div>
                      </div>
                      <div
                        className={`w-2 h-2 rounded-full flex-shrink-0 ${
                          connector.enabled ? "bg-[var(--success)]" : "bg-[var(--muted-foreground)]"
                        }`}
                      />
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Right panel - Tab-based detail view */}
        <div className="split-panel__content">
          {selectedConnector ? (
            <div style={{ padding: "var(--spacing-6)" }} className="h-full overflow-auto">
              {/* Header */}
              <div className="flex items-start justify-between mb-6">
                <div className="flex items-center gap-4">
                  <div
                    className={`w-12 h-12 rounded-xl flex items-center justify-center ${
                      selectedConnector.enabled
                        ? "bg-[var(--primary-muted)]"
                        : "bg-[var(--muted)]"
                    }`}
                  >
                    {selectedConnector.transport.type === "http" ? (
                      <Globe className="w-6 h-6 text-[var(--primary)]" />
                    ) : (
                      <Terminal className="w-6 h-6 text-[var(--primary)]" />
                    )}
                  </div>
                  <div>
                    <h3 className="text-xl font-semibold text-[var(--foreground)]">
                      {selectedConnector.name}
                    </h3>
                    <p className="text-sm text-[var(--muted-foreground)]">{selectedConnector.id}</p>
                  </div>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={handleToggleEnabled}
                    className="btn btn--icon-ghost"
                    title={selectedConnector.enabled ? "Disable" : "Enable"}
                  >
                    {selectedConnector.enabled ? (
                      <ToggleRight className="w-5 h-5 text-[var(--success)]" />
                    ) : (
                      <ToggleLeft className="w-5 h-5 text-[var(--muted-foreground)]" />
                    )}
                  </button>
                  <button
                    onClick={openEditDialog}
                    className="btn btn--icon-ghost"
                    title="Edit"
                  >
                    <Pencil className="w-4 h-4" />
                  </button>
                  <button
                    onClick={handleDelete}
                    className="btn btn--icon-ghost btn--icon-danger"
                    title="Delete"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>

              {/* Tabs */}
              <ConnectorDetail
                key={selectedConnector.id}
                connector={selectedConnector}
                apiBase={API_BASE}
                onUpdate={loadConnectors}
              />
            </div>
          ) : (
            <div className="split-panel__empty">
              <div className="empty-state">
                <div className="empty-state__icon">
                  <Cable className="w-6 h-6" />
                </div>
                <p className="empty-state__title">Select a connector</p>
                <p className="empty-state__description">Choose a connector from the sidebar to view details</p>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Create/Edit Dialog */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="bg-[var(--card)] border-[var(--border)] max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {dialogMode === "create" ? "Create Connector" : "Edit Connector"}
            </DialogTitle>
            <DialogDescription>
              {dialogMode === "create"
                ? "Add a new external connector for messaging"
                : "Update connector configuration"}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            {/* ID (only for create) */}
            {dialogMode === "create" && (
              <div className="form-group">
                <label className="form-label">ID</label>
                <input
                  type="text"
                  value={formId}
                  onChange={(e) => setFormId(e.target.value)}
                  placeholder="my-connector"
                  className="form-input"
                />
              </div>
            )}

            {/* Name */}
            <div className="form-group">
              <label className="form-label">Name</label>
              <input
                type="text"
                value={formName}
                onChange={(e) => setFormName(e.target.value)}
                placeholder="My Connector"
                className="form-input"
              />
            </div>

            {/* Transport type */}
            <div className="form-group">
              <label className="form-label">Transport Type</label>
              <select
                value={formTransportType}
                onChange={(e) => setFormTransportType(e.target.value as TransportType)}
                className="form-input form-select"
              >
                <option value="http">HTTP Webhook</option>
                <option value="cli">CLI Command</option>
              </select>
            </div>

            {/* HTTP transport fields */}
            {formTransportType === "http" && (
              <>
                <div className="form-group">
                  <label className="form-label">Callback URL</label>
                  <input
                    type="url"
                    value={formCallbackUrl}
                    onChange={(e) => setFormCallbackUrl(e.target.value)}
                    placeholder="https://example.com/webhook"
                    className="form-input"
                  />
                </div>
                <div className="form-group">
                  <label className="form-label">Method</label>
                  <select
                    value={formMethod}
                    onChange={(e) => setFormMethod(e.target.value)}
                    className="form-input form-select"
                  >
                    <option value="POST">POST</option>
                    <option value="PUT">PUT</option>
                  </select>
                </div>
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <label className="form-label">Headers</label>
                    <button
                      type="button"
                      onClick={addHeader}
                      className="btn btn--ghost btn--sm"
                      style={{ padding: "2px 8px" }}
                    >
                      + Add Header
                    </button>
                  </div>
                  {formHeaders.map((header) => (
                    <div key={header.id} className="flex gap-2 mb-2">
                      <input
                        type="text"
                        value={header.key}
                        onChange={(e) => updateHeader(header.id, "key", e.target.value)}
                        placeholder="Key"
                        className="form-input"
                        style={{ flex: 1, fontSize: "var(--text-sm)" }}
                      />
                      <input
                        type="text"
                        value={header.value}
                        onChange={(e) => updateHeader(header.id, "value", e.target.value)}
                        placeholder="Value"
                        className="form-input"
                        style={{ flex: 1, fontSize: "var(--text-sm)" }}
                      />
                      <button
                        type="button"
                        onClick={() => removeHeader(header.id)}
                        className="btn btn--icon-ghost btn--icon-danger"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* CLI transport fields */}
            {formTransportType === "cli" && (
              <>
                <div className="form-group">
                  <label className="form-label">Command</label>
                  <input
                    type="text"
                    value={formCommand}
                    onChange={(e) => setFormCommand(e.target.value)}
                    placeholder="/usr/local/bin/my-script"
                    className="form-input form-input--mono"
                  />
                </div>
                <div className="form-group">
                  <label className="form-label">Arguments (comma-separated)</label>
                  <input
                    type="text"
                    value={formArgs}
                    onChange={(e) => setFormArgs(e.target.value)}
                    placeholder="--flag, value, --another"
                    className="form-input form-input--mono"
                  />
                </div>
              </>
            )}

            {/* Enabled toggles */}
            <div className="flex gap-5">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formEnabled}
                  onChange={(e) => setFormEnabled(e.target.checked)}
                  className="w-4 h-4 rounded accent-[var(--primary)]"
                />
                <span className="text-sm text-[var(--foreground)]">Enabled</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formOutboundEnabled}
                  onChange={(e) => setFormOutboundEnabled(e.target.checked)}
                  className="w-4 h-4 rounded accent-[var(--primary)]"
                />
                <span className="text-sm text-[var(--foreground)]">Outbound</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formInboundEnabled}
                  onChange={(e) => setFormInboundEnabled(e.target.checked)}
                  className="w-4 h-4 rounded accent-[var(--primary)]"
                />
                <span className="text-sm text-[var(--foreground)]">Inbound</span>
              </label>
            </div>
          </div>

          <DialogFooter>
            <button
              onClick={() => setDialogOpen(false)}
              className="btn btn--secondary btn--md"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={!formName || (formTransportType === "http" ? !formCallbackUrl : !formCommand)}
              className="btn btn--primary btn--md"
            >
              {dialogMode === "create" ? "Create" : "Save"}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
