// ============================================================================
// useFeatureFlag hook
// Reads feature flags from execution settings (featureFlags map).
// Returns `defaultValue` when the flag is absent or settings can't be loaded.
// Pass `defaultValue=true` for flags that have rolled out — the legacy fallback
// only kicks in when the user has explicitly opted out.
// ============================================================================

import { useEffect, useState } from "react";
import { getTransport } from "@/services/transport";

export function useFeatureFlag(name: string, defaultValue = false): boolean {
  const [on, setOn] = useState(defaultValue);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const transport = await getTransport();
        const result = await transport.getExecutionSettings();
        if (!alive) return;
        if (result.success && result.data) {
          const flags = result.data.featureFlags;
          const explicit = flags?.[name];
          setOn(explicit === undefined ? defaultValue : explicit);
        }
      } catch {
        // Leave flag at defaultValue on transport failure.
      }
    })();
    return () => {
      alive = false;
    };
  }, [name, defaultValue]);

  return on;
}
