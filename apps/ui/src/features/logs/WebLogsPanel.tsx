// ============================================================================
// WEB LOGS PANEL
// Redirects to the Execution Intelligence Dashboard.
// Keeps the WebLogsPanel export name so existing imports (App.tsx etc.) work.
// ============================================================================

import { ExecutionDashboard } from "./ExecutionDashboard";

export function WebLogsPanel() {
  return <ExecutionDashboard />;
}
