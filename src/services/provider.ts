// ============================================================================
// PROVIDERS SERVICE
// Frontend service for provider management
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { Provider, ProviderTestResult } from "@/shared/types";

/**
 * List all providers
 */
export async function listProviders(): Promise<Provider[]> {
  return invoke("list_providers");
}

/**
 * Get a single provider by ID
 */
export async function getProvider(id: string): Promise<Provider> {
  return invoke("get_provider", { id });
}

/**
 * Create a new provider
 */
export async function createProvider(provider: Omit<Provider, "id" | "createdAt">): Promise<Provider> {
  const providerWithId: Provider = {
    ...provider,
    id: provider.name.toLowerCase().replace(/\s+/g, "-"),
    createdAt: new Date().toISOString(),
  };
  return invoke("create_provider", { provider: providerWithId });
}

/**
 * Update an existing provider
 */
export async function updateProvider(id: string, provider: Omit<Provider, "id" | "createdAt">): Promise<Provider> {
  const providerWithId: Provider = {
    ...provider,
    id: provider.name.toLowerCase().replace(/\s+/g, "-"),
    createdAt: new Date().toISOString(),
  };
  return invoke("update_provider", { id, provider: providerWithId });
}

/**
 * Delete a provider
 */
export async function deleteProvider(id: string): Promise<void> {
  return invoke("delete_provider", { id });
}

/**
 * Test a provider connection
 */
export async function testProvider(provider: Omit<Provider, "id" | "createdAt">): Promise<ProviderTestResult> {
  return invoke("test_provider", { provider });
}
