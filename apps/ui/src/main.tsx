// ============================================================================
// WEB ENTRY POINT
// Entry point for standalone web dashboard (no Tauri)
// ============================================================================

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";

// Configure gateway URL from window config or URL params
const configureGateway = () => {
  const params = new URLSearchParams(window.location.search);
  const httpUrl = params.get("gateway_http");
  const wsUrl = params.get("gateway_ws");

  if (httpUrl || wsUrl) {
    (window as { __ZERO_CONFIG__?: { httpUrl: string; wsUrl: string } }).__ZERO_CONFIG__ = {
      httpUrl: httpUrl || "http://localhost:18791",
      // Unified-port default: WebSocket upgrade lives at /ws on the HTTP
      // port. Phones and reverse proxies no longer need a second port
      // open. Override via ?gateway_ws=... if you're running
      // --legacy-ws-port-enabled on the daemon.
      wsUrl: wsUrl || "ws://localhost:18791/ws",
    };
  }
};

configureGateway();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
