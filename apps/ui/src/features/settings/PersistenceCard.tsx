// ============================================================================
// PERSISTENCE CARD
// Settings → Advanced section announcing the SurrealDB 3.0 backend.
// The crate (zero-stores-surreal) is built behind the `surreal-backend`
// Cargo feature; backend selection from settings.json is a follow-up PR.
// ============================================================================

import { Database } from "lucide-react";

export function PersistenceCard() {
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
        <label className="settings-field-label" htmlFor="persistence-backend">
          Knowledge Backend
        </label>
        <select
          id="persistence-backend"
          className="form-input"
          value="sqlite"
          disabled
          aria-label="Knowledge backend (read-only — feature-gated)"
          style={{ marginTop: 4 }}
        >
          <option value="sqlite">SQLite (default)</option>
          <option value="surreal" disabled>
            SurrealDB (build with --features surreal-backend)
          </option>
        </select>
        <div className="page-subtitle" style={{ marginTop: 4 }}>
          SQLite is the only backend wired into the running daemon today.
          SurrealDB 3.0 is available as an opt-in build (Mode A: embedded
          RocksDB) for testing.
        </div>
      </div>

      <div
        className="settings-alert settings-alert--warning"
        style={{ marginBottom: "var(--spacing-3)" }}
      >
        <strong>SurrealDB backend is experimental.</strong> Build the daemon
        with <code>cargo run -p daemon --features surreal-backend</code> to
        opt in. The factory plumbing in{" "}
        <code>persistence_factory::build_surreal_pair</code> is ready;
        settings.json driven dispatch lands in a follow-up PR.
      </div>

      <div className="page-subtitle">
        <strong>Recovery:</strong> on corruption, the daemon refuses to
        start. Recovery is a manual CLI step backed by{" "}
        <code>zero-stores-surreal-recovery</code>: read-only export to a
        JSON sidecar, then rename the corrupt directory aside.
      </div>
    </div>
  );
}
