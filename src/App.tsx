// ============================================================================
// APP ENTRY POINT
// Main application with routing configuration
// ============================================================================

import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "./core";

import {
  AgentChannelPanel,
  AgentsPanel,
  ProvidersPanel,
  MCPServersPanel,
  SkillsPanel,
  SettingsPanel,
  SearchPanel,
} from "./features";

function App() {
  return (
    <BrowserRouter>
      <AppShell>
        <Routes>
          <Route path="/" element={<AgentChannelPanel />} />
          <Route path="/agents" element={<AgentsPanel />} />
          <Route path="/providers" element={<ProvidersPanel />} />
          <Route path="/mcp" element={<MCPServersPanel />} />
          <Route path="/skills" element={<SkillsPanel />} />
          <Route path="/settings" element={<SettingsPanel />} />
          <Route path="/search" element={<SearchPanel />} />
        </Routes>
      </AppShell>
    </BrowserRouter>
  );
}

export default App;
