// ============================================================================
// PERSISTENCE CARD
// Settings → Advanced section announcing the SurrealDB 3.0 backend.
// The crate (zero-stores-surreal) is built behind the `surreal-backend`
// Cargo feature; backend selection from settings.json is a follow-up PR.
// ============================================================================

import { useState } from "react";
import { Database } from "lucide-react";

type Backend = "sqlite" | "surreal";

export function PersistenceCard() {
  const [backend, setBackend] = useState<Backend>("sqlite");

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

      <div style={{ marginBottom: "var(--spacing-3)" }}>
        <div className="field-label">Knowledge Backend</div>
        <select
          className="form-input"
          value={backend}
          onChange={(e) => setBackend(e.target.value as Backend)}
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
          <strong>To switch to SurrealDB:</strong>
          <ol style={{ marginTop: 6, marginBottom: 0, paddingLeft: 20 }}>
            <li>
              Build with the feature flag:{" "}
              <code>cargo run -p daemon --features surreal-backend</code>
            </li>
            <li>
              Set{" "}
              <code>{`{"persistence": {"knowledge_backend": "surreal"}}`}</code>{" "}
              in <code>config/settings.json</code>
            </li>
            <li>Restart the daemon</li>
          </ol>
          <div style={{ marginTop: 6 }}>
            Mixed-mode: trait-routed paths use SurrealDB; legacy concrete-typed
            callers (a few HTTP handlers + sleep jobs) still hit SQLite until
            the TD-023 follow-up retires them.
          </div>
        </div>
      ) : (
        <div className="page-subtitle" style={{ marginBottom: "var(--spacing-3)" }}>
          SQLite is the default backend. Selecting SurrealDB shows the
          steps to opt in.
        </div>
      )}

      <div className="page-subtitle">
        <strong>Recovery:</strong> on corruption, the daemon refuses to
        start. Recovery is a manual CLI step backed by{" "}
        <code>zero-stores-surreal-recovery</code>: read-only export to a
        JSON sidecar, then rename the corrupt directory aside.
      </div>
    </div>
  );
}
