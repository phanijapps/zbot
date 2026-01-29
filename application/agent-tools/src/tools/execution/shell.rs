// ============================================================================
// SHELL TOOL
// Execute shell commands with platform-aware shell selection and security guardrails
// ============================================================================

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

use zero_core::{Result, Tool, ToolContext, ZeroError};

// ============================================================================
// SECURITY CONFIGURATION
// ============================================================================

/// Commands that are completely blocked - these are too dangerous
const BLOCKED_COMMANDS: &[&str] = &[
    // Disk/partition destruction
    "mkfs",
    "dd",
    "fdisk",
    "parted",
    "gdisk",
    "diskutil",
    // System destruction
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "rm -rf ~/*",
    ":(){ :|:& };:", // Fork bomb
    // Permission/ownership attacks
    "chmod -R 777 /",
    "chown -R",
    // Network attacks
    "iptables -F",
    "ufw disable",
    "netsh advfirewall set",
    // Kernel/boot attacks
    "insmod",
    "rmmod",
    "modprobe",
    // Windows destructive
    "format c:",
    "format d:",
    "del /f /s /q c:\\",
    "rd /s /q c:\\",
    "diskpart",
    // Credential theft
    "mimikatz",
    "hashcat",
    "john",
    // Privilege escalation attempts
    "sudo su",
    "sudo -i",
    "sudo bash",
    "sudo sh",
    "runas /user:administrator",
    // System modification
    "visudo",
    "passwd root",
    "usermod",
    "useradd",
    "userdel",
    "groupadd",
    "groupdel",
    // Registry attacks (Windows)
    "reg delete HKLM",
    "reg delete HKCU",
    // Process injection
    "ptrace",
    "gdb -p",
    "lldb -p",
    // Crypto mining indicators
    "xmrig",
    "minerd",
    "cgminer",
    // Reverse shells
    "nc -e",
    "ncat -e",
    "bash -i >& /dev/tcp",
    "python -c 'import socket",
    "perl -e 'use Socket",
    "ruby -rsocket",
    "php -r '$sock=fsockopen",
];

/// Command prefixes/patterns that are suspicious but context-dependent
const SUSPICIOUS_PATTERNS: &[&str] = &[
    "curl | sh",
    "curl | bash",
    "wget | sh",
    "wget | bash",
    "eval $(",
    "base64 -d",
    "> /dev/sd",
    "> /dev/nvme",
    "echo '' > /etc/",
    "cat > /etc/",
    "tee /etc/",
    "> /etc/passwd",
    "> /etc/shadow",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "init 0",
    "init 6",
    "systemctl stop",
    "systemctl disable",
    "service stop",
    "launchctl unload",
    "Stop-Service",
    "Set-ExecutionPolicy Unrestricted",
];

/// Maximum command output size (1MB)
const MAX_OUTPUT_SIZE: usize = 1024 * 1024;

/// Default timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Maximum allowed timeout in seconds (10 minutes)
const MAX_TIMEOUT_SECS: u64 = 600;

// ============================================================================
// SHELL TOOL
// ============================================================================

/// Tool for executing shell commands with security guardrails
///
/// Platform behavior:
/// - macOS/Linux: Uses zsh if available, falls back to bash, then sh
/// - Windows: Uses PowerShell, with WSL bash as fallback option
///
/// Security features:
/// - Blocks dangerous commands that could harm the system
/// - Warns on suspicious patterns
/// - Disabled when running as root/administrator
/// - Configurable timeout to prevent runaway processes
pub struct ShellTool {
    /// Whether the tool is disabled due to elevated privileges
    disabled: bool,
    /// Reason for being disabled
    disabled_reason: Option<String>,
}

impl ShellTool {
    /// Create a new Shell tool
    ///
    /// Automatically detects if running with elevated privileges and disables
    /// the tool if so for security reasons.
    #[must_use]
    pub fn new() -> Self {
        let (disabled, disabled_reason) = Self::check_elevated_privileges();
        Self {
            disabled,
            disabled_reason,
        }
    }

    /// Check if running with elevated privileges (root/sudo/administrator)
    fn check_elevated_privileges() -> (bool, Option<String>) {
        #[cfg(unix)]
        {
            use std::env;

            // Check if running as root (UID 0)
            if unsafe { libc::getuid() } == 0 {
                return (
                    true,
                    Some("Shell tool is disabled when running as root".to_string()),
                );
            }

            // Check if running under sudo
            if env::var("SUDO_USER").is_ok() {
                return (
                    true,
                    Some("Shell tool is disabled when running under sudo".to_string()),
                );
            }
        }

        #[cfg(windows)]
        {
            // Check if running as administrator on Windows
            if is_windows_admin() {
                return (
                    true,
                    Some("Shell tool is disabled when running as Administrator".to_string()),
                );
            }
        }

        (false, None)
    }

    /// Validate a command against security rules
    fn validate_command(command: &str) -> Result<()> {
        let command_lower = command.to_lowercase();
        let command_normalized = command_lower.replace("  ", " ").trim().to_string();

        // Check against blocked commands
        for blocked in BLOCKED_COMMANDS {
            if command_normalized.contains(&blocked.to_lowercase()) {
                return Err(ZeroError::Tool(format!(
                    "Command blocked for security: contains forbidden pattern '{}'",
                    blocked
                )));
            }
        }

        // Check for suspicious patterns (warn but allow)
        for pattern in SUSPICIOUS_PATTERNS {
            if command_normalized.contains(&pattern.to_lowercase()) {
                tracing::warn!(
                    "Shell command contains suspicious pattern '{}': {}",
                    pattern,
                    command
                );
            }
        }

        // Additional validation
        // Block commands that try to escape quotes or inject
        if command.contains("$(") && command.contains("rm ") {
            return Err(ZeroError::Tool(
                "Command blocked: potential command injection with rm".to_string(),
            ));
        }

        // Block backtick command substitution with dangerous commands
        if command.contains('`') && (command.contains("rm ") || command.contains("dd ")) {
            return Err(ZeroError::Tool(
                "Command blocked: potential command injection".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the appropriate shell for the current platform
    #[cfg(unix)]
    fn get_shell() -> (String, Vec<String>) {
        use std::path::Path;

        // Try zsh first (more common on macOS)
        if Path::new("/bin/zsh").exists() {
            return ("zsh".to_string(), vec!["-c".to_string()]);
        }

        // Fall back to bash
        if Path::new("/bin/bash").exists() {
            return ("bash".to_string(), vec!["-c".to_string()]);
        }

        // Last resort: sh
        ("sh".to_string(), vec!["-c".to_string()])
    }

    /// Get the appropriate shell for Windows
    #[cfg(windows)]
    fn get_shell() -> (String, Vec<String>) {
        // Use PowerShell on Windows
        ("powershell".to_string(), vec!["-Command".to_string()])
    }

    /// Truncate output if too large
    fn truncate_output(output: String, max_size: usize) -> (String, bool) {
        if output.len() > max_size {
            let truncated = output.chars().take(max_size).collect::<String>();
            (
                format!("{}\n\n[Output truncated - exceeded {} bytes]", truncated, max_size),
                true,
            )
        } else {
            (output, false)
        }
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute shell commands. Uses zsh/bash on macOS/Linux, PowerShell on Windows. \
         Has security guardrails to prevent dangerous operations. \
         Disabled when running with elevated privileges (root/administrator)."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Maximum execution time in seconds (default: 60, max: 600)",
                    "minimum": 1,
                    "maximum": 600
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command (optional)"
                },
                "use_wsl": {
                    "type": "boolean",
                    "description": "On Windows, use WSL bash instead of PowerShell (default: false)"
                }
            },
            "required": ["command"]
        }))
    }

    async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: Value) -> Result<Value> {
        // Check if tool is disabled due to elevated privileges
        if self.disabled {
            return Err(ZeroError::Tool(
                self.disabled_reason
                    .clone()
                    .unwrap_or_else(|| "Shell tool is disabled".to_string()),
            ));
        }

        // Extract parameters
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZeroError::Tool("Missing 'command' parameter".to_string()))?;

        let timeout_seconds = args
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(MAX_TIMEOUT_SECS);

        let cwd = args.get("cwd").and_then(|v| v.as_str());

        #[cfg(windows)]
        let use_wsl = args
            .get("use_wsl")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate command against security rules
        Self::validate_command(command)?;

        tracing::debug!(
            "Shell: executing command ({} chars) with {}s timeout",
            command.len(),
            timeout_seconds
        );

        // Get shell and arguments
        #[cfg(unix)]
        let (shell, shell_args) = Self::get_shell();

        #[cfg(windows)]
        let (shell, shell_args) = if use_wsl {
            ("wsl".to_string(), vec!["bash".to_string(), "-c".to_string()])
        } else {
            Self::get_shell()
        };

        // Build the command
        let mut cmd = Command::new(&shell);

        // Add shell args
        for arg in &shell_args {
            cmd.arg(arg);
        }
        cmd.arg(command);

        // Set sandboxed Python virtual environment
        if let Some(config_dir) = dirs::config_dir() {
            let agentzero_dir = config_dir.join("agentzero");

            // === Python Virtual Environment ===
            let venv_path = agentzero_dir.join("venv");

            // Set VIRTUAL_ENV to activate the venv
            cmd.env("VIRTUAL_ENV", &venv_path);

            // Get the venv bin/Scripts directory
            #[cfg(windows)]
            let venv_bin = venv_path.join("Scripts");
            #[cfg(not(windows))]
            let venv_bin = venv_path.join("bin");

            // Build PATH: venv/bin + original PATH
            if let Ok(current_path) = std::env::var("PATH") {
                #[cfg(windows)]
                let new_path = format!("{};{}", venv_bin.display(), current_path);
                #[cfg(not(windows))]
                let new_path = format!("{}:{}", venv_bin.display(), current_path);
                cmd.env("PATH", new_path);
            }

            // Unset PYTHONHOME to avoid conflicts with system Python
            cmd.env_remove("PYTHONHOME");
        }

        // Set working directory if provided
        if let Some(dir) = cwd {
            // Validate cwd doesn't contain path traversal
            if dir.contains("..") {
                return Err(ZeroError::Tool(
                    "Working directory cannot contain '..' for security".to_string(),
                ));
            }
            cmd.current_dir(dir);
        }

        // Execute with timeout
        let result = timeout(
            Duration::from_secs(timeout_seconds),
            cmd.output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr_raw = String::from_utf8_lossy(&output.stderr).to_string();

                let (stdout, stdout_truncated) = Self::truncate_output(stdout_raw, MAX_OUTPUT_SIZE);
                let (stderr, stderr_truncated) = Self::truncate_output(stderr_raw, MAX_OUTPUT_SIZE);

                let exit_code = output.status.code().unwrap_or(-1);
                let success = output.status.success();

                tracing::debug!(
                    "Shell: command completed with exit code {} (stdout: {} bytes, stderr: {} bytes)",
                    exit_code,
                    stdout.len(),
                    stderr.len()
                );

                Ok(json!({
                    "success": success,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                    "truncated": stdout_truncated || stderr_truncated,
                    "shell": shell,
                }))
            }
            Ok(Err(e)) => {
                Err(ZeroError::Tool(format!("Failed to execute command: {}", e)))
            }
            Err(_) => {
                Err(ZeroError::Tool(format!(
                    "Command timed out after {} seconds",
                    timeout_seconds
                )))
            }
        }
    }
}

// ============================================================================
// WINDOWS ADMIN CHECK
// ============================================================================

#[cfg(windows)]
fn is_windows_admin() -> bool {
    use std::mem;
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = ptr::null_mut() as HANDLE;

        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation: TOKEN_ELEVATION = mem::zeroed();
        let mut size: u32 = mem::size_of::<TOKEN_ELEVATION>() as u32;

        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size,
            &mut size,
        );

        CloseHandle(token);

        result != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_commands() {
        assert!(ShellTool::validate_command("rm -rf /").is_err());
        assert!(ShellTool::validate_command("mkfs.ext4 /dev/sda1").is_err());
        assert!(ShellTool::validate_command("dd if=/dev/zero of=/dev/sda").is_err());
        assert!(ShellTool::validate_command("format c:").is_err());
        assert!(ShellTool::validate_command("sudo su").is_err());
    }

    #[test]
    fn test_allowed_commands() {
        assert!(ShellTool::validate_command("ls -la").is_ok());
        assert!(ShellTool::validate_command("echo hello").is_ok());
        assert!(ShellTool::validate_command("pwd").is_ok());
        assert!(ShellTool::validate_command("cat file.txt").is_ok());
        assert!(ShellTool::validate_command("git status").is_ok());
    }

    #[test]
    fn test_output_truncation() {
        let short = "hello".to_string();
        let (result, truncated) = ShellTool::truncate_output(short, 100);
        assert!(!truncated);
        assert_eq!(result, "hello");

        let long = "a".repeat(200);
        let (result, truncated) = ShellTool::truncate_output(long, 100);
        assert!(truncated);
        assert!(result.contains("[Output truncated"));
    }
}
