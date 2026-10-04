import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";
import { useEffect, useRef, useState } from "react";

/**
 * Session-bound artifact manifest reader for the Files inspector tab. Reads
 * once per selected session, again when an active run settles (final
 * artifacts), and on manual retry. Late responses for a previously selected
 * session are discarded.
 */
export function useSessionArtifacts(sessionId: string | undefined, active: boolean) {
  const [artifacts, setArtifacts] = useState<Artifact[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);
  const [retry, setRetry] = useState(0);
  const [settledTick, setSettledTick] = useState(0);
  const wasActive = useRef(false);

  useEffect(() => {
    if (wasActive.current && !active) setSettledTick(tick => tick + 1);
    wasActive.current = active;
  }, [active]);

  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;
    setLoading(true); setError(false);
    void (async () => {
      try {
        const transport = await getTransport();
        const response = await transport.listSessionArtifacts(sessionId);
        if (cancelled) return;
        if (!response.success || !response.data) throw new Error("Unavailable");
        setArtifacts(response.data.filter(artifact => artifact.sessionId === sessionId));
        setError(false);
      } catch {
        if (!cancelled) setError(true);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {cancelled = true;};
  }, [sessionId, retry, settledTick]);

  return {artifacts, loading, error, retry: () => setRetry(value => value + 1)};
}
