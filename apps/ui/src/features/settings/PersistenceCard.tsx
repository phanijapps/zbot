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
          <strong>SurrealDB requires a daemon rebuild.</strong> Stop the
          daemon and restart it with{" "}
          <code>cargo run -p daemon --features surreal-backend</code>. The
          factory dispatch in{" "}
          <code>persistence_factory::build_surreal_pair</code> is wired;
          settings.json driven runtime switching lands in a follow-up PR.
        </div>
      ) : (
        <div className="page-subtitle" style={{ marginBottom: "var(--spacing-3)" }}>
          SQLite is the only backend wired into the running daemon today.
          Selecting SurrealDB shows the build instructions.
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
