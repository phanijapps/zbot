// ============================================================================
// SETTINGS FEATURE - Types
// TypeScript types for settings
// ============================================================================

/** Application settings structure */
export interface Settings {
  appearance: AppearanceSettings;
  performance: PerformanceSettings;
  notifications: NotificationSettings;
  privacy: PrivacySettings;
  default_provider: string;
}

/** Appearance settings */
export interface AppearanceSettings {
  dark_mode: boolean;
  theme: string;
  font_size: string;
}

/** Performance settings */
export interface PerformanceSettings {
  hardware_acceleration: boolean;
  stream_responses: boolean;
}

/** Notification settings */
export interface NotificationSettings {
  desktop_notifications: boolean;
  sound_effects: boolean;
}

/** Privacy settings */
export interface PrivacySettings {
  save_chat_history: boolean;
  analytics: boolean;
}

/** Storage information */
export interface StorageInfo {
  total_used: number;
  database_size: number;
  agents_size: number;
  skills_size: number;
}

/** Application directories information */
export interface DirectoriesInfo {
  config_dir: string;
  settings_file: string;
  database_path: string;
  agents_dir: string;
  skills_dir: string;
  venv_dir: string;
}

/** Default settings values */
export const defaultSettings: Settings = {
  appearance: {
    dark_mode: true,
    theme: "default",
    font_size: "medium",
  },
  performance: {
    hardware_acceleration: true,
    stream_responses: true,
  },
  notifications: {
    desktop_notifications: true,
    sound_effects: false,
  },
  privacy: {
    save_chat_history: true,
    analytics: false,
  },
  default_provider: "openai",
};
