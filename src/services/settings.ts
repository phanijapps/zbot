// ============================================================================
// SETTINGS SERVICE
// Frontend service for settings management
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type {
  Settings,
  StorageInfo,
  DirectoriesInfo,
} from "@/features/settings/types";

/**
 * Get all application settings
 */
export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

/**
 * Save application settings
 */
export async function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

/**
 * Reset settings to defaults
 */
export async function resetSettings(): Promise<Settings> {
  return invoke("reset_settings");
}

/**
 * Get storage information
 */
export async function getStorageInfo(): Promise<StorageInfo> {
  return invoke("get_storage_info");
}

/**
 * Clear all application data (except settings)
 */
export async function clearAllData(): Promise<void> {
  return invoke("clear_all_data");
}

/**
 * Get the config directory path
 */
export async function getConfigPath(): Promise<string> {
  return invoke("get_config_path");
}

/**
 * Initialize application directories
 */
export async function initializeDirectories(): Promise<DirectoriesInfo> {
  return invoke("initialize_directories");
}

/**
 * Format bytes to human readable format
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 Bytes";

  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return Math.round(bytes / Math.pow(k, i) * 100) / 100 + " " + sizes[i];
}

/**
 * Get storage usage percentage
 */
export function getStoragePercentage(used: number, total: number = 10 * 1024 * 1024 * 1024): number {
  return Math.round((used / total) * 100);
}

// ============================================================================
// PYTHON VENV REQUIREMENTS
// ============================================================================

/**
 * Information about the Python venv
 */
export interface VenvInfo {
  venv_path: string;
  venv_exists: boolean;
  requirements_exists: boolean;
  installed_packages: string | null;  // JSON string
}

/**
 * Information about an installed package
 */
export interface PackageInfo {
  name: string;
  version: string;
}

/**
 * Get venv information
 */
export async function getVenvInfo(): Promise<VenvInfo> {
  return invoke("get_venv_info");
}

/**
 * Read requirements.txt content
 */
export async function readRequirements(): Promise<string> {
  return invoke("read_requirements");
}

/**
 * Save requirements.txt content
 */
export async function saveRequirements(content: string): Promise<void> {
  return invoke("save_requirements", { content });
}

/**
 * Install requirements from requirements.txt
 */
export async function installRequirements(): Promise<string> {
  return invoke("install_requirements");
}

/**
 * List installed packages in the venv
 */
export async function listInstalledPackages(): Promise<PackageInfo[]> {
  return invoke("list_installed_packages");
}
