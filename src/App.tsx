// ============================================================================
// APP ENTRY POINT
// Main application with routing configuration
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "./core";
import { VaultSelector, VaultSwitchingLoader, useVaults } from "./features/vaults";
import type { Vault } from "@/shared/types";

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
  const [vault, setVault] = useState<Vault | null>(null);
  const [isCheckingVault, setIsCheckingVault] = useState(true);
  const { isSwitchingVault } = useVaults(); // Get vault switching state

  useEffect(() => {
    checkVaultStatus();
  }, []);

  const checkVaultStatus = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const status = await invoke<{
        registryExists: boolean;
        hasVaults: boolean;
        hasActiveVault: boolean;
        activeVault: Vault | null;
        vaults: Vault[];
      }>("get_vault_status");

      // If there's a valid active vault, use it
      if (status.hasActiveVault && status.activeVault) {
        setVault(status.activeVault);
      }
    } catch (error) {
      console.error("Failed to check vault status:", error);
    } finally {
      setIsCheckingVault(false);
    }
  };

  const handleVaultSelected = (selectedVault: Vault) => {
    setVault(selectedVault);
  };

  // Show loading while checking vault status
  if (isCheckingVault) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#1a1a1a]">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-violet-500 mb-4"></div>
          <p className="text-gray-400">Loading...</p>
        </div>
      </div>
    );
  }

  // Show vault selector if no vault is selected
  if (!vault) {
    return <VaultSelector onVaultSelected={handleVaultSelected} />;
  }

  // Show main app when vault is ready
  return (
    <>
      <VaultSwitchingLoader show={isSwitchingVault} />
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
    </>
  );
}

export default App;
