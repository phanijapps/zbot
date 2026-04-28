// ============================================================================
// PERSISTENCE CARD
// Settings → Advanced section for opting into the SurrealDB 3.0 backend.
// Persists the choice via execution.featureFlags.surreal_backend (the
// existing free-form flag bag), which the daemon reads at startup to
// decide whether to wire SurrealDB into AppState.
// ============================================================================

import { useCallback, useEffect, useState } from "react";
import { Database, Loader2 } from "lucide-react";
import { getTransport, type ExecutionSettings } from "@/services/transport";

type Backend = "sqlite" | "surreal";

const FLAG_KEY = "surreal_backend";

function backendFromFlags(flags: Record<string, boolean> | undefined): Backend {
  return flags?.[FLAG_KEY] === true ? "surreal" : "sqlite";
}

export function PersistenceCard() {
  const [settings, setSettings] = useState<ExecutionSettings | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  // Initial load
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const transport = await getTransport();
      const res = await transport.getExecutionSettings();
      if (cancelled) return;
      if (res.success && res.data) {
        setSettings(res.data);
      } else {
        setError(res.error ?? "Failed to load execution settings");
      }
      setIsLoading(false);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const backend: Backend = backendFromFlags(settings?.featureFlags);

  const handleChange = useCallback(
    async (next: Backend) => {
      if (!settings) return;
      setIsSaving(true);
      setError(null);
      const updated: ExecutionSettings = {
        ...settings,
        featureFlags: {
          ...(settings.featureFlags ?? {}),
          [FLAG_KEY]: next === "surreal",
        },
      };
      const transport = await getTransport();
      const res = await transport.updateExecutionSettings(updated);
      if (res.success && res.data) {
        setSettings(res.data);
        setSavedAt(Date.now());
      } else {
        setError(res.error ?? "Failed to save backend choice");
      }
      setIsSaving(false);
    },
    [settings],
  );

  if (isLoading) {
    return (
      <div className="card card__padding--lg">
        <div className="flex items-center gap-3">
          <Loader2 className="loading-spinner__icon" />
          <span>Loading persistence settings…</span>
        </div>
      </div>
    );
  }

  return (
    <div className="card card__padding--lg">
      <div className="flex items-center gap-3" style={{ marginBottom: "var(--spacing-3)" }}>
        <div className="card__icon card__icon--primary">
          <Database style={{ width: 18, height: 18 }} />
        </div>
        <div>
          <h2 className="settings-section-header">Persistence</h2>
          <p className="page-subtitle">Knowledge graph + memory storage backend</p>
        </div>
      </div>

      {error ? (
        <div
          className="settings-alert settings-alert--error"
          style={{ marginBottom: "var(--spacing-3)" }}
        >
          {error}
        </div>
      ) : null}

      <div style={{ marginBottom: "var(--spacing-3)" }}>
        <div className="field-label">Knowledge Backend</div>
        <select
          className="form-input"
          value={backend}
          disabled={isSaving}
          onChange={(e) => handleChange(e.target.value as Backend)}
          aria-label="Knowledge backend"
        >
          <option value="sqlite">SQLite (default)</option>
          <option value="surreal">SurrealDB 3.0 (experimental)</option>
        </select>
      </div>

      {backend === "surreal" ? (
        <div
          className="settings-alert settings-alert--warning"
          style={{ marginBottom: "var(--spacing-3)" }}
          role="status"
        >
          <strong>SurrealDB selected — saved to settings.json.</strong> The
          switch only takes effect on daemon restart with the feature
          flag: stop the daemon and run{" "}
          <code>npm run daemon:surreal:watch</code> (or{" "}
          <code>cargo run -p daemon --features surreal-backend</code>).
          Mixed-mode: trait-routed paths use SurrealDB; legacy
          concrete-typed callers still hit SQLite until TD-023 retires
          them.
        </div>
      ) : (
        <div className="page-subtitle" style={{ marginBottom: "var(--spacing-3)" }}>
          SQLite is the default backend. Selecting SurrealDB persists
          the choice and prints the restart instructions.
        </div>
      )}

      <div className="page-subtitle">
        <strong>Recovery:</strong> on corruption, the daemon refuses to
        start. Recovery is a manual CLI step backed by{" "}
        <code>zero-stores-surreal-recovery</code>: read-only export to a
        JSON sidecar, then rename the corrupt directory aside.
      </div>

      {savedAt ? (
        <div
          className="page-subtitle"
          data-testid="persistence-saved-marker"
          style={{ marginTop: "var(--spacing-2)" }}
        >
          Saved.
        </div>
      ) : null}
    </div>
  );
}
