/**
 * Connection status indicator component.
 *
 * Shows the current WebSocket connection state and provides
 * a reconnect button when disconnected.
 */

import { WifiOff, Loader2, AlertCircle } from "lucide-react";
import { useConnectionState } from "@/hooks/useConnectionState";
import { getTransport } from "@/services/transport";

/**
 * Displays the current WebSocket connection status.
 *
 * - Connected: Hidden (no indicator needed)
 * - Connecting: Yellow spinner with "Connecting..."
 * - Reconnecting: Yellow spinner with attempt count
 * - Disconnected: Gray with reconnect button
 * - Failed: Red with retry button
 */
export function ConnectionStatus() {
  const state = useConnectionState();

  const handleReconnect = async () => {
    const transport = await getTransport();
    transport.reconnect();
  };

  switch (state.status) {
    case "connected":
      // Don't show anything when connected
      return null;

    case "connecting":
      return (
        <div className="flex items-center gap-2 text-yellow-600 text-sm px-3 py-1.5 bg-yellow-50 rounded-lg">
          <Loader2 className="w-4 h-4 animate-spin" />
          Connecting...
        </div>
      );

    case "reconnecting":
      return (
        <div className="flex items-center gap-2 text-yellow-600 text-sm px-3 py-1.5 bg-yellow-50 rounded-lg">
          <Loader2 className="w-4 h-4 animate-spin" />
          Reconnecting ({state.attempt}/{state.maxAttempts})...
        </div>
      );

    case "disconnected":
      return (
        <div className="flex items-center gap-2 text-gray-500 text-sm px-3 py-1.5 bg-gray-100 rounded-lg">
          <WifiOff className="w-4 h-4" />
          Disconnected
          <button
            onClick={handleReconnect}
            className="underline ml-1 hover:text-gray-700"
          >
            Reconnect
          </button>
        </div>
      );

    case "failed":
      return (
        <div className="flex items-center gap-2 text-red-600 text-sm px-3 py-1.5 bg-red-50 rounded-lg">
          <AlertCircle className="w-4 h-4" />
          Connection failed
          <button
            onClick={handleReconnect}
            className="underline ml-1 hover:text-red-700"
          >
            Retry
          </button>
        </div>
      );
  }
}
