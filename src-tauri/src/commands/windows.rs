// ============================================================================
// WINDOW COMMANDS
// Multi-window management commands
// ============================================================================

use tauri::Emitter;

/// Opens a new window for the skill editor
#[tauri::command]
pub fn open_skill_editor_window(window: tauri::Window) -> Result<(), String> {
    // Emit an event that the frontend can listen to
    window
        .emit("open-skill-editor", ())
        .map_err(|e| format!("Failed to open skill editor: {}", e))?;
    Ok(())
}

/// Opens a URL in an external browser
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(&url, None::<String>).map_err(|e| format!("Failed to open URL: {}", e))?;
    Ok(())
}

/// Opens a folder in the system file explorer
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(std::path::Path::new(&path))
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    Ok(())
}
