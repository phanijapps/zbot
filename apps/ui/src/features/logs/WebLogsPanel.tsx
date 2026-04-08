// ============================================================================
// WEB LOGS PANEL
// Redirects to the Observability Dashboard.
// Keeps the WebLogsPanel export name so existing imports (App.tsx etc.) work.
// ============================================================================

import { ObservabilityDashboard } from "./ObservabilityDashboard";

export function WebLogsPanel() {
  return <ObservabilityDashboard />;
}
