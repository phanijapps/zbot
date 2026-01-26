import { invoke } from "@tauri-apps/api/core";

export interface ThemeInfo {
  id: string;
  name: string;
  author: string;
  version: string;
  is_builtin: boolean;
}

export async function listThemes(): Promise<ThemeInfo[]> {
  return invoke("list_themes");
}

export async function getThemeCss(themeId: string): Promise<string> {
  return invoke("get_theme_css", { themeId });
}

export async function getThemeInfo(themeId: string): Promise<ThemeInfo> {
  return invoke("get_theme_info", { themeId });
}

export async function getThemesDirPath(): Promise<string> {
  return invoke("get_themes_dir_path");
}
