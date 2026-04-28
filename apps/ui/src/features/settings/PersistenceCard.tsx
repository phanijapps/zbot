// ============================================================================
// PERSISTENCE CARD
// Settings → Advanced section announcing the SurrealDB 3.0 backend.
// The crate (zero-stores-surreal) is built behind the `surreal-backend`
// Cargo feature; backend selection from settings.json is a follow-up PR.
// ============================================================================

import { Database } from "lucide-react";

export function PersistenceCard() {
  return (
    <div className="settings-card">
      <div className="settings-card__header">
        <Database className="settings-card__icon" aria-hidden="true" />
        <div>
          <h3 className="settings-card__title">Persistence</h3>
          <p className="settings-card__subtitle">
            Knowledge graph + memory storage backend
          </p>
        </div>
      </div>

      <div className="settings-card__body">
        <div className="settings-row">
          <div className="settings-row__label">
            <strong>Knowledge Backend</strong>
            <p className="settings-row__hint">
              SQLite is the default and the only backend wired into the
              running daemon today. SurrealDB 3.0 is available as an
              opt-in build (Mode A: embedded RocksDB) for testing.
            </p>
          </div>
          <div className="settings-row__control">
            <select
              id="knowledge-backend-readonly"
              value="sqlite"
              disabled
              aria-label="Knowledge backend (read-only — feature-gated)"
            >
              <option value="sqlite">SQLite (default)</option>
              <option value="surreal" disabled>
                SurrealDB (build with --features surreal-backend)
              </option>
            </select>
          </div>
        </div>

        <div className="settings-banner settings-banner--info">
          <strong>SurrealDB backend is experimental.</strong> Build the
          daemon with <code>cargo run -p daemon --features surreal-backend</code>{" "}
          to opt in. The factory plumbing in{" "}
          <code>gateway/src/state/persistence_factory.rs::build_surreal_pair</code>{" "}
          is ready; settings.json driven dispatch lands in a follow-up PR.
        </div>

        <div className="settings-row">
          <div className="settings-row__label">
            <strong>Recovery</strong>
            <p className="settings-row__hint">
              On corruption, the daemon refuses to start. Recovery is a
              manual CLI step backed by{" "}
              <code>zero-stores-surreal-recovery</code>: read-only export
              to a JSON sidecar, then rename the corrupt directory aside.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
