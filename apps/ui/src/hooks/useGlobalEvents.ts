/**
 * React hook for subscribing to global events.
 *
 * Global events include stats updates and session notifications
 * that are broadcast to all connected clients.
 */

import { useEffect, useRef } from "react";
import { getTransport } from "@/services/transport";
import type { GlobalEvent } from "@/services/transport/types";

/**
 * Subscribe to global events (stats updates, notifications).
 *
 * @param onEvent - Callback for received global events
 */
export function useGlobalEvents(onEvent: (event: GlobalEvent) => void) {
  const onEventRef = useRef(onEvent);

  useEffect(() => {
    onEventRef.current = onEvent;
  }, [onEvent]);

  useEffect(() => {
    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;
      unsubscribe = transport.onGlobalEvent((event) => onEventRef.current(event));
    };

    setup();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, []);
}
