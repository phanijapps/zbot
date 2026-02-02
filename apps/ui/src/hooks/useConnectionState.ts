/**
 * React hook for tracking WebSocket connection state.
 *
 * Use this to show connection status indicators in the UI.
 */

import { useState, useEffect } from "react";
import { getTransport } from "@/services/transport";
import type { ConnectionState } from "@/services/transport/types";

/**
 * Get the current WebSocket connection state.
 *
 * @returns The current connection state
 */
export function useConnectionState(): ConnectionState {
  const [state, setState] = useState<ConnectionState>({ status: "disconnected" });

  useEffect(() => {
    let unsubscribe: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      const transport = await getTransport();
      if (cancelled) return;
      unsubscribe = transport.onConnectionStateChange(setState);
    };

    setup();

    return () => {
      cancelled = true;
      if (unsubscribe) unsubscribe();
    };
  }, []);

  return state;
}
