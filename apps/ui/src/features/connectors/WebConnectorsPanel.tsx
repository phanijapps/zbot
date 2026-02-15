// ============================================================================
// WORKERS PANEL
// Read-only view of connected bridge workers
// ============================================================================

import { useState, useEffect, useCallback, useRef } from "react";
import { Cable, Loader2, X, Cpu, Wrench, Database } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { BridgeWorker } from "@/services/transport/types";

const POLL_INTERVAL_MS = 5000;

export function WebConnectorsPanel() {
  const [workers, setWorkers] = useState<BridgeWorker[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // ─────────────────────────────────────────────────────────────────────────
  // Data Loading
  // ─────────────────────────────────────────────────────────────────────────

  const loadWorkers = useCallback(async () => {
    try {
      const transport = await getTransport();
      const result = await transport.listBridgeWorkers();
      if (result.success && result.data) {
        setWorkers(result.data);
        setError(null);
      } else {
        setError(result.error || "Failed to load workers");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadWorkers();
    intervalRef.current = setInterval(loadWorkers, POLL_INTERVAL_MS);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [loadWorkers]);

  const selectedWorker = workers.find((w) => w.adapter_id === selectedId) ?? null;

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
        {/* Left sidebar - Worker list */}
        <div className="split-panel__sidebar">
          <div className="page-header" style={{ padding: "var(--spacing-4)", marginBottom: 0 }}>
            <div>
              <h2 className="page-title" style={{ fontSize: "var(--text-lg)" }}>Workers</h2>
              <p className="page-subtitle">Connected bridge workers</p>
            </div>
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
            {workers.length === 0 ? (
              <div className="empty-state">
                <div className="empty-state__icon">
                  <Cable className="w-5 h-5" />
                </div>
                <p className="empty-state__title">No workers connected</p>
                <p className="empty-state__description">
                  Workers connect via WebSocket at <code className="badge" style={{ fontSize: "var(--text-xs)" }}>/bridge/ws</code>
                </p>
              </div>
            ) : (
              <div className="space-y-1.5">
                {workers.map((worker) => (
                  <button
                    key={worker.adapter_id}
                    onClick={() => setSelectedId(worker.adapter_id)}
                    className={`w-full text-left p-3 rounded-lg transition-all ${
                      selectedId === worker.adapter_id
                        ? "bg-[var(--primary)]/10 border border-[var(--primary)]/30"
                        : "bg-[var(--card)] hover:bg-[var(--muted)] border border-transparent"
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <div className="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 bg-[var(--primary-muted)]">
                        <Cpu className="w-4 h-4 text-[var(--primary)]" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="list-item__title truncate">
                          {worker.adapter_id}
                        </div>
                        <div className="list-item__subtitle">
                          {worker.capabilities.length} capabilities, {worker.resources.length} resources
                        </div>
                      </div>
                      <div className="w-2 h-2 rounded-full flex-shrink-0 bg-[var(--success)]" />
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Right panel - Worker detail */}
        <div className="split-panel__content">
          {selectedWorker ? (
            <WorkerDetail worker={selectedWorker} />
          ) : (
            <div className="split-panel__empty">
              <div className="empty-state">
                <div className="empty-state__icon">
                  <Cable className="w-6 h-6" />
                </div>
                <p className="empty-state__title">Select a worker</p>
                <p className="empty-state__description">Choose a worker from the sidebar to view details</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Worker Detail
// ============================================================================

function WorkerDetail({ worker }: { worker: BridgeWorker }) {
  const connectedAt = new Date(worker.connected_at);
  const uptime = formatUptime(connectedAt);

  return (
    <div style={{ padding: "var(--spacing-6)" }} className="h-full overflow-auto">
      {/* Header */}
      <div className="flex items-start justify-between mb-6">
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl flex items-center justify-center bg-[var(--primary-muted)]">
            <Cpu className="w-6 h-6 text-[var(--primary)]" />
          </div>
          <div>
            <h3 className="text-xl font-semibold text-[var(--foreground)]">
              {worker.adapter_id}
            </h3>
            <p className="text-sm text-[var(--muted-foreground)]">
              Connected {uptime}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <span className="badge" style={{
            backgroundColor: "var(--success-muted)",
            color: "var(--success)",
            fontSize: "var(--text-xs)",
            fontWeight: 600,
          }}>
            Connected
          </span>
        </div>
      </div>

      {/* Capabilities */}
      <div className="mb-6">
        <div className="flex items-center gap-2 mb-3">
          <Wrench style={{ width: 16, height: 16, color: "var(--muted-foreground)" }} />
          <h4 style={{ fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)" }}>
            Capabilities ({worker.capabilities.length})
          </h4>
        </div>
        {worker.capabilities.length === 0 ? (
          <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)", paddingLeft: "var(--spacing-6)" }}>
            No capabilities declared
          </p>
        ) : (
          <div className="flex flex-col gap-2">
            {worker.capabilities.map((cap) => (
              <div
                key={cap.name}
                className="card"
                style={{ padding: "var(--spacing-3)" }}
              >
                <div style={{ fontSize: "var(--text-sm)", fontWeight: 500, color: "var(--foreground)" }}>
                  {cap.name}
                </div>
                {cap.description && (
                  <div style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)", marginTop: "var(--spacing-1)" }}>
                    {cap.description}
                  </div>
                )}
                {cap.schema && (
                  <pre style={{
                    fontSize: "var(--text-xs)",
                    color: "var(--muted-foreground)",
                    marginTop: "var(--spacing-2)",
                    padding: "var(--spacing-2)",
                    backgroundColor: "var(--muted)",
                    borderRadius: "var(--radius-sm)",
                    overflow: "auto",
                    maxHeight: "120px",
                  }}>
                    {JSON.stringify(cap.schema, null, 2)}
                  </pre>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Resources */}
      <div>
        <div className="flex items-center gap-2 mb-3">
          <Database style={{ width: 16, height: 16, color: "var(--muted-foreground)" }} />
          <h4 style={{ fontSize: "var(--text-sm)", fontWeight: 600, color: "var(--foreground)" }}>
            Resources ({worker.resources.length})
          </h4>
        </div>
        {worker.resources.length === 0 ? (
          <p style={{ fontSize: "var(--text-sm)", color: "var(--muted-foreground)", paddingLeft: "var(--spacing-6)" }}>
            No resources declared
          </p>
        ) : (
          <div className="flex flex-col gap-2">
            {worker.resources.map((res) => (
              <div
                key={res.name}
                className="card"
                style={{ padding: "var(--spacing-3)" }}
              >
                <div style={{ fontSize: "var(--text-sm)", fontWeight: 500, color: "var(--foreground)" }}>
                  {res.name}
                </div>
                {res.description && (
                  <div style={{ fontSize: "var(--text-xs)", color: "var(--muted-foreground)", marginTop: "var(--spacing-1)" }}>
                    {res.description}
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

// ============================================================================
// Helpers
// ============================================================================

function formatUptime(connectedAt: Date): string {
  const diffMs = Date.now() - connectedAt.getTime();
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 60) return `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ${diffMin % 60}m ago`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay}d ${diffHr % 24}h ago`;
}
