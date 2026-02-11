// ============================================================================
// INBOUND TAB
// Endpoint URL, payload docs, toggle, message log
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import { Copy, Check, Loader2, RefreshCw, Inbox } from "lucide-react";
import { getTransport } from "@/services/transport";
import type {
  ConnectorResponse,
  InboundLogEntry,
  UpdateConnectorRequest,
} from "@/services/transport/types";

interface InboundTabProps {
  connector: ConnectorResponse;
  apiBase: string;
  onUpdate: () => void;
}

export function InboundTab({ connector, apiBase, onUpdate }: InboundTabProps) {
  const [logs, setLogs] = useState<InboundLogEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  const endpointUrl = `${apiBase}/api/connectors/${encodeURIComponent(connector.id)}/inbound`;

  const loadLogs = useCallback(async () => {
    try {
      const transport = await getTransport();
      const result = await transport.getConnectorInboundLog(connector.id);
      if (result.success && result.data) {
        setLogs(result.data);
      }
    } catch {
      // Silently fail — log is best-effort
    } finally {
      setIsLoading(false);
    }
  }, [connector.id]);

  useEffect(() => {
    loadLogs();
    const interval = setInterval(loadLogs, 15000);
    return () => clearInterval(interval);
  }, [loadLogs]);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(endpointUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleToggleInbound = async () => {
    try {
      const transport = await getTransport();
      const request: UpdateConnectorRequest = {
        inbound_enabled: !connector.inbound_enabled,
      };
      const result = await transport.updateConnector(connector.id, request);
      if (result.success) {
        onUpdate();
      }
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
            <h4 className="text-sm font-medium text-[var(--foreground)]">Inbound Messages</h4>
            <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
              Accept messages from external systems
            </p>
          </div>
          <button
            onClick={handleToggleInbound}
            className={`btn btn--sm ${
              connector.inbound_enabled
                ? "btn--primary"
                : "btn--secondary"
            }`}
          >
            {connector.inbound_enabled ? "Enabled" : "Disabled"}
          </button>
        </div>
      </div>

      {/* Endpoint URL */}
      <div className="card card--bordered card__padding">
        <h4 className="text-sm font-medium text-[var(--foreground)] mb-3">Endpoint URL</h4>
        <div className="flex items-center gap-2">
          <code className="flex-1 text-xs font-mono bg-[var(--muted)] rounded-md px-3 py-2 text-[var(--foreground)] overflow-x-auto border border-[var(--border)]">
            POST {endpointUrl}
          </code>
          <button
            onClick={handleCopy}
            className="btn btn--icon-ghost"
            title="Copy URL"
          >
            {copied ? (
              <Check className="w-4 h-4 text-[var(--success)]" />
            ) : (
              <Copy className="w-4 h-4" />
            )}
          </button>
        </div>
      </div>

      {/* Payload docs */}
      <div className="card card--bordered card__padding">
        <h4 className="text-sm font-medium text-[var(--foreground)] mb-3">Payload Format</h4>
        <pre className="text-xs font-mono bg-[var(--muted)] rounded-md px-3 py-2.5 text-[var(--foreground)] overflow-x-auto border border-[var(--border)] leading-relaxed">
{`{
  "message": "Hello from external system",
  "thread_id": "optional-thread-id",
  "sender": { "id": "user-123", "name": "Alice" },
  "agent_id": "root",
  "respond_to": ["${connector.id}"],
  "metadata": {}
}`}
        </pre>
      </div>

      {/* Message log */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h4 className="text-sm font-medium text-[var(--foreground)]">Recent Messages</h4>
          <button
            onClick={loadLogs}
            className="btn btn--icon-ghost"
            title="Refresh"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
        </div>

        {isLoading ? (
          <div className="loading-spinner" style={{ height: "120px" }}>
            <Loader2 className="loading-spinner__icon animate-spin" />
          </div>
        ) : logs.length === 0 ? (
          <div className="empty-state" style={{ padding: "var(--spacing-8)" }}>
            <div className="empty-state__icon">
              <Inbox className="w-5 h-5" />
            </div>
            <p className="empty-state__description">No inbound messages yet</p>
          </div>
        ) : (
          <div className="space-y-2 max-h-80 overflow-y-auto">
            {logs.map((entry, i) => (
              <div
                key={`${entry.session_id}-${i}`}
                className="card card--bordered card__padding"
              >
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-sm font-medium text-[var(--foreground)]">
                    {entry.sender?.name || entry.sender?.id || "Unknown"}
                  </span>
                  <span className="text-xs text-[var(--muted-foreground)]">
                    {new Date(entry.received_at).toLocaleString()}
                  </span>
                </div>
                <p className="text-sm text-[var(--foreground)] truncate">{entry.message}</p>
                <div className="flex gap-3 mt-1.5 text-xs text-[var(--muted-foreground)]">
                  <span className="font-mono">{entry.session_id}</span>
                  {entry.thread_id && <span>Thread: {entry.thread_id}</span>}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
