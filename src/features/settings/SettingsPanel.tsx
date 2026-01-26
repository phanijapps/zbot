// ============================================================================
// SETTINGS FEATURE
// Application settings with persistence
// ============================================================================

import { useEffect, useState } from "react";
import { Bell, Lock, Database, Palette, Zap, Loader2, FileCode, Package, CheckCircle2, AlertCircle, FolderOpen } from "lucide-react";
import { Switch } from "@/shared/ui/switch";
import { Button } from "@/shared/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import { Textarea } from "@/shared/ui/textarea";
import * as settingsService from "@/services/settings";
import * as themesService from "@/services/themes";
import type { ThemeInfo } from "@/services/themes";
import type { Settings, StorageInfo } from "./types";
import { defaultSettings } from "./types";
import type { VenvInfo, PackageInfo } from "@/services/settings";
import { useTheme } from "@/core";

export function SettingsPanel() {
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  // Theme state
  const { setTheme } = useTheme();
  const [themes, setThemes] = useState<ThemeInfo[]>([]);

  // Python venv state
  const [venvInfo, setVenvInfo] = useState<VenvInfo | null>(null);
  const [requirements, setRequirements] = useState("");
  const [requirementsChanged, setRequirementsChanged] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installStatus, setInstallStatus] = useState<{ type: "success" | "error"; message: string } | null>(null);
  const [installedPackages, setInstalledPackages] = useState<PackageInfo[]>([]);
  const [showPackages, setShowPackages] = useState(false);

  // Load settings on mount
  useEffect(() => {
    loadSettings();
    loadStorageInfo();
    loadVenvInfo();
    loadThemes();
  }, []);

  const loadThemes = async () => {
    try {
      const themeList = await themesService.listThemes();
      setThemes(themeList);
    } catch (error) {
      console.error("Failed to load themes:", error);
    }
  };

  const loadSettings = async () => {
    try {
      const loaded = await settingsService.getSettings();
      setSettings(loaded);
    } catch (error) {
      console.error("Failed to load settings:", error);
    } finally {
      setLoading(false);
    }
  };

  const loadStorageInfo = async () => {
    try {
      const info = await settingsService.getStorageInfo();
      setStorageInfo(info);
    } catch (error) {
      console.error("Failed to load storage info:", error);
    }
  };

  const updateSetting = <K extends keyof Settings>(
    key: K,
    value: Settings[K]
  ) => {
    const newSettings = { ...settings, [key]: value };
    setSettings(newSettings);
    debouncedSave(newSettings);
  };

  const updateAppearanceSetting = <K extends keyof Settings["appearance"]>(
    key: K,
    value: Settings["appearance"][K]
  ) => {
    updateSetting("appearance", { ...settings.appearance, [key]: value });
    // Apply theme immediately when changed
    if (key === "theme" && typeof value === "string") {
      setTheme(value);
    }
  };

  const handleOpenThemesFolder = async () => {
    try {
      const path = await themesService.getThemesDirPath();
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("open_folder", { path });
    } catch (error) {
      console.error("Failed to open themes folder:", error);
    }
  };

  const updatePerformanceSetting = <K extends keyof Settings["performance"]>(
    key: K,
    value: Settings["performance"][K]
  ) => {
    updateSetting("performance", { ...settings.performance, [key]: value });
  };

  const updateNotificationSetting = <K extends keyof Settings["notifications"]>(
    key: K,
    value: Settings["notifications"][K]
  ) => {
    updateSetting("notifications", { ...settings.notifications, [key]: value });
  };


  // Debounced save to avoid too many writes
  let saveTimeout: ReturnType<typeof setTimeout>;
  const debouncedSave = (newSettings: Settings) => {
    clearTimeout(saveTimeout);
    saveTimeout = setTimeout(() => {
      saveSettings(newSettings);
    }, 500);
  };

  const saveSettings = async (newSettings: Settings) => {
    setSaving(true);
    try {
      await settingsService.saveSettings(newSettings);
    } catch (error) {
      console.error("Failed to save settings:", error);
      // Reload settings on error
      loadSettings();
    } finally {
      setSaving(false);
    }
  };

  const handleClearAllData = async () => {
    if (confirm("Are you sure you want to clear all data? This cannot be undone.")) {
      try {
        await settingsService.clearAllData();
        await loadStorageInfo();
      } catch (error) {
        console.error("Failed to clear data:", error);
      }
    }
  };

  const handleResetSettings = async () => {
    if (confirm("Reset all settings to defaults?")) {
      try {
        const reset = await settingsService.resetSettings();
        setSettings(reset);
      } catch (error) {
        console.error("Failed to reset settings:", error);
      }
    }
  };

  // Python venv functions
  const loadVenvInfo = async () => {
    try {
      const info = await settingsService.getVenvInfo();
      setVenvInfo(info);

      // Load requirements if they exist
      if (info.requirements_exists) {
        const req = await settingsService.readRequirements();
        setRequirements(req);
      } else {
        // Default template
        setRequirements("# Add your Python requirements here (one per line)\n# Example: numpy==1.24.0\n");
      }

      // Load installed packages
      if (info.venv_exists) {
        try {
          const packages = await settingsService.listInstalledPackages();
          setInstalledPackages(packages);
        } catch (e) {
          console.error("Failed to load packages:", e);
        }
      }
    } catch (error) {
      console.error("Failed to load venv info:", error);
    }
  };

  const handleRequirementsChange = (value: string) => {
    setRequirements(value);
    setRequirementsChanged(true);
    setInstallStatus(null);
  };

  const handleSaveRequirements = async () => {
    try {
      await settingsService.saveRequirements(requirements);
      setRequirementsChanged(false);
      setInstallStatus({ type: "success", message: "Requirements saved successfully" });
    } catch (error) {
      console.error("Failed to save requirements:", error);
      setInstallStatus({ type: "error", message: `Failed to save: ${error}` });
    }
  };

  const handleInstallRequirements = async () => {
    setInstalling(true);
    setInstallStatus(null);

    try {
      // First save if changed
      if (requirementsChanged) {
        await settingsService.saveRequirements(requirements);
        setRequirementsChanged(false);
      }

      // Then install
      const result = await settingsService.installRequirements();
      setInstallStatus({ type: "success", message: result });

      // Reload packages
      const packages = await settingsService.listInstalledPackages();
      setInstalledPackages(packages);

      // Reload venv info
      await loadVenvInfo();
    } catch (error) {
      console.error("Failed to install requirements:", error);
      setInstallStatus({ type: "error", message: `Installation failed: ${error}` });
    } finally {
      setInstalling(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <Loader2 className="size-8 text-foreground animate-spin" />
      </div>
    );
  }

  const storageUsed = storageInfo ? settingsService.formatBytes(storageInfo.total_used) : "Unknown";
  const storagePercentage = storageInfo ? settingsService.getStoragePercentage(storageInfo.total_used) : 0;

  return (
    <div className="p-8 max-w-4xl">
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-foreground mb-2">Settings</h1>
          <p className="text-muted-foreground">Customize your AI assistant experience</p>
        </div>
        {saving && (
          <div className="flex items-center gap-2 text-muted-foreground text-sm">
            <Loader2 className="size-4 animate-spin" />
            Saving...
          </div>
        )}
      </div>

      <div className="space-y-6">
        {/* Appearance */}
        <div className="bg-card rounded-2xl p-6 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-purple-500 to-pink-600 p-2 rounded-lg">
              <Palette className="size-5 text-white" />
            </div>
            <h2 className="text-foreground font-semibold text-lg">Appearance</h2>
          </div>
          <div className="space-y-4">
            <div>
              <div className="flex items-center justify-between mb-2">
                <p className="text-foreground text-sm">Theme</p>
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground hover:text-foreground h-6 px-2"
                  onClick={handleOpenThemesFolder}
                >
                  <FolderOpen className="size-3 mr-1" />
                  Open Folder
                </Button>
              </div>
              <Select
                value={settings.appearance.theme}
                onValueChange={(value) => updateAppearanceSetting("theme", value)}
              >
                <SelectTrigger className="bg-input border-border text-foreground">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="bg-popover border-border">
                  {themes.map((theme) => (
                    <SelectItem key={theme.id} value={theme.id}>
                      <div className="flex items-center gap-2">
                        <span>{theme.name}</span>
                        {theme.author && (
                          <span className="text-xs text-muted-foreground">by {theme.author}</span>
                        )}
                      </div>
                    </SelectItem>
                  ))}
                  {themes.length === 0 && (
                    <SelectItem value="default">Default</SelectItem>
                  )}
                </SelectContent>
              </Select>
              <p className="text-muted-foreground text-xs mt-1">
                Drop .css theme files in the themes folder to add custom themes
              </p>
            </div>
            <div>
              <p className="text-white text-sm mb-2">Font Size</p>
              <Select
                value={settings.appearance.font_size}
                onValueChange={(value) => updateAppearanceSetting("font_size", value)}
              >
                <SelectTrigger className="bg-input border-border text-foreground">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="bg-popover border-border">
                  <SelectItem value="small">Small</SelectItem>
                  <SelectItem value="medium">Medium</SelectItem>
                  <SelectItem value="large">Large</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>

        {/* Performance */}
        <div className="bg-card rounded-2xl p-6 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-blue-500 to-cyan-600 p-2 rounded-lg">
              <Zap className="size-5 text-white" />
            </div>
            <h2 className="text-foreground font-semibold text-lg">Performance</h2>
          </div>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-foreground text-sm">Hardware Acceleration</p>
                <p className="text-muted-foreground text-xs">
                  Use GPU for faster processing
                </p>
              </div>
              <Switch
                checked={settings.performance.hardware_acceleration}
                onCheckedChange={(checked) => updatePerformanceSetting("hardware_acceleration", checked)}
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="text-foreground text-sm">Stream Responses</p>
                <p className="text-muted-foreground text-xs">
                  Show responses as they generate
                </p>
              </div>
              <Switch
                checked={settings.performance.stream_responses}
                onCheckedChange={(checked) => updatePerformanceSetting("stream_responses", checked)}
              />
            </div>
          </div>
        </div>

        {/* Notifications */}
        <div className="bg-card rounded-2xl p-6 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-orange-500 to-red-600 p-2 rounded-lg">
              <Bell className="size-5 text-white" />
            </div>
            <h2 className="text-foreground font-semibold text-lg">Notifications</h2>
          </div>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-foreground text-sm">Desktop Notifications</p>
                <p className="text-muted-foreground text-xs">
                  Show notifications on desktop
                </p>
              </div>
              <Switch
                checked={settings.notifications.desktop_notifications}
                onCheckedChange={(checked) => updateNotificationSetting("desktop_notifications", checked)}
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="text-foreground text-sm">Sound Effects</p>
                <p className="text-muted-foreground text-xs">
                  Play sound for new messages
                </p>
              </div>
              <Switch
                checked={settings.notifications.sound_effects}
                onCheckedChange={(checked) => updateNotificationSetting("sound_effects", checked)}
              />
            </div>
          </div>
        </div>

        {/* Data Management */}
        <div className="bg-card rounded-2xl p-6 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-red-500 to-orange-600 p-2 rounded-lg">
              <Lock className="size-5 text-white" />
            </div>
            <h2 className="text-foreground font-semibold text-lg">Data Management</h2>
          </div>
          <div className="space-y-3">
            <div>
              <Button
                variant="outline"
                className="w-full border-orange-500/30 text-orange-400 hover:bg-orange-500/10 hover:text-orange-300"
                onClick={handleClearAllData}
              >
                Delete All Conversations
              </Button>
              <p className="text-muted-foreground text-xs mt-1">
                Removes all chat history and session data
              </p>
            </div>
            <div>
              <Button
                variant="outline"
                className="w-full border-red-500/30 text-red-400 hover:bg-red-500/10 hover:text-red-300"
                onClick={async () => {
                  if (confirm("Are you sure you want to wipe all knowledge graph data? This cannot be undone.")) {
                    try {
                      await settingsService.clearKnowledgeGraph();
                      alert("Knowledge graph wiped successfully");
                    } catch (error) {
                      console.error("Failed to clear knowledge graph:", error);
                      alert("Failed to wipe knowledge graph");
                    }
                  }
                }}
              >
                Wipe Knowledge Graph
              </Button>
              <p className="text-muted-foreground text-xs mt-1">
                Removes all learned entities and relationships
              </p>
            </div>
          </div>
        </div>

        {/* Storage */}
        <div className="bg-card rounded-2xl p-6 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-yellow-500 to-orange-600 p-2 rounded-lg">
              <Database className="size-5 text-white" />
            </div>
            <h2 className="text-foreground font-semibold text-lg">Storage</h2>
          </div>
          <div className="space-y-4">
            <div>
              <div className="flex items-center justify-between mb-2">
                <p className="text-foreground text-sm">Storage Used</p>
                <p className="text-muted-foreground text-sm">{storageUsed} / 10 GB</p>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-blue-500 to-purple-600 rounded-full transition-all"
                  style={{ width: `${storagePercentage}%` }}
                />
              </div>
            </div>
            <div className="flex gap-2">
              <Button
                variant="outline"
                className="flex-1 border-border text-foreground hover:bg-accent"
                onClick={loadStorageInfo}
              >
                Refresh
              </Button>
              <Button
                variant="outline"
                className="flex-1 border-border text-foreground hover:bg-accent"
                onClick={handleResetSettings}
              >
                Reset Settings
              </Button>
            </div>
          </div>
        </div>

        {/* Python Environment */}
        <div className="bg-card rounded-2xl p-6 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-emerald-500 to-teal-600 p-2 rounded-lg">
              <FileCode className="size-5 text-white" />
            </div>
            <div className="flex-1">
              <h2 className="text-foreground font-semibold text-lg">Python Environment</h2>
              <p className="text-muted-foreground text-xs">
                {venvInfo?.venv_exists
                  ? `Venv at ${venvInfo.venv_path?.split('/').slice(-2).join('/')}`
                  : "No Python venv found"
                }
              </p>
            </div>
            {venvInfo?.venv_exists && (
              <Button
                variant="outline"
                size="sm"
                className="border-border text-foreground hover:bg-accent"
                onClick={() => setShowPackages(!showPackages)}
              >
                <Package className="size-4 mr-2" />
                {installedPackages.length} Packages
              </Button>
            )}
          </div>

          {venvInfo && !venvInfo.venv_exists && (
            <div className="mb-4 p-3 bg-orange-500/10 border border-orange-500/30 rounded-lg">
              <p className="text-orange-400 text-sm">
                Python venv not found. Please create it first to manage packages.
              </p>
            </div>
          )}

          {venvInfo?.venv_exists && (
            <div className="space-y-4">
              {/* Installed packages list */}
              {showPackages && (
                <div className="p-3 bg-muted rounded-lg max-h-48 overflow-y-auto">
                  <div className="text-xs text-muted-foreground mb-2">Installed packages:</div>
                  <div className="grid grid-cols-2 gap-1">
                    {installedPackages.map((pkg) => (
                      <div key={pkg.name} className="text-xs text-foreground flex items-center gap-1">
                        <CheckCircle2 className="size-3 text-green-500 shrink-0" />
                        <span className="truncate">{pkg.name}</span>
                        <span className="text-muted-foreground shrink-0">v{pkg.version}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Requirements editor */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <p className="text-foreground text-sm">requirements.txt</p>
                  {requirementsChanged && (
                    <span className="text-xs text-orange-400">Unsaved changes</span>
                  )}
                </div>
                <Textarea
                  value={requirements}
                  onChange={(e) => handleRequirementsChange(e.target.value)}
                  placeholder="# Add your Python requirements here (one per line)&#10;# Example: numpy==1.24.0"
                  className="bg-input border-border text-foreground text-sm font-mono min-h-[120px] resize-y"
                />
              </div>

              {/* Status message */}
              {installStatus && (
                <div className={`p-3 rounded-lg flex items-start gap-2 ${
                  installStatus.type === "success"
                    ? "bg-green-500/10 border border-green-500/30"
                    : "bg-red-500/10 border border-red-500/30"
                }`}>
                  {installStatus.type === "success" ? (
                    <CheckCircle2 className="size-4 text-green-500 shrink-0 mt-0.5" />
                  ) : (
                    <AlertCircle className="size-4 text-red-500 shrink-0 mt-0.5" />
                  )}
                  <pre className="text-xs text-foreground whitespace-pre-wrap font-sans">
                    {installStatus.message}
                  </pre>
                </div>
              )}

              {/* Action buttons */}
              <div className="flex gap-2">
                {requirementsChanged && (
                  <Button
                    variant="outline"
                    className="border-border text-foreground hover:bg-accent"
                    onClick={handleSaveRequirements}
                  >
                    Save Only
                  </Button>
                )}
                <Button
                  className="flex-1 bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-700 hover:to-teal-700 text-white"
                  onClick={handleInstallRequirements}
                  disabled={installing}
                >
                  {installing ? (
                    <>
                      <Loader2 className="size-4 mr-2 animate-spin" />
                      Installing...
                    </>
                  ) : (
                    <>
                      <Package className="size-4 mr-2" />
                      Apply & Install
                    </>
                  )}
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
