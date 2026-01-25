// ============================================================================
// APP ENTRY POINT
// Main application with routing configuration
// ============================================================================

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "./core";
import { VaultSelector, VaultSwitchingLoader, useVaults } from "./features/vaults";
import { initializeVaultSystem } from "@/services/vaults";
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
import { WorkflowIDEPage } from "./features/workflow-ide";

function App() {
  const [vault, setVault] = useState<Vault | null>(null);
  const [isCheckingVault, setIsCheckingVault] = useState(true);
  const { isSwitchingVault } = useVaults(); // Get vault switching state

  // Apply dark theme to document - this ensures CSS variables work correctly
  useEffect(() => {
    document.body.classList.add('dark');
    // Also set data-theme attribute for completeness
    document.documentElement.setAttribute('data-theme', 'dark');
  }, []);

  useEffect(() => {
    checkVaultStatus();
  }, []);

  const checkVaultStatus = async () => {
    try {
      // First, initialize the vault system (this sets the active vault path in Rust backend)
      const initializedVault = await initializeVaultSystem();

      // Then check the vault status to get all available vaults
      const { invoke } = await import("@tauri-apps/api/core");
      const status = await invoke<{
        registryExists: boolean;
        hasVaults: boolean;
        hasActiveVault: boolean;
        activeVault: Vault | null;
        vaults: Vault[];
      }>("get_vault_status");

      // Use the initialized vault or fall back to active vault from status
      setVault(initializedVault || status.activeVault);
    } catch (error) {
      console.error("Failed to initialize vault system:", error);
      // If initialization fails, try to get vault status anyway
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const status = await invoke<{
          registryExists: boolean;
          hasVaults: boolean;
          hasActiveVault: boolean;
          activeVault: Vault | null;
          vaults: Vault[];
        }>("get_vault_status");

        if (status.hasActiveVault && status.activeVault) {
          setVault(status.activeVault);
        }
      } catch (statusError) {
        console.error("Failed to check vault status:", statusError);
      }
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
            <Route path="/workflow/:agentId" element={<WorkflowIDEPage />} />
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
