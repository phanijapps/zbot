// ============================================================================
// useFeatureFlag hook
// Reads feature flags from execution settings (featureFlags map).
// Defaults to `false` when the flag is absent or settings can't be loaded.
// ============================================================================

import { useEffect, useState } from "react";
import { getTransport } from "@/services/transport";

export function useFeatureFlag(name: string): boolean {
  const [on, setOn] = useState(false);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const transport = await getTransport();
        const result = await transport.getExecutionSettings();
        if (!alive) return;
        if (result.success && result.data) {
          const flags = result.data.featureFlags;
          setOn(Boolean(flags?.[name]));
        }
      } catch {
        // Leave flag as false on transport failure.
      }
    })();
    return () => {
      alive = false;
    };
  }, [name]);

  return on;
}
