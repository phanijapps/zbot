// ============================================================================
// VAULT SWITCHER
// Dropdown component for switching between vaults
// ============================================================================

import { Plus, Check } from "lucide-react";
import { useVaults } from "./useVaults";
import { CreateVaultDialog } from "./CreateVaultDialog";
import { useState } from "react";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/shared/ui/dropdown-menu";
import { cn } from "@/shared/utils";

interface VaultSwitcherProps {
  className?: string;
}

export function VaultSwitcher({ className = "" }: VaultSwitcherProps) {
  const { currentVault, vaults, switchVault, isLoading } = useVaults();
  const [showCreateDialog, setShowCreateDialog] = useState(false);

  const handleVaultChange = async (vaultId: string) => {
    await switchVault(vaultId);
  };

  const handleCreateVault = () => {
    setShowCreateDialog(true);
  };

  if (isLoading) {
    return (
      <div className={cn("flex items-center gap-2 px-2 py-1.5", className)}>
        <span className="text-xs text-gray-400">Loading...</span>
      </div>
    );
  }

  return (
    <>
      <div className="flex items-center gap-1 px-2 py-1.5">
        <span className="text-xs text-gray-400">Vault:</span>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              className="flex-1 flex items-center justify-between gap-1 bg-[#383a40] hover:bg-[#404249] text-white px-2 py-1 rounded border border-white/10 text-xs min-w-0 outline-none focus:border-violet-500 transition-colors"
              disabled={vaults.length === 0}
            >
              <span className="truncate">
                {currentVault?.name || "Select vault..."}
              </span>
              <svg className="size-3 shrink-0 opacity-50" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M6 9l6 6 6-6" />
              </svg>
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-[200px]">
            {vaults.map((vault) => (
              <DropdownMenuItem
                key={vault.id}
                onClick={() => handleVaultChange(vault.id)}
                className="flex items-center justify-between cursor-pointer"
              >
                <span className="flex-1">{vault.name}</span>
                {vault.isDefault && (
                  <span className="text-[10px] text-gray-400">Default</span>
                )}
                {currentVault?.id === vault.id && (
                  <Check className="size-3.5 text-violet-400" />
                )}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
        <button
          onClick={handleCreateVault}
          className="text-gray-400 hover:text-white transition-colors p-0.5"
          title="Create new vault"
        >
          <Plus className="size-3.5" />
        </button>
      </div>

      <CreateVaultDialog
        open={showCreateDialog}
        onClose={() => setShowCreateDialog(false)}
        onCreated={() => {
          setShowCreateDialog(false);
        }}
      />
    </>
  );
}
