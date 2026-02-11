// ============================================================================
// OVERVIEW TAB
// Status badges, timestamps, test connection
// ============================================================================

import { useState } from "react";
import { Play, Loader2, Check, X } from "lucide-react";
import { getTransport } from "@/services/transport";
import type { ConnectorResponse, ConnectorTestResult } from "@/services/transport/types";

interface OverviewTabProps {
  connector: ConnectorResponse;
}

export function OverviewTab({ connector }: OverviewTabProps) {
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<ConnectorTestResult | null>(null);

  const handleTest = async () => {
    setIsTesting(true);
    setTestResult(null);

    try {
      const transport = await getTransport();
      const result = await transport.testConnector(connector.id);
      if (result.success && result.data) {
        setTestResult(result.data);
      } else {
        setTestResult({ success: false, message: result.error || "Test failed" });
      }
    } catch (err) {
      setTestResult({
        success: false,
        message: err instanceof Error ? err.message : "Unknown error",
      });
    } finally {
      setIsTesting(false);
    }
  };

  return (
    <div className="space-y-5">
      {/* Status badges */}
      <div className="flex flex-wrap gap-2">
        <span
          className={`badge ${
            connector.enabled ? "badge--success" : ""
          }`}
        >
          {connector.enabled ? "Enabled" : "Disabled"}
        </span>
        <span
          className={`badge ${
            connector.outbound_enabled ? "badge--primary" : ""
          }`}
        >
          {connector.outbound_enabled ? "Outbound On" : "Outbound Off"}
        </span>
        <span
          className={`badge ${
            connector.inbound_enabled ? "badge--primary" : ""
          }`}
        >
          {connector.inbound_enabled ? "Inbound On" : "Inbound Off"}
        </span>
        <span className="badge badge--warning">
          {connector.transport.type.toUpperCase()}
        </span>
      </div>

      {/* Timestamps */}
      {(connector.created_at || connector.updated_at) && (
        <div className="card card--bordered card__padding">
          <h4 className="text-sm font-medium text-[var(--foreground)] mb-3">Timestamps</h4>
          <div className="grid grid-cols-2 gap-4">
            {connector.created_at && (
              <div>
                <span className="text-xs text-[var(--muted-foreground)]">Created</span>
                <p className="text-sm text-[var(--foreground)] mt-0.5">
                  {new Date(connector.created_at).toLocaleString()}
                </p>
              </div>
            )}
            {connector.updated_at && (
              <div>
                <span className="text-xs text-[var(--muted-foreground)]">Updated</span>
                <p className="text-sm text-[var(--foreground)] mt-0.5">
                  {new Date(connector.updated_at).toLocaleString()}
                </p>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Test connection */}
      <div className="card card--bordered card__padding">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h4 className="text-sm font-medium text-[var(--foreground)]">Test Connection</h4>
            <p className="text-xs text-[var(--muted-foreground)] mt-0.5">
              Verify the transport endpoint is reachable
            </p>
          </div>
          <button
            onClick={handleTest}
            disabled={isTesting}
            className="btn btn--primary btn--sm"
          >
            {isTesting ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Play className="w-3.5 h-3.5" />
            )}
            Test
          </button>
        </div>
        {testResult && (
          <div
            className={`alert ${
              testResult.success ? "alert--success" : "alert--error"
            }`}
          >
            {testResult.success ? (
              <Check className="alert__icon" />
            ) : (
              <X className="alert__icon" />
            )}
            <span>{testResult.message}</span>
          </div>
        )}
      </div>
    </div>
  );
}
