use super::{HookDefinition, HookEventPayload, HookFailure, HookOutcome, HookStatus};
use agent_primitives::vault_paths::VaultPaths;
use std::{path::PathBuf, sync::atomic::AtomicBool};

#[cfg(target_os = "linux")]
use std::{sync::atomic::Ordering, time::Duration};

/// Execute one literal operator command, once. The caller composes file-order results.
pub async fn invoke_hook(
    definition: &HookDefinition,
    paths: &VaultPaths,
    forbidden_roots: &[PathBuf],
    event: &HookEventPayload,
    secrets: &[String],
    stop: &AtomicBool,
) -> HookOutcome {
    invoke_hook_with_permit(
        definition,
        paths,
        forbidden_roots,
        event,
        secrets,
        stop,
        None,
    )
    .await
}

pub(super) async fn invoke_hook_with_permit(
    definition: &HookDefinition,
    paths: &VaultPaths,
    forbidden_roots: &[PathBuf],
    event: &HookEventPayload,
    secrets: &[String],
    stop: &AtomicBool,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
) -> HookOutcome {
    let start = std::time::Instant::now();
    let mut outcome = HookOutcome {
        status: HookStatus::Skipped,
        duration_ms: 0,
        exit_code: None,
        response: None,
        failure: None,
    };
    if !definition.enabled {
        return outcome;
    }
    let result = invoke(
        definition,
        paths,
        forbidden_roots,
        event,
        secrets,
        (stop, permit),
        &mut outcome,
    )
    .await;
    outcome.duration_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    match result {
        Ok(response) => {
            outcome.status = if response.action == super::HookAction::Block {
                HookStatus::Blocked
            } else {
                HookStatus::Completed
            };
            outcome.response = Some(response);
        }
        Err(failure) => {
            outcome.status = match failure {
                HookFailure::Timeout => HookStatus::Timeout,
                HookFailure::Cancelled => HookStatus::Cancelled,
                _ => HookStatus::Failed,
            };
            outcome.failure = Some(failure);
        }
    }
    outcome
}

#[cfg(not(target_os = "linux"))]
async fn invoke(
    _: &HookDefinition,
    _: &VaultPaths,
    _: &[PathBuf],
    _: &HookEventPayload,
    _: &[String],
    _: (&AtomicBool, Option<tokio::sync::OwnedSemaphorePermit>),
    _: &mut HookOutcome,
) -> Result<super::HookResponse, HookFailure> {
    Err(HookFailure::UnsupportedHost)
}

#[cfg(target_os = "linux")]
async fn invoke(
    definition: &HookDefinition,
    paths: &VaultPaths,
    roots: &[PathBuf],
    event: &HookEventPayload,
    secrets: &[String],
    control: (&AtomicBool, Option<tokio::sync::OwnedSemaphorePermit>),
    outcome: &mut HookOutcome,
) -> Result<super::HookResponse, HookFailure> {
    let (stop, permit) = control;
    if std::fs::read_to_string("/proc/1/comm")
        .ok()
        .as_deref()
        .map(str::trim)
        != Some("systemd")
    {
        return Err(HookFailure::UnsupportedHost);
    }
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    if definition.event != event.event() || !(1..=300_000).contains(&definition.timeout_ms) {
        return Err(HookFailure::InvalidEvent);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_millis(definition.timeout_ms);
    let prepared = async {
        let input = event.encode(secrets)?;
        let command = super::trust::prepare(&definition.command, paths, roots).await?;
        Ok::<_, HookFailure>((input, command))
    };
    let (input, command) = tokio::select! {
        biased;
        _=cancelled(stop)=>return Err(HookFailure::Cancelled),
        prepared=tokio::time::timeout_at(deadline,prepared)=>prepared.map_err(|_|HookFailure::Timeout)??,
    };
    let mut builder = tokio::process::Command::new(command.program);
    builder
        .args(command.arguments)
        .current_dir(command.cwd)
        .env_clear()
        .env("PATH", super::trust::PERMITTED_PATH)
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    let child = builder.spawn().map_err(|_| HookFailure::Process)?;
    let mut process = ProcessGuard {
        pgid: child.id().map(|id| id as i32),
        child: Some(child),
        permit,
    };
    let child = process.child.as_mut().ok_or(HookFailure::Process)?;
    let mut stdin = child.stdin.take().ok_or(HookFailure::Process)?;
    let stdout = child.stdout.take().ok_or(HookFailure::Process)?;
    let stderr = child.stderr.take().ok_or(HookFailure::Process)?;
    let pgid = &mut process.pgid;
    let operation = async {
        let write = async {
            stdin
                .write_all(&input)
                .await
                .map_err(|_| HookFailure::Process)?;
            stdin.shutdown().await.map_err(|_| HookFailure::Process)?;
            drop(stdin);
            Ok::<_, HookFailure>(())
        };
        let wait = async {
            let status = child.wait().await.map_err(|_| HookFailure::Process)?;
            // Kill background holders as soon as the command exits, before pipe drain.
            if let Some(pgid) = pgid.take() {
                kill_group(pgid);
            }
            Ok::<_, HookFailure>(status)
        };
        let (_, stdout, _, status) =
            tokio::try_join!(write, read_bounded(stdout), read_bounded(stderr), wait)?;
        Ok::<_, HookFailure>((stdout, status))
    };
    let result = tokio::select! {
        biased;
        _=cancelled(stop)=>Err(HookFailure::Cancelled),
        output=tokio::time::timeout_at(deadline,operation)=>output.map_err(|_|HookFailure::Timeout).and_then(|result|result),
    };
    process.terminate().await;
    let (stdout, status) = result?;
    outcome.exit_code = status.code();
    if !status.success() {
        return Err(HookFailure::NonZeroExit);
    }
    super::HookResponse::parse(&stdout, event.event())
}

#[cfg(target_os = "linux")]
async fn read_bounded(reader: impl tokio::io::AsyncRead + Unpin) -> Result<Vec<u8>, HookFailure> {
    use tokio::io::AsyncReadExt;
    let mut output = Vec::new();
    reader
        .take((super::MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut output)
        .await
        .map_err(|_| HookFailure::Process)?;
    if output.len() > super::MAX_OUTPUT_BYTES {
        return Err(HookFailure::OutputTooLarge);
    }
    Ok(output)
}
#[cfg(target_os = "linux")]
async fn cancelled(stop: &AtomicBool) {
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
#[cfg(target_os = "linux")]
fn kill_group(pgid: i32) {
    // SAFETY: this positive pgid comes from our process_group(0) child's PID.
    // Negative PID addresses that group, never the daemon's process group.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}
#[cfg(target_os = "linux")]
struct ProcessGuard {
    pgid: Option<i32>,
    child: Option<tokio::process::Child>,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
}
#[cfg(target_os = "linux")]
impl ProcessGuard {
    async fn terminate(&mut self) {
        if let Some(pgid) = self.pgid.take() {
            kill_group(pgid);
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}
#[cfg(target_os = "linux")]
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(pgid) = self.pgid.take() {
            kill_group(pgid);
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                let permit = self.permit.take();
                runtime.spawn(async move {
                    let _ = child.wait().await;
                    drop(permit);
                });
            }
            // Without a live runtime, kill_on_drop and the host's process reaper apply.
        }
    }
}
#[cfg(test)]
mod tests;
