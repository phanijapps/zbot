use agent_primitives::vault_paths::VaultPaths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io, path::PathBuf, sync::Arc};
use tokio::io::AsyncReadExt;

pub const MAX_CONFIG_BYTES: usize = 256 * 1024;

/// Immutable per-invocation configuration. Share the same Arc with descendants.
#[derive(Debug)]
pub struct HookSnapshot {
    revision: String,
    hooks: Vec<HookDefinition>,
}

impl HookSnapshot {
    /// Read exactly once at accepted root ingress, before activating execution.
    /// Diagnostics deliberately contain neither file content nor command paths.
    pub async fn load(
        paths: &VaultPaths,
        project_roots: &[PathBuf],
    ) -> Result<Arc<Self>, HookConfigError> {
        let canonical = match tokio::fs::canonicalize(paths.hooks_config()).await {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A broken configuration symlink is invalid, rather than absent.
                if tokio::fs::symlink_metadata(paths.hooks_config())
                    .await
                    .is_ok()
                {
                    return Err(HookConfigError::Unreadable);
                }
                return Ok(Arc::new(Self::from_bytes(b"", Vec::new())));
            }
            Err(_) => return Err(HookConfigError::Unreadable),
        };
        for root in project_roots {
            match tokio::fs::canonicalize(root).await {
                Ok(root) if canonical.starts_with(&root) => {
                    return Err(HookConfigError::ProjectFile)
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return Err(HookConfigError::Unreadable),
            }
        }
        let mut options = tokio::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = options
            .open(&canonical)
            .await
            .map_err(|_| HookConfigError::Unreadable)?;
        let metadata = file
            .metadata()
            .await
            .map_err(|_| HookConfigError::Unreadable)?;
        if !metadata.is_file() {
            return Err(HookConfigError::UntrustedFile);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // SAFETY: geteuid has no arguments, memory access, or failure mode.
            let operator_uid = unsafe { libc::geteuid() };
            if (metadata.uid() != operator_uid && metadata.uid() != 0)
                || metadata.mode() & 0o022 != 0
            {
                return Err(HookConfigError::UntrustedFile);
            }
        }
        if metadata.len() > MAX_CONFIG_BYTES as u64 {
            return Err(HookConfigError::TooLarge);
        }
        let mut bytes = Vec::new();
        file.take((MAX_CONFIG_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| HookConfigError::Unreadable)?;
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(HookConfigError::TooLarge);
        }
        let config: ConfigFile =
            serde_json::from_slice(&bytes).map_err(|_| HookConfigError::InvalidConfig)?;
        if config.version != 1 || config.hooks.len() > 100 {
            return Err(HookConfigError::InvalidConfig);
        }
        let mut ids = HashSet::new();
        for hook in &config.hooks {
            if !valid_id(&hook.id)
                || !ids.insert(hook.id.as_str())
                || hook.command.is_empty()
                || hook.command.len() > 64
                || hook
                    .command
                    .iter()
                    .any(|arg| arg.is_empty() || arg.chars().count() > 4096 || arg.contains('\0'))
                || !(1..=300_000).contains(&hook.timeout_ms)
                || (hook.on_failure == HookFailurePolicy::Block && !hook.event.allows_block())
            {
                return Err(HookConfigError::InvalidConfig);
            }
        }
        Ok(Arc::new(Self::from_bytes(&bytes, config.hooks)))
    }

    fn from_bytes(bytes: &[u8], hooks: Vec<HookDefinition>) -> Self {
        Self {
            revision: format!("{:x}", Sha256::digest(bytes)),
            hooks,
        }
    }
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn hooks(&self) -> &[HookDefinition] {
        &self.hooks
    }
    pub fn matching(&self, event: HookEvent) -> impl Iterator<Item = &HookDefinition> {
        self.hooks
            .iter()
            .filter(move |hook| hook.enabled && hook.event == event)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(rename = "$schema", default, deserialize_with = "schema_reference")]
    _schema: Option<String>,
    #[serde(deserialize_with = "schema_integer")]
    version: u64,
    hooks: Vec<HookDefinition>,
}

fn schema_reference<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

/// JSON Schema treats numeric 1.0 as integer 1, independently of lexical spelling.
pub(super) fn schema_integer<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    let number = serde_json::Number::deserialize(deserializer)?;
    if let Some(value) = number.as_u64() {
        return Ok(value);
    }
    if let Some(value) = number
        .as_f64()
        .filter(|value| *value >= 0.0 && value.fract() == 0.0 && *value < u64::MAX as f64)
    {
        return Ok(value as u64);
    }
    Err(serde::de::Error::custom("expected a non-negative integer"))
}
fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn enabled_default() -> bool {
    true
}
fn timeout_default() -> u64 {
    5000
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookDefinition {
    pub id: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    pub event: HookEvent,
    pub command: Vec<String>,
    #[serde(default)]
    pub cwd: HookCwd,
    #[serde(default = "timeout_default", deserialize_with = "schema_integer")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub on_failure: HookFailurePolicy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", try_from = "String")]
pub enum HookEvent {
    SessionStart,
    UserPrompt,
    RunStart,
    RunEnd,
    BeforeModel,
    AfterModel,
    BeforeTool,
    AfterTool,
    InvalidToolCall,
}
impl TryFrom<String> for HookEvent {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "session_start" => Ok(Self::SessionStart),
            "user_prompt" => Ok(Self::UserPrompt),
            "run_start" => Ok(Self::RunStart),
            "run_end" => Ok(Self::RunEnd),
            "before_model" => Ok(Self::BeforeModel),
            "after_model" => Ok(Self::AfterModel),
            "before_tool" => Ok(Self::BeforeTool),
            "after_tool" => Ok(Self::AfterTool),
            "invalid_tool_call" => Ok(Self::InvalidToolCall),
            _ => Err("unsupported event"),
        }
    }
}
impl HookEvent {
    pub fn allows_block(self) -> bool {
        matches!(
            self,
            Self::SessionStart
                | Self::UserPrompt
                | Self::RunStart
                | Self::BeforeModel
                | Self::BeforeTool
                | Self::InvalidToolCall
        )
    }
    pub fn allows_context(self) -> bool {
        matches!(
            self,
            Self::SessionStart | Self::UserPrompt | Self::RunStart | Self::BeforeModel
        )
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", try_from = "String")]
pub enum HookCwd {
    #[default]
    Vault,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", try_from = "String")]
pub enum HookFailurePolicy {
    #[default]
    Continue,
    Block,
}
impl TryFrom<String> for HookCwd {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "vault" => Ok(Self::Vault),
            _ => Err("unsupported cwd"),
        }
    }
}
impl TryFrom<String> for HookFailurePolicy {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "continue" => Ok(Self::Continue),
            "block" => Ok(Self::Block),
            _ => Err("unsupported failure policy"),
        }
    }
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HookConfigError {
    #[error("External hooks configuration is invalid")]
    InvalidConfig,
    #[error("External hooks configuration exceeds 256 KiB")]
    TooLarge,
    #[error("External hooks configuration is not a trusted operator file")]
    UntrustedFile,
    #[error("External hooks configuration is inside a project directory")]
    ProjectFile,
    #[error("External hooks configuration cannot be read")]
    Unreadable,
}

#[cfg(test)]
mod tests;
