import { useCallback, useEffect, useRef, useState } from "react";
import { getTransport } from "@/services/transport";
import type { SessionDetail } from "@/services/transport/types";

export interface DetailBundle {
  /** The root session detail. Set when sessionId is non-null and load succeeded. */
  root: SessionDetail | null;
  /** Detail records for each direct child session. */
  children: SessionDetail[];
}

export function useSessionDetailBundle(sessionId: string | null, isRunning: boolean) {
  const [bundle, setBundle] = useState<DetailBundle>({ root: null, children: [] });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const loadInFlightRef = useRef<Promise<DetailBundle> | null>(null);
  const loadingKeyRef = useRef<string | null>(null);
  const refetch = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (!sessionId) {
      setBundle({ root: null, children: [] });
      setLoading(false);
      return;
    }

    let cancelled = false;
    const loadKey = `${sessionId}:${tick}`;
    const load = async () => {
      if (loadInFlightRef.current && loadingKeyRef.current === loadKey) {
        return loadInFlightRef.current;
      }

      loadingKeyRef.current = loadKey;
      const loadPromise = (async () => {
        const transport = await getTransport();
        const rootResult = await transport.getLogSession(sessionId);
        if (!rootResult.success || !rootResult.data) {
          throw new Error(rootResult.error ?? "failed");
        }

        const root = rootResult.data;
        const childIds = root.session.child_session_ids ?? [];
        const childResults = await Promise.all(childIds.map((id) => transport.getLogSession(id)));

        const children: SessionDetail[] = [];
        for (const cr of childResults) {
          if (cr.success && cr.data) children.push(cr.data);
        }

        return { root, children };
      })();

      loadInFlightRef.current = loadPromise;
      const clearInFlight = () => {
        if (loadingKeyRef.current === loadKey) {
          loadingKeyRef.current = null;
          loadInFlightRef.current = null;
        }
      };
      loadPromise.then(clearInFlight, clearInFlight);
      return loadPromise;
    };

    setLoading(true);
    load()
      .then((nextBundle) => {
        if (cancelled) return;
        setBundle(nextBundle);
        setError(null);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId, tick]);

  useEffect(() => {
    if (!sessionId || !isRunning) return;
    const id = setInterval(() => setTick((t) => t + 1), 2000);
    return () => clearInterval(id);
  }, [sessionId, isRunning]);

  return { bundle, loading, error, refetch };
}
