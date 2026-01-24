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
  const [isSwitchingVault, setIsSwitchingVault] = useState(false);

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
          // Hide loading animation after vault is switched
          setIsSwitchingVault(false);
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
    // Show loading animation immediately
    setIsSwitchingVault(true);
    try {
      const vault = await switchVaultService(vaultId);
      // Note: The actual vault state update happens in onVaultChanged listener
      // The loader will be hidden there after the event fires
      return vault;
    } catch (error) {
      console.error("Failed to switch vault:", error);
      setIsSwitchingVault(false); // Hide loader on error
      throw error;
    }
  }, []);

  const reloadVaults = useCallback(async () => {
    try {
      const allVaults = await listVaults();
      setVaults(allVaults);
    } catch (error) {
      console.error("Failed to reload vaults:", error);
    }
  }, []);

  return {
    vaults,
    currentVault,
    isLoading,
    isSwitchingVault,
    switchVault,
    reloadVaults,
  };
}
