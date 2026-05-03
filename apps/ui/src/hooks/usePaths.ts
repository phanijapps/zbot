// ============================================================================
// USE PATHS HOOK
// Fetches the daemon's vault paths once per page load. The daemon resolves
// `<vault>` at startup based on host environment (`~/Documents/zbot/` on a
// desktop with Documents, `~/zbot/` on a Pi without it), so the UI must ask
// rather than hardcode.
// ============================================================================

import { useEffect, useState } from "react";

export interface Paths {
  vaultDir: string;
  configDir: string;
  logsDir: string;
  pluginsDir: string;
  agentsDir: string;
  vaultDirDisplay: string;
  configDirDisplay: string;
  logsDirDisplay: string;
  pluginsDirDisplay: string;
}

let cached: Paths | null = null;
let pending: Promise<Paths | null> | null = null;

async function fetchPaths(): Promise<Paths | null> {
  try {
    const response = await fetch("/api/paths");
    if (!response.ok) return null;
    return (await response.json()) as Paths;
  } catch {
    return null;
  }
}

/**
 * Returns the daemon's vault paths, or `null` while loading or on fetch
 * failure. Memoized at module scope: only one network call per page load.
 */
export function usePaths(): Paths | null {
  const [paths, setPaths] = useState<Paths | null>(cached);

  useEffect(() => {
    if (cached) return;
    if (!pending) {
      pending = fetchPaths().then((p) => {
        cached = p;
        return p;
      });
    }
    let cancelled = false;
    void pending.then((p) => {
      if (!cancelled) setPaths(p);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return paths;
}

/** Test-only: clears the module-level cache so each test starts cold. */
export function __resetPathsCacheForTest(): void {
  cached = null;
  pending = null;
}
