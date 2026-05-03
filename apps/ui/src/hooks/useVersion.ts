// ============================================================================
// USE VERSION HOOK
// Fetches the running daemon's version once per page load via /api/health.
// Module-level memoization mirrors the usePaths pattern — single network
// call regardless of how many components subscribe.
// ============================================================================

import { useEffect, useState } from "react";

interface HealthResponse {
  status: string;
  version: string;
}

let cached: string | null = null;
let pending: Promise<string | null> | null = null;

async function fetchVersion(): Promise<string | null> {
  try {
    const response = await fetch("/api/health");
    if (!response.ok) return null;
    const body = (await response.json()) as HealthResponse;
    return typeof body.version === "string" && body.version.length > 0
      ? body.version
      : null;
  } catch {
    return null;
  }
}

/**
 * Returns the daemon's reported version string (e.g. `2026.5.3` or
 * `2026.5.3.develop` for branch-suffixed installs), or `null` while
 * loading or on fetch failure. Memoized at module scope: only one
 * network call per page load.
 */
export function useVersion(): string | null {
  const [version, setVersion] = useState<string | null>(cached);

  useEffect(() => {
    if (cached) return;
    if (!pending) {
      pending = fetchVersion().then((v) => {
        cached = v;
        return v;
      });
    }
    let cancelled = false;
    void pending.then((v) => {
      if (!cancelled) setVersion(v);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return version;
}

/** Test-only: clears the module-level cache so each test starts cold. */
export function __resetVersionCacheForTest(): void {
  cached = null;
  pending = null;
}
