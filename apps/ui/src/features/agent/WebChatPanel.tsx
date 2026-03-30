// ============================================================================
// WEB CHAT PANEL
// Replaced by Mission Control — full-page execution theater
// Keeps the named export so parent imports (App.tsx, ChatSlider) continue working.
// ============================================================================

import { MissionControl } from "../chat/MissionControl";

export function WebChatPanel() {
  return <MissionControl />;
}
