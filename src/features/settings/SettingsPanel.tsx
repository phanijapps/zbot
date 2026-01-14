// ============================================================================
// SETTINGS FEATURE
// Application settings with persistence
// ============================================================================

import { useEffect, useState } from "react";
import { Bell, Lock, Database, Palette, Zap, Loader2 } from "lucide-react";
import { Switch } from "@/shared/ui/switch";
import { Button } from "@/shared/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/shared/ui/select";
import * as settingsService from "@/services/settings";
import type { Settings, StorageInfo } from "./types";
import { defaultSettings } from "./types";

export function SettingsPanel() {
  const [settings, setSettings] = useState<Settings>(defaultSettings);
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  // Load settings on mount
  useEffect(() => {
    loadSettings();
    loadStorageInfo();
  }, []);

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

  const updatePrivacySetting = <K extends keyof Settings["privacy"]>(
    key: K,
    value: Settings["privacy"][K]
  ) => {
    updateSetting("privacy", { ...settings.privacy, [key]: value });
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

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <Loader2 className="size-8 text-white animate-spin" />
      </div>
    );
  }

  const storageUsed = storageInfo ? settingsService.formatBytes(storageInfo.total_used) : "Unknown";
  const storagePercentage = storageInfo ? settingsService.getStoragePercentage(storageInfo.total_used) : 0;

  return (
    <div className="p-8 max-w-4xl">
      <div className="mb-8 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-white mb-2">Settings</h1>
          <p className="text-gray-400">Customize your AI assistant experience</p>
        </div>
        {saving && (
          <div className="flex items-center gap-2 text-gray-400 text-sm">
            <Loader2 className="size-4 animate-spin" />
            Saving...
          </div>
        )}
      </div>

      <div className="space-y-6">
        {/* Appearance */}
        <div className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-2xl p-6 border border-white/10">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-purple-500 to-pink-600 p-2 rounded-lg">
              <Palette className="size-5 text-white" />
            </div>
            <h2 className="text-white font-semibold text-lg">Appearance</h2>
          </div>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-white text-sm">Dark Mode</p>
                <p className="text-gray-500 text-xs">Use dark theme</p>
              </div>
              <Switch
                checked={settings.appearance.dark_mode}
                onCheckedChange={(checked) => updateAppearanceSetting("dark_mode", checked)}
              />
            </div>
            <div>
              <p className="text-white text-sm mb-2">Theme</p>
              <Select
                value={settings.appearance.theme}
                onValueChange={(value) => updateAppearanceSetting("theme", value)}
              >
                <SelectTrigger className="bg-white/5 border-white/10 text-white">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="bg-[#1a1a1a] border-white/10">
                  <SelectItem value="default">Default</SelectItem>
                  <SelectItem value="purple">Purple Dream</SelectItem>
                  <SelectItem value="blue">Ocean Blue</SelectItem>
                  <SelectItem value="green">Forest Green</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div>
              <p className="text-white text-sm mb-2">Font Size</p>
              <Select
                value={settings.appearance.font_size}
                onValueChange={(value) => updateAppearanceSetting("font_size", value)}
              >
                <SelectTrigger className="bg-white/5 border-white/10 text-white">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="bg-[#1a1a1a] border-white/10">
                  <SelectItem value="small">Small</SelectItem>
                  <SelectItem value="medium">Medium</SelectItem>
                  <SelectItem value="large">Large</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>

        {/* Performance */}
        <div className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-2xl p-6 border border-white/10">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-blue-500 to-cyan-600 p-2 rounded-lg">
              <Zap className="size-5 text-white" />
            </div>
            <h2 className="text-white font-semibold text-lg">Performance</h2>
          </div>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-white text-sm">Hardware Acceleration</p>
                <p className="text-gray-500 text-xs">
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
                <p className="text-white text-sm">Stream Responses</p>
                <p className="text-gray-500 text-xs">
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
        <div className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-2xl p-6 border border-white/10">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-orange-500 to-red-600 p-2 rounded-lg">
              <Bell className="size-5 text-white" />
            </div>
            <h2 className="text-white font-semibold text-lg">Notifications</h2>
          </div>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-white text-sm">Desktop Notifications</p>
                <p className="text-gray-500 text-xs">
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
                <p className="text-white text-sm">Sound Effects</p>
                <p className="text-gray-500 text-xs">
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

        {/* Privacy */}
        <div className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-2xl p-6 border border-white/10">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-green-500 to-emerald-600 p-2 rounded-lg">
              <Lock className="size-5 text-white" />
            </div>
            <h2 className="text-white font-semibold text-lg">Privacy</h2>
          </div>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-white text-sm">Save Chat History</p>
                <p className="text-gray-500 text-xs">
                  Store conversations locally
                </p>
              </div>
              <Switch
                checked={settings.privacy.save_chat_history}
                onCheckedChange={(checked) => updatePrivacySetting("save_chat_history", checked)}
              />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <p className="text-white text-sm">Analytics</p>
                <p className="text-gray-500 text-xs">
                  Help improve the app with usage data
                </p>
              </div>
              <Switch
                checked={settings.privacy.analytics}
                onCheckedChange={(checked) => updatePrivacySetting("analytics", checked)}
              />
            </div>
            <Button
              variant="outline"
              className="w-full border-red-500/30 text-red-400 hover:bg-red-500/10 hover:text-red-300"
              onClick={handleClearAllData}
            >
              Clear All Data
            </Button>
          </div>
        </div>

        {/* Storage */}
        <div className="bg-gradient-to-br from-white/5 to-white/[0.02] rounded-2xl p-6 border border-white/10">
          <div className="flex items-center gap-3 mb-4">
            <div className="bg-gradient-to-br from-yellow-500 to-orange-600 p-2 rounded-lg">
              <Database className="size-5 text-white" />
            </div>
            <h2 className="text-white font-semibold text-lg">Storage</h2>
          </div>
          <div className="space-y-4">
            <div>
              <div className="flex items-center justify-between mb-2">
                <p className="text-white text-sm">Storage Used</p>
                <p className="text-gray-400 text-sm">{storageUsed} / 10 GB</p>
              </div>
              <div className="h-2 bg-white/5 rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-blue-500 to-purple-600 rounded-full transition-all"
                  style={{ width: `${storagePercentage}%` }}
                />
              </div>
            </div>
            <div className="flex gap-2">
              <Button
                variant="outline"
                className="flex-1 border-white/20 text-white hover:bg-white/5"
                onClick={loadStorageInfo}
              >
                Refresh
              </Button>
              <Button
                variant="outline"
                className="flex-1 border-white/20 text-white hover:bg-white/5"
                onClick={handleResetSettings}
              >
                Reset Settings
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
