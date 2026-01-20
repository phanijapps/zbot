// ============================================================================
// VAULT SERVICE
// API wrapper for vault management commands
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Vault,
  CreateVaultRequest,
  VaultInfo,
} from "@/shared/types";

/**
 * List all vaults
 */
export async function listVaults(): Promise<Vault[]> {
  return await invoke<Vault[]>("list_vaults");
}

/**
 * Get the currently active vault
 */
export async function getActiveVault(): Promise<Vault> {
  return await invoke<Vault>("get_active_vault");
}

/**
 * Create a new vault
 */
export async function createVault(request: CreateVaultRequest): Promise<Vault> {
  return await invoke<Vault>("create_vault", { request });
}

/**
 * Switch to a different vault
 * This will reload the application with the new vault's configuration
 */
export async function switchVault(vaultId: string): Promise<Vault> {
  return await invoke<Vault>("switch_vault", { vaultId });
}

/**
 * Delete a vault
 * Note: Cannot delete the active vault or the default vault
 */
export async function deleteVault(vaultId: string): Promise<void> {
  await invoke("delete_vault", { vaultId });
}

/**
 * Get detailed information about a vault
 */
export async function getVaultInfo(vaultId: string): Promise<VaultInfo> {
  return await invoke<VaultInfo>("get_vault_info", { vaultId });
}

/**
 * Initialize the vault system
 * This should be called on app startup to ensure vault registry exists
 */
export async function initializeVaultSystem(): Promise<Vault> {
  return await invoke<Vault>("initialize_vault_system");
}

/**
 * Listen for vault change events
 * When a vault is changed, this callback will be invoked with the new vault
 */
export function onVaultChanged(callback: (vault: Vault) => void) {
  return listen<Vault>("vault-changed", (event) => {
    callback(event.payload);
  });
}

// Export types for convenience
export type { Vault, CreateVaultRequest, VaultInfo };
