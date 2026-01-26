// ============================================================================
// THEMES MODULE
// Theme management commands for loading and applying custom themes
// ============================================================================

use crate::settings::AppDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Theme metadata parsed from CSS comment header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeInfo {
    /// Theme ID (filename without .css extension)
    pub id: String,
    /// Display name from @name comment
    pub name: String,
    /// Author from @author comment
    pub author: String,
    /// Version from @version comment
    pub version: String,
    /// Whether this is a built-in theme
    pub is_builtin: bool,
}

/// Get the global themes directory path
fn get_themes_dir() -> Result<PathBuf, String> {
    let global_config = AppDirs::get_global_config_dir().map_err(|e| e.to_string())?;
    let themes_dir = global_config.join("themes");
    Ok(themes_dir)
}

/// Get the built-in themes directory (in app resources)
fn get_builtin_themes_source() -> Option<PathBuf> {
    // When running in dev, templates are in src-tauri/templates
    // When bundled, they're in the resources directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    // Try bundled resources first (next to executable)
    if let Some(ref dir) = exe_dir {
        let bundled_path = dir.join("templates").join("themes");
        if bundled_path.exists() {
            return Some(bundled_path);
        }
    }

    // Try various development paths
    let dev_paths = [
        PathBuf::from("templates/themes"),
        PathBuf::from("src-tauri/templates/themes"),
        // For when CWD is project root
        std::env::current_dir()
            .ok()
            .map(|p| p.join("src-tauri/templates/themes"))
            .unwrap_or_default(),
    ];

    for path in dev_paths {
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Copy built-in themes to the themes directory if they don't exist
pub fn ensure_builtin_themes() -> Result<(), String> {
    let themes_dir = get_themes_dir()?;
    fs::create_dir_all(&themes_dir).map_err(|e| e.to_string())?;

    if let Some(source_dir) = get_builtin_themes_source() {
        copy_themes_from_dir(&source_dir, &themes_dir)?;
    } else {
        tracing::warn!("Built-in themes source not found, skipping copy");
    }

    Ok(())
}

fn copy_themes_from_dir(source_dir: &PathBuf, themes_dir: &PathBuf) -> Result<(), String> {
    let entries = fs::read_dir(source_dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "css") {
            let filename = path.file_name().unwrap();
            let dest_path = themes_dir.join(filename);

            // Only copy if destination doesn't exist (don't overwrite user customizations)
            if !dest_path.exists() {
                fs::copy(&path, &dest_path).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

/// Parse theme metadata from CSS content
fn parse_theme_metadata(css_content: &str, id: &str) -> ThemeInfo {
    let mut name = id.to_string();
    let mut author = String::new();
    let mut version = "1.0".to_string();

    // Look for metadata in CSS comment block
    if let Some(start) = css_content.find("/*") {
        if let Some(end) = css_content.find("*/") {
            let comment = &css_content[start + 2..end];

            for line in comment.lines() {
                let line = line.trim().trim_start_matches('*').trim();

                if let Some(value) = line.strip_prefix("@name:") {
                    name = value.trim().to_string();
                } else if let Some(value) = line.strip_prefix("@author:") {
                    author = value.trim().to_string();
                } else if let Some(value) = line.strip_prefix("@version:") {
                    version = value.trim().to_string();
                }
            }
        }
    }

    // Determine if it's a built-in theme
    let builtin_ids = ["default", "solarized-dark", "solarized-light", "nord", "dracula"];
    let is_builtin = builtin_ids.contains(&id);

    ThemeInfo {
        id: id.to_string(),
        name,
        author,
        version,
        is_builtin,
    }
}

/// List all available themes
#[tauri::command]
pub async fn list_themes() -> Result<Vec<ThemeInfo>, String> {
    // Ensure built-in themes are copied
    ensure_builtin_themes()?;

    let themes_dir = get_themes_dir()?;
    let mut themes = Vec::new();

    if !themes_dir.exists() {
        return Ok(themes);
    }

    let entries = fs::read_dir(&themes_dir).map_err(|e| e.to_string())?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "css") {
            if let Some(stem) = path.file_stem() {
                let id = stem.to_string_lossy().to_string();
                let content = fs::read_to_string(&path).unwrap_or_default();
                let info = parse_theme_metadata(&content, &id);
                themes.push(info);
            }
        }
    }

    // Sort: built-in first, then alphabetically by name
    themes.sort_by(|a, b| {
        match (a.is_builtin, b.is_builtin) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    Ok(themes)
}

/// Get the CSS content for a specific theme
#[tauri::command]
pub async fn get_theme_css(theme_id: String) -> Result<String, String> {
    let themes_dir = get_themes_dir()?;
    let theme_path = themes_dir.join(format!("{}.css", theme_id));

    if !theme_path.exists() {
        return Err(format!("Theme '{}' not found", theme_id));
    }

    fs::read_to_string(&theme_path).map_err(|e| e.to_string())
}

/// Get metadata for a specific theme
#[tauri::command]
pub async fn get_theme_info(theme_id: String) -> Result<ThemeInfo, String> {
    let themes_dir = get_themes_dir()?;
    let theme_path = themes_dir.join(format!("{}.css", theme_id));

    if !theme_path.exists() {
        return Err(format!("Theme '{}' not found", theme_id));
    }

    let content = fs::read_to_string(&theme_path).map_err(|e| e.to_string())?;
    Ok(parse_theme_metadata(&content, &theme_id))
}

/// Get the themes directory path (for "Open themes folder" feature)
#[tauri::command]
pub async fn get_themes_dir_path() -> Result<String, String> {
    // Ensure themes directory exists
    ensure_builtin_themes()?;

    let themes_dir = get_themes_dir()?;
    Ok(themes_dir.to_string_lossy().to_string())
}
