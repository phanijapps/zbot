// ============================================================================
// VAULT SELECTOR
// Full-page vault selection and creation interface
// Shows when no vault is selected or vault needs to be created
// ============================================================================

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, FolderOpen, Sparkles, CheckCircle } from "lucide-react";
import { CreateVaultDialog } from "./CreateVaultDialog";
import type { Vault, VaultStatus } from "@/shared/types";

interface VaultSelectorProps {
  onVaultSelected: (vault: Vault) => void;
}

type ViewState = "loading" | "no_vaults" | "has_vaults";

export function VaultSelector({ onVaultSelected }: VaultSelectorProps) {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [viewState, setViewState] = useState<ViewState>("loading");
  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);

  useEffect(() => {
    loadVaultStatus();
  }, []);

  const loadVaultStatus = async () => {
    try {
      const vaultStatus = await invoke<VaultStatus>("get_vault_status");
      setStatus(vaultStatus);

      // If no vaults exist, show creation flow
      if (!vaultStatus.hasVaults) {
        setViewState("no_vaults");
      }
      // If vaults exist but no active vault, show selector
      else if (!vaultStatus.hasActiveVault) {
        setViewState("has_vaults");
      }
      // If there's a valid active vault, use it
      else if (vaultStatus.activeVault) {
        onVaultSelected(vaultStatus.activeVault);
      }
    } catch (error) {
      console.error("Failed to load vault status:", error);
      setViewState("no_vaults");
    }
  };

  const handleSelectVault = async (vault: Vault) => {
    setIsSwitching(true);
    try {
      // Set as default vault (this will be remembered on next launch)
      const selected = await invoke<Vault>("set_default_vault", {
        vaultId: vault.id,
      });

      onVaultSelected(selected);
    } catch (error) {
      console.error("Failed to select vault:", error);
      alert(`Failed to select vault: ${error}`);
    } finally {
      setIsSwitching(false);
    }
  };

  const handleVaultCreated = () => {
    setShowCreateDialog(false);
    loadVaultStatus();
  };

  // Loading state
  if (viewState === "loading") {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#1a1a1a]">
        <div className="text-center">
          <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-violet-500 mb-4"></div>
          <p className="text-gray-400">Loading vaults...</p>
        </div>
      </div>
    );
  }

  // No vaults exist - show creation flow
  if (viewState === "no_vaults") {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#1a1a1a] p-4">
        <div className="max-w-md w-full">
          {/* Welcome Card */}
          <div className="bg-[#252525] border border-white/10 rounded-2xl p-8 text-center">
            <div className="flex justify-center mb-6">
              <div className="w-16 h-16 bg-violet-500/20 rounded-full flex items-center justify-center">
                <Sparkles className="w-8 h-8 text-violet-400" />
              </div>
            </div>

            <h1 className="text-2xl font-bold text-white mb-2">
              Welcome to AgentZero
            </h1>
            <p className="text-gray-400 mb-8">
              Create your first vault to get started. A vault stores your agents, conversations, and settings.
            </p>

            <div className="space-y-4">
              <div className="bg-[#1a1a1a] border border-white/5 rounded-lg p-4 text-left">
                <h3 className="text-white font-medium mb-2">What is a vault?</h3>
                <p className="text-gray-400 text-sm">
                  A vault is like a workspace or profile. Each vault has its own:
                </p>
                <ul className="text-gray-400 text-sm mt-2 space-y-1">
                  <li>• Agents and their configurations</li>
                  <li>• Conversation history</li>
                  <li>• Skills and tools</li>
                  <li>• Settings and preferences</li>
                </ul>
              </div>

              <button
                onClick={() => setShowCreateDialog(true)}
                className="w-full bg-violet-600 hover:bg-violet-700 text-white font-medium py-3 px-6 rounded-lg transition-colors flex items-center justify-center gap-2"
              >
                <Plus className="w-5 h-5" />
                Create Your First Vault
              </button>
            </div>
          </div>
        </div>

        {showCreateDialog && (
          <CreateVaultDialog
            open={showCreateDialog}
            onClose={() => setShowCreateDialog(false)}
            onCreated={handleVaultCreated}
          />
        )}
      </div>
    );
  }

  // Has vaults but no active vault selected - show selector
  return (
    <div className="min-h-screen flex items-center justify-center bg-[#1a1a1a] p-4">
      <div className="max-w-2xl w-full">
        <div className="bg-[#252525] border border-white/10 rounded-2xl p-8">
          <div className="flex justify-center mb-6">
            <div className="w-16 h-16 bg-blue-500/20 rounded-full flex items-center justify-center">
              <FolderOpen className="w-8 h-8 text-blue-400" />
            </div>
          </div>

          <h1 className="text-2xl font-bold text-white text-center mb-2">
            Select a Vault
          </h1>
          <p className="text-gray-400 text-center mb-8">
            Choose a vault to continue. The vault with the checkmark will be used by default.
          </p>

          {status && status.vaults.length > 0 && (
            <div className="space-y-3 mb-6">
              {status.vaults.map((vault) => {
                const isActive = vault.id === status.activeVault?.id;

                return (
                  <button
                    key={vault.id}
                    onClick={() => !isSwitching && handleSelectVault(vault)}
                    disabled={isSwitching}
                    className={`
                      w-full text-left p-4 rounded-lg border transition-all
                      ${isActive
                        ? "bg-violet-500/20 border-violet-500"
                        : "bg-[#1a1a1a] border-white/10 hover:border-white/20"
                      }
                      ${isSwitching ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}
                    `}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        <FolderOpen className="w-5 h-5 text-gray-400" />
                        <div>
                          <div className="text-white font-medium">{vault.name}</div>
                          <div className="text-gray-500 text-sm">{vault.path}</div>
                        </div>
                      </div>
                      <div className="flex items-center gap-3">
                        {isActive && (
                          <span className="text-xs bg-blue-500/20 text-blue-300 px-2 py-1 rounded">
                            Default
                          </span>
                        )}
                        <div className={`
                          w-6 h-6 rounded border-2 flex items-center justify-center transition-colors
                          ${isActive
                            ? "bg-violet-500 border-violet-500"
                            : "border-gray-500"
                          }
                        `}>
                          {isActive && <CheckCircle className="w-4 h-4 text-white" />}
                        </div>
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          )}

          <div className="flex gap-3">
            <button
              onClick={() => setShowCreateDialog(true)}
              disabled={isSwitching}
              className="flex-1 bg-violet-600 hover:bg-violet-700 disabled:bg-violet-800 text-white font-medium py-3 px-4 rounded-lg transition-colors flex items-center justify-center gap-2"
            >
              <Plus className="w-5 h-5" />
              Create New Vault
            </button>
          </div>
        </div>

        {showCreateDialog && (
          <CreateVaultDialog
            open={showCreateDialog}
            onClose={() => setShowCreateDialog(false)}
            onCreated={handleVaultCreated}
          />
        )}
      </div>
    </div>
  );
}
