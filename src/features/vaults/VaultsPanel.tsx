// ============================================================================
// VAULTS PANEL
// Full vault management panel with detailed information
// ============================================================================

import { useEffect, useState } from "react";
import { Trash2, FolderOpen, HardDrive, FileText, Wrench } from "lucide-react";
import {
  listVaults,
  getVaultInfo,
  deleteVault,
  type Vault,
  type VaultInfo,
} from "@/services/vaults";

interface VaultsPanelProps {
  onClose?: () => void;
  onVaultClick?: (vault: Vault) => void;
}

// Helper function to format bytes
function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function VaultsPanel({ onClose, onVaultClick }: VaultsPanelProps) {
  const [vaults, setVaults] = useState<Vault[]>([]);
  const [vaultInfos, setVaultInfos] = useState<Map<string, VaultInfo>>(new Map());
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadVaults();
  }, []);

  const loadVaults = async () => {
    setIsLoading(true);
    try {
      const allVaults = await listVaults();
      setVaults(allVaults);

      // Load info for each vault
      const infos = new Map<string, VaultInfo>();
      for (const vault of allVaults) {
        try {
          const info = await getVaultInfo(vault.id);
          infos.set(vault.id, info);
        } catch (error) {
          console.error(`Failed to load info for vault ${vault.id}:`, error);
        }
      }
      setVaultInfos(infos);
    } catch (error) {
      console.error("Failed to load vaults:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleDeleteVault = async (vaultId: string, vaultName: string) => {
    if (!confirm(`Are you sure you want to delete the vault "${vaultName}"? This will remove all agents, skills, and data in this vault.`)) {
      return;
    }

    try {
      await deleteVault(vaultId);
      await loadVaults(); // Reload the list
    } catch (error) {
      alert(`Failed to delete vault: ${error}`);
    }
  };

  if (isLoading) {
    return (
      <div className="p-4">
        <div className="text-center text-gray-400">Loading vaults...</div>
      </div>
    );
  }

  return (
    <div className="p-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-white">Vaults</h2>
        {onClose && (
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white"
          >
            ✕
          </button>
        )}
      </div>

      {/* Vault List */}
      <div className="space-y-2">
        {vaults.map((vault) => {
          const info = vaultInfos.get(vault.id);
          return (
            <div
              key={vault.id}
              className="bg-gray-800 rounded-lg border border-white/10 overflow-hidden"
            >
              {/* Vault Header */}
              <div
                className="flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-gray-700/50 transition-colors"
                onClick={() => onVaultClick?.(vault)}
              >
                <div className="flex items-center gap-3">
                  <FolderOpen className="size-5 text-purple-400" />
                  <div>
                    <div className="font-medium text-white">{vault.name}</div>
                    <div className="text-xs text-gray-400">{vault.path}</div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {vault.isDefault && (
                    <span className="text-xs bg-purple-500/20 text-purple-300 px-2 py-1 rounded">
                      Default
                    </span>
                  )}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteVault(vault.id, vault.name);
                    }}
                    disabled={vault.isDefault}
                    className="text-gray-400 hover:text-red-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    title="Delete vault"
                  >
                    <Trash2 className="size-4" />
                  </button>
                </div>
              </div>

              {/* Vault Details */}
              {info && (
                <div className="px-4 py-3 bg-gray-900/50 border-t border-white/10 grid grid-cols-2 gap-4 text-sm">
                  {/* Storage */}
                  <div className="flex items-center gap-2">
                    <HardDrive className="size-4 text-gray-400" />
                    <span className="text-gray-400">Size:</span>
                    <span className="text-white">{formatBytes(info.storageInfo.totalUsed)}</span>
                  </div>

                  {/* Agents */}
                  <div className="flex items-center gap-2">
                    <Wrench className="size-4 text-gray-400" />
                    <span className="text-gray-400">Agents:</span>
                    <span className="text-white">{info.agentCount}</span>
                  </div>

                  {/* Skills */}
                  <div className="flex items-center gap-2">
                    <FileText className="size-4 text-gray-400" />
                    <span className="text-gray-400">Skills:</span>
                    <span className="text-white">{info.skillCount}</span>
                  </div>

                  {/* Last Accessed */}
                  <div className="flex items-center gap-2">
                    <span className="text-gray-400">Last accessed:</span>
                    <span className="text-white">
                      {new Date(info.vault.lastAccessed).toLocaleDateString()}
                    </span>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Empty State */}
      {vaults.length === 0 && (
        <div className="text-center py-8 text-gray-400">
          <p>No vaults found.</p>
        </div>
      )}
    </div>
  );
}
