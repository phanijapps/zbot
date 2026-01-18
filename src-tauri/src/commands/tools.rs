// ============================================================================
// TOOLS COMMANDS
// Tauri commands for agent tool execution
// ============================================================================

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::settings::AppDirs;

// ============================================================================
// CONFIG
// ============================================================================

/// Shell configuration for cross-platform command execution
#[derive(Debug, Clone)]
struct ShellConfig {
    shell: String,
    arg: String,
    fallback_shell: Option<String>,
}

/// Detect the appropriate shell for the current platform
fn detect_shell() -> ShellConfig {
    #[cfg(target_os = "windows")]
    {
        // Try PowerShell first, then fallback to WSL bash
        if Command::new("powershell")
            .arg("-Version")
            .output()
            .is_ok()
        {
            ShellConfig {
                shell: "powershell".to_string(),
                arg: "-Command".to_string(),
                fallback_shell: Some("wsl bash".to_string()),
            }
        } else {
            ShellConfig {
                shell: "cmd".to_string(),
                arg: "/C".to_string(),
                fallback_shell: None,
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        ShellConfig {
            shell: "bash".to_string(),
            arg: "-c".to_string(),
            fallback_shell: Some("sh".to_string()),
        }
    }
}

/// Get the Python interpreter path from the venv
fn get_python_venv() -> Result<PathBuf, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let venv_path = dirs.config_dir.join("venv");

    #[cfg(target_os = "windows")]
    let python_path = venv_path.join("Scripts").join("python.exe");

    #[cfg(not(target_os = "windows"))]
    let python_path = venv_path.join("bin").join("python");

    if !python_path.exists() {
        return Err(format!(
            "Python venv not found at {}. Please create it first.",
            venv_path.display()
        ));
    }

    Ok(python_path)
}

// ============================================================================
// TAURI COMMANDS
// ============================================================================

/// Read file contents with optional offset and limit
#[tauri::command]
pub async fn read_file_lines(
    path: String,
    offset: usize,
    limit: i64, // -1 means read all lines
) -> Result<String, String> {
    let path_buf = PathBuf::from(&path);

    if !path_buf.exists() {
        return Err(format!("File not found: {}", path));
    }

    let content = fs::read_to_string(&path_buf)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Calculate the range
    let start = offset.min(total_lines);
    let end = if limit < 0 {
        total_lines
    } else {
        (offset + limit as usize).min(total_lines)
    };

    if start >= total_lines {
        return Ok(String::new());
    }

    let selected_lines: Vec<&str> = lines[start..end].to_vec();
    let result = selected_lines.join("\n");

    Ok(result)
}

/// Write content to a file, creating parent directories if needed
#[tauri::command]
pub async fn write_file_with_dirs(
    path: String,
    content: String,
) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);

    // Create parent directories if they don't exist
    if let Some(parent) = path_buf.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {}", e))?;
        }
    }

    // Write the file
    fs::write(&path_buf, content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

/// Execute a shell command cross-platform
#[tauri::command]
pub async fn execute_shell_command(
    command: String,
) -> Result<String, String> {
    let shell_config = detect_shell();

    let output = Command::new(&shell_config.shell)
        .arg(&shell_config.arg)
        .arg(&command)
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        // Try fallback shell if available
        if let Some(fallback) = shell_config.fallback_shell {
            let parts: Vec<&str> = fallback.split_whitespace().collect();
            if parts.len() >= 2 {
                let fallback_output = Command::new(parts[0])
                    .args(&parts[1..])
                    .arg("-c")
                    .arg(&command)
                    .output();

                if let Ok(fallback_ok) = fallback_output {
                    if fallback_ok.status.success() {
                        let fallback_stdout = String::from_utf8_lossy(&fallback_ok.stdout).to_string();
                        let fallback_stderr = String::from_utf8_lossy(&fallback_ok.stderr).to_string();
                        return Ok(format!("{}{}", fallback_stdout, fallback_stderr));
                    }
                }
            }
        }

        return Err(format!(
            "Command failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        ));
    }

    Ok(format!("{}{}", stdout, stderr))
}

/// Execute Python code in the venv
#[tauri::command]
pub async fn execute_python_code(
    code: String,
) -> Result<String, String> {
    let python_path = get_python_venv()?;

    // Create a temporary Python script
    let temp_script = format!(
        r#"
import sys
from io import StringIO

# Capture stdout
old_stdout = sys.stdout
sys.stdout = captured = StringIO()

try:
{}

finally:
    # Restore stdout
    sys.stdout = old_stdout
    # Get captured output
    output = captured.getvalue()
    print(output, end='')
"#,
        code
    );

    let output = Command::new(&python_path)
        .arg("-c")
        .arg(&temp_script)
        .output()
        .map_err(|e| format!("Failed to execute Python: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Python execution failed: {}", stderr));
    }

    Ok(stdout)
}

/// Grep files for a pattern
#[tauri::command]
pub async fn grep_files(
    pattern: String,
    path: String,
    recursive: bool,
    case_insensitive: bool,
    context_before: usize,
    context_after: usize,
    max_results: usize,
) -> Result<String, String> {
    use regex::Regex;

    let path_buf = PathBuf::from(&path);

    if !path_buf.exists() {
        return Err(format!("Path not found: {}", path));
    }

    // Build regex
    let regex = Regex::new(&format!(
        "(?{}){}",
        if case_insensitive { "i" } else { "" },
        &pattern
    )).map_err(|e| format!("Invalid regex pattern: {}", e))?;

    let mut results = Vec::new();
    let mut match_count = 0;

    // Collect files to search
    let mut files_to_search: Vec<PathBuf> = Vec::new();

    if path_buf.is_file() {
        files_to_search.push(path_buf);
    } else {
        collect_files_for_grep(&path_buf, &mut files_to_search, recursive);
    }

    // Search each file
    for file_path in files_to_search {
        if match_count >= max_results {
            break;
        }

        match fs::read_to_string(&file_path) {
            Ok(content) => {
                for (line_num, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        match_count += 1;

                        // Add context lines
                        let content_lines: Vec<&str> = content.lines().collect();
                        let start = line_num.saturating_sub(context_before);
                        let end = (line_num + context_after + 1).min(content_lines.len());

                        for ctx_line in start..end {
                            let marker = if ctx_line == line_num { ">>>" } else { "   " };
                            let context_suffix = if ctx_line == line_num { "" } else { " (context)" };
                            results.push(format!(
                                "{} {}: {}{}",
                                marker,
                                ctx_line + 1,
                                file_path.display(),
                                context_suffix
                            ));
                            results.push(format!("    {}", content_lines[ctx_line]));
                        }
                        results.push(String::new()); // Empty line separator

                        if match_count >= max_results {
                            break;
                        }
                    }
                }
            }
            Err(_) => continue, // Skip files we can't read
        }
    }

    if results.is_empty() {
        return Ok(String::new());
    }

    Ok(results.join("\n"))
}

/// Helper function to collect files for grep
fn collect_files_for_grep(dir: &PathBuf, files: &mut Vec<PathBuf>, recursive: bool) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        if path.is_file() {
            // Only search text files
            if is_text_file(&path) {
                files.push(path);
            }
        } else if recursive && path.is_dir() {
            collect_files_for_grep(&path, files, recursive);
        }
    }
}

/// Check if a file is a text file
fn is_text_file(path: &PathBuf) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        matches!(
            ext_str.as_str(),
            "txt" | "md" | "js" | "ts" | "tsx" | "jsx" | "rs"
                | "toml" | "yaml" | "yml" | "json" | "xml"
                | "html" | "css" | "scss" | "py" | "sh"
        )
    } else {
        // No extension, check if it's likely text by reading first bytes
        is_likely_text(path)
    }
}

/// Check if a file is likely text by reading first bytes
fn is_likely_text(path: &PathBuf) -> bool {
    match fs::read(path) {
        Ok(contents) => {
            // Check first 512 bytes for non-text characters
            let check_bytes = contents.iter().take(512);
            for byte in check_bytes {
                if *byte < 9 || (*byte > 13 && *byte < 32) || *byte == 127 {
                    return false;
                }
            }
            true
        }
        Err(_) => false,
    }
}

/// Glob files matching a pattern
#[tauri::command]
pub async fn glob_files(
    pattern: String,
    path: String,
    include_hidden: bool,
) -> Result<String, String> {
    use glob::glob;

    let search_path = PathBuf::from(&path);

    if !search_path.exists() {
        return Err(format!("Path not found: {}", path));
    }

    // Build the full glob pattern
    let full_pattern = if search_path.is_absolute() {
        format!("{}/{}", search_path.display(), pattern)
    } else {
        format!("{}/{}", std::env::current_dir().unwrap().display(), pattern)
    };

    let mut results = Vec::new();

    match glob(&full_pattern) {
        Ok(paths) => {
            for entry in paths.flatten() {
                let file_name = entry.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Skip hidden files unless requested
                if !include_hidden && file_name.starts_with('.') {
                    continue;
                }

                results.push(entry.display().to_string());
            }
        }
        Err(e) => {
            return Err(format!("Invalid glob pattern: {}", e));
        }
    }

    if results.is_empty() {
        return Ok(String::new());
    }

    results.sort();
    Ok(results.join("\n"))
}

/// Write content to the attachments directory for a conversation
/// Supports both text and base64-encoded binary content
#[tauri::command]
pub async fn write_attachment_file(
    conversation_id: String,
    filename: String,
    content: String,
    is_base64: bool,
) -> Result<String, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let attachments_dir = dirs.conversation_dir(&conversation_id).join("attachments");

    // Create attachments directory if it doesn't exist
    if !attachments_dir.exists() {
        fs::create_dir_all(&attachments_dir)
            .map_err(|e| format!("Failed to create attachments directory: {}", e))?;
    }

    let file_path = attachments_dir.join(&filename);

    // Decode base64 if needed
    let final_content = if is_base64 {
        use base64::prelude::*;
        BASE64_STANDARD.decode(&content)
            .map_err(|e| format!("Failed to decode base64 content: {}", e))?
    } else {
        content.into_bytes()
    };

    // Write the file
    fs::write(&file_path, final_content)
        .map_err(|e| format!("Failed to write attachment file: {}", e))?;

    // Set permissions to 644 (rw-r--r--) so other users on the machine can read
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&file_path)
            .map_err(|e| format!("Failed to get file metadata: {}", e))?
            .permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file_path, perms)
            .map_err(|e| format!("Failed to set file permissions: {}", e))?;
    }

    // Return the relative path (conversation_id/attachments/filename)
    Ok(format!("{}/attachments/{}", conversation_id, filename))
}

/// Read content from an attachment file
/// Returns base64-encoded content for binary files, plain text for text files
#[tauri::command]
pub async fn read_attachment_file(
    conversation_id: String,
    filename: String,
) -> Result<String, String> {
    let dirs = AppDirs::get().map_err(|e| e.to_string())?;
    let attachments_dir = dirs.conversation_dir(&conversation_id).join("attachments");
    let file_path = attachments_dir.join(&filename);

    tracing::info!("=== read_attachment_file ===");
    tracing::info!("conversation_id: {}", conversation_id);
    tracing::info!("filename: {}", filename);
    tracing::info!("attachments_dir: {}", attachments_dir.display());
    tracing::info!("file_path: {}", file_path.display());
    tracing::info!("file exists: {}", file_path.exists());

    // List files in attachments dir for debugging
    if let Ok(entries) = std::fs::read_dir(&attachments_dir) {
        let files: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.file_name().into_string().ok()).flatten().collect();
        tracing::info!("files in attachments_dir: {:?}", files);
    } else {
        tracing::info!("attachments_dir doesn't exist or can't be read");
    }

    if !file_path.exists() {
        return Err(format!("Attachment file not found: {}", file_path.display()));
    }

    // Read the file
    let content = fs::read(&file_path)
        .map_err(|e| format!("Failed to read attachment file: {}", e))?;

    // Check if content is valid UTF-8 (text) or binary
    // For text files (HTML, markdown, etc.), return as string
    // For binary files (images, PDFs), return as base64
    let is_text = String::from_utf8(content.clone()).is_ok();

    if is_text {
        // Return as plain text
        String::from_utf8(content)
            .map_err(|e| format!("Attachment file contains invalid UTF-8: {}", e))
    } else {
        // Return as base64 for binary content
        use base64::prelude::*;
        Ok(BASE64_STANDARD.encode(&content))
    }
}
