//! Narrow operator entry-point validation. This is not a sandbox for trusted programs.
use super::HookFailure;
use agent_primitives::vault_paths::VaultPaths;
use std::path::{Path, PathBuf};

pub(super) const PERMITTED_PATH: &str = "/usr/local/bin:/usr/bin:/bin";
pub(super) struct ManagedCommand {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub cwd: PathBuf,
}

pub(super) async fn prepare(
    command: &[String],
    paths: &VaultPaths,
    roots: &[PathBuf],
) -> Result<ManagedCommand, HookFailure> {
    let program = command
        .first()
        .filter(|program| !program.is_empty())
        .ok_or(HookFailure::UntrustedProgram)?;
    let cwd = tokio::fs::canonicalize(paths.vault_dir())
        .await
        .map_err(|_| HookFailure::UntrustedProgram)?;
    let program_path = Path::new(program);
    let resolved = if program_path.is_absolute() {
        managed_file(program_path, roots, true).await?
    } else if program_path.components().count() > 1 {
        managed_file(&cwd.join(program_path), roots, true).await?
    } else {
        let mut found = None;
        for directory in PERMITTED_PATH.split(':') {
            let candidate = Path::new(directory).join(program_path);
            if tokio::fs::symlink_metadata(&candidate).await.is_ok() {
                found = Some(managed_file(&candidate, roots, true).await?);
                break;
            }
        }
        found.ok_or(HookFailure::UntrustedProgram)?
    };
    let mut arguments = command[1..].to_vec();
    let name = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(HookFailure::UntrustedProgram)?;
    let file_form = name.strip_prefix("python").is_some_and(|version| {
        version.is_empty() || version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    }) || matches!(
        name,
        "node"
            | "nodejs"
            | "ruby"
            | "perl"
            | "php"
            | "lua"
            | "lua5.4"
            | "sh"
            | "bash"
            | "dash"
            | "zsh"
            | "Rscript"
    );
    if file_form {
        let file = arguments
            .first()
            .filter(|file| !file.is_empty() && !file.starts_with('-'))
            .ok_or(HookFailure::UntrustedProgram)?;
        let file_path = Path::new(file);
        let script = managed_file(
            &if file_path.is_absolute() {
                file_path.to_owned()
            } else {
                cwd.join(file_path)
            },
            roots,
            false,
        )
        .await?;
        arguments[0] = script
            .to_str()
            .ok_or(HookFailure::UntrustedProgram)?
            .to_owned();
    } else if matches!(
        name,
        "go" | "java"
            | "js"
            | "qjs"
            | "deno"
            | "bun"
            | "dotnet"
            | "pwsh"
            | "powershell"
            | "env"
            | "busybox"
            | "xargs"
            | "timeout"
            | "nice"
            | "nohup"
            | "sudo"
            | "doas"
            | "su"
            | "runuser"
            | "npx"
            | "npm"
            | "yarn"
            | "pnpm"
            | "corepack"
            | "uv"
            | "uvx"
            | "pipx"
    ) {
        // Their eval/module/package forms need a separate provable file mapping.
        return Err(HookFailure::UntrustedProgram);
    }
    Ok(ManagedCommand {
        program: resolved,
        arguments,
        cwd,
    })
}

async fn managed_file(
    path: &Path,
    roots: &[PathBuf],
    executable: bool,
) -> Result<PathBuf, HookFailure> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|_| HookFailure::UntrustedProgram)?;
    for root in roots {
        match tokio::fs::canonicalize(root).await {
            Ok(root) if canonical.starts_with(&root) => return Err(HookFailure::UntrustedProgram),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(HookFailure::UntrustedProgram),
        }
    }
    for parent in canonical.ancestors().skip(1) {
        if tokio::fs::symlink_metadata(parent.join(".git"))
            .await
            .is_ok()
        {
            return Err(HookFailure::UntrustedProgram);
        }
    }
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|_| HookFailure::UntrustedProgram)?;
    if !metadata.is_file() {
        return Err(HookFailure::UntrustedProgram);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid has no arguments, memory access, or failure mode.
        let uid = unsafe { libc::geteuid() };
        if (metadata.uid() != uid && metadata.uid() != 0)
            || metadata.mode() & 0o022 != 0
            || (executable && metadata.mode() & 0o111 == 0)
        {
            return Err(HookFailure::UntrustedProgram);
        }
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(canonical)
}
