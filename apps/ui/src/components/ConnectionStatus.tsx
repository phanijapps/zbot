/**
 * Connection status indicator component.
 *
 * Shows the current WebSocket connection state and provides
 * a reconnect button when disconnected.
 */

import { WifiOff, Loader2, AlertCircle } from "lucide-react";
import { useConnectionState } from "@/hooks/useConnectionState";
import { getTransport } from "@/services/transport";

export function ConnectionStatus() {
  const state = useConnectionState();

  const handleReconnect = async () => {
    const transport = await getTransport();
    transport.reconnect();
  };

  switch (state.status) {
    case "connected":
      return null;

    case "connecting":
      return (
        <div className="connection-status connection-status--connecting">
          <Loader2 className="connection-status__spinner" />
          <span className="connection-status__text">Connecting...</span>
        </div>
      );

    case "reconnecting":
      return (
        <div className="connection-status connection-status--connecting">
          <Loader2 className="connection-status__spinner" />
          <span className="connection-status__text">
            Reconnecting ({state.attempt}/{state.maxAttempts})...
          </span>
        </div>
      );

    case "disconnected":
      return (
        <div className="connection-status connection-status--disconnected">
          <WifiOff style={{ width: 16, height: 16 }} />
          <span className="connection-status__text">Disconnected</span>
          <button onClick={handleReconnect} className="connection-status__action">
            Reconnect
          </button>
        </div>
      );

    case "failed":
      return (
        <div className="connection-status connection-status--failed">
          <AlertCircle style={{ width: 16, height: 16 }} />
          <span className="connection-status__text">Connection failed</span>
          <button onClick={handleReconnect} className="connection-status__action">
            Retry
          </button>
        </div>
      );
  }
}
