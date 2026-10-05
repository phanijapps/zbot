import { useEffect, useState } from "react";
import { getTransport } from "@/services/transport";
import type { SessionDetails } from "@/services/transport/types";

/**
 * Session-bound details reader shared by the Activity and Sources inspector
 * tabs. Every response is tied to its initiating session ID: a response for
 * another session is discarded, and a late response for a previously selected
 * session never renders. While `active` (or a hook is running, or within the
 * short mount window) the details are re-read on a 1s cadence so Stop and
 * cleanup outcomes land; idle sessions read once.
 */
export function useSessionDetails(sessionId: string | undefined, active: boolean) {
  const [cached, setCached] = useState<SessionDetails | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);
  const [retry, setRetry] = useState(0);
  const details = cached && cached.sessionId === sessionId ? cached : null;

  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const cleanupUntil = Date.now() + 5000;
    setLoading(true); setError(false);
    const load = async () => {
      try {
        const transport = await getTransport();
        const response = await transport.getSessionDetails(sessionId);
        if (cancelled) return;
        if (!response.success || !response.data || response.data.sessionId !== sessionId) throw new Error("Unavailable");
        setCached(response.data); setError(false);
        const running = response.data.activity.some(row => row.kind === "hook" && row.hook?.status === "running");
        if (active || running || Date.now() < cleanupUntil) timer = setTimeout(() => void load(), 1000);
      } catch {
        if (!cancelled) setError(true);
        // A transient failure during an active run must not freeze the
        // inspector: keep the 1s cadence (with the error notice) while the
        // session is active so Stop/cleanup outcomes still land.
        if (active || Date.now() < cleanupUntil) timer = setTimeout(() => void load(), 1000);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    return () => {cancelled = true; clearTimeout(timer);};
  }, [sessionId, active, retry]);

  return {details, loading, error, retry: () => setRetry(value => value + 1)};
}
