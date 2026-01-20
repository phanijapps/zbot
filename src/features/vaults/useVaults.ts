// ============================================================================
// USE VAULTS HOOK
// React hook for vault state management
// ============================================================================

import { useState, useEffect, useCallback } from "react";
import {
  listVaults,
  switchVault as switchVaultService,
  onVaultChanged,
  type Vault,
} from "@/services/vaults";
import { initializeVaultSystem } from "@/services/vaults";

export function useVaults() {
  const [vaults, setVaults] = useState<Vault[]>([]);
  const [currentVault, setCurrentVault] = useState<Vault | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Load vaults on mount
  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    const loadVaults = async () => {
      setIsLoading(true);
      try {
        // Initialize vault system (creates default vault if needed)
        const active = await initializeVaultSystem();
        setCurrentVault(active);

        // Load all vaults
        const allVaults = await listVaults();
        setVaults(allVaults);

        // Listen for vault changes
        const unlistenPromise = onVaultChanged((vault) => {
          setCurrentVault(vault);
          // Reload all data after vault change
          window.location.reload();
        });

        unlistenPromise.then((unlisten) => {
          unlistenFn = unlisten;
        });
      } catch (error) {
        console.error("Failed to load vaults:", error);
      } finally {
        setIsLoading(false);
      }
    };

    loadVaults();

    return () => {
      unlistenFn?.();
    };
  }, []);

  const switchVault = useCallback(async (vaultId: string) => {
    try {
      const vault = await switchVaultService(vaultId);
      return vault;
    } catch (error) {
      console.error("Failed to switch vault:", error);
      throw error;
    }
  }, []);

  return {
    vaults,
    currentVault,
    isLoading,
    switchVault,
  };
}
