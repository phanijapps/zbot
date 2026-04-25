// ============================================================================
// WEB ENTRY POINT
// Entry point for standalone web dashboard (no Tauri)
// ============================================================================

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";

// Configure gateway URL from window config or URL params.
// Default behaviour (no params) is same-origin — the transport layer
// builds httpUrl="" + wsUrl="ws(s)://<page-host>/ws" automatically.
// Only set __ZERO_CONFIG__ when the user explicitly overrides via query
// params, in which case the missing side falls back to same-origin too.
const configureGateway = () => {
  const params = new URLSearchParams(window.location.search);
  const httpUrl = params.get("gateway_http");
  const wsUrl = params.get("gateway_ws");

  if (httpUrl || wsUrl) {
    const wsProto = window.location.protocol === "https:" ? "wss" : "ws";
    (window as { __ZERO_CONFIG__?: { httpUrl: string; wsUrl: string } }).__ZERO_CONFIG__ = {
      httpUrl: httpUrl ?? "",
      wsUrl: wsUrl ?? `${wsProto}://${window.location.host}/ws`,
    };
  }
};

configureGateway();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
