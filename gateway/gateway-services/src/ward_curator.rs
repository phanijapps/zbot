//! # Ward curator — Layer-1 heuristic cleanup
//!
//! Spec: `memory-bank/future-state/2026-05-23-ward-curator-spec.md` §2.
//!
//! Operates on agent-created, non-pinned wards. Inactive longer than
//! `archive_days` → archived (directory moved to `_archive/`); inactive
//! longer than `stale_days` → marked stale (sidecar only); recently active
//! again → reactivated. Bundled / user-authored wards are never touched.
//!
//! Backups are written as `.tar.gz` via the system `tar` command before any
//! mutation. Per-run audit logs land under `<data_dir>/curator_logs/<ts>/`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::ward_usage::{WardProvenance, WardRecord, WardState, WardUsage};

pub const DEFAULT_STALE_DAYS: i64 = 30;
pub const DEFAULT_ARCHIVE_DAYS: i64 = 90;
pub const DEFAULT_BACKUP_KEEP: usize = 5;

/// `POST /api/curator/cleanup` body. All fields optional — defaults match
/// Hermes (`stale=30d`, `archive=90d`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CleanupRequest {
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub stale_days: Option<i64>,
    #[serde(default)]
    pub archive_days: Option<i64>,
}

/// One state transition produced by a cleanup pass.
#[derive(Debug, Clone, Serialize)]
pub struct Transition {
    pub ward: String,
    pub from: WardState,
    pub to: WardState,
    pub anchor: DateTime<Utc>,
    pub age_days: i64,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_path: Option<PathBuf>,
}

/// JSON returned by the cleanup endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct CleanupReport {
    pub ok: bool,
    pub ran_at: DateTime<Utc>,
    pub dry_run: bool,
    pub scanned: usize,
    pub skipped_pinned: usize,
    pub skipped_non_agent: usize,
    pub transitions: Vec<Transition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_path: Option<PathBuf>,
}

/// `POST /api/curator/restore` body.
#[derive(Debug, Clone, Deserialize)]
pub struct RestoreRequest {
    pub backup: String,
}

/// JSON returned by the restore endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub ok: bool,
    pub restored_at: DateTime<Utc>,
    pub backup: String,
}

/// Layer-1 ward curator. Stateless across calls — every request reads the
/// current sidecar and walks the filesystem fresh.
pub struct WardCurator {
    wards_dir: PathBuf,
    audit_dir: PathBuf,
}

impl WardCurator {
    /// `wards_dir` is `<vault>/wards/`; `data_dir` is `<vault>/data/`. The
    /// audit log lives at `<data_dir>/curator_logs/<ts>/`.
    pub fn new(wards_dir: impl Into<PathBuf>, data_dir: impl AsRef<Path>) -> Self {
        let audit_dir = data_dir.as_ref().join("curator_logs");
        Self {
            wards_dir: wards_dir.into(),
            audit_dir,
        }
    }

    /// Execute a cleanup pass. Returns the run report; mutations are
    /// skipped entirely when `req.dry_run` is true or there are no
    /// pending transitions.
    pub fn cleanup(&self, req: &CleanupRequest) -> Result<CleanupReport, String> {
        let ran_at = Utc::now();
        let stale_days = req.stale_days.unwrap_or(DEFAULT_STALE_DAYS);
        let archive_days = req.archive_days.unwrap_or(DEFAULT_ARCHIVE_DAYS);

        let usage = WardUsage::new(&self.wards_dir);
        let map = usage.load();

        let mut scanned = 0usize;
        let mut skipped_pinned = 0usize;
        let mut skipped_non_agent = 0usize;
        let mut planned: Vec<Transition> = Vec::new();

        for (name, rec) in &map {
            scanned += 1;
            if rec.created_by != WardProvenance::Agent {
                skipped_non_agent += 1;
                continue;
            }
            if rec.pinned {
                skipped_pinned += 1;
                continue;
            }
            let anchor = max_anchor(rec);
            let age_days = (ran_at - anchor).num_days();
            let next = compute_transition(rec.state, age_days, stale_days, archive_days);
            if next == rec.state {
                continue;
            }
            planned.push(Transition {
                ward: name.clone(),
                from: rec.state,
                to: next,
                anchor,
                age_days,
                reason: format!("no activity in {age_days}d"),
                archive_path: None,
            });
        }

        // Dry-run or no-op — return without touching anything.
        if req.dry_run || planned.is_empty() {
            return Ok(CleanupReport {
                ok: true,
                ran_at,
                dry_run: req.dry_run,
                scanned,
                skipped_pinned,
                skipped_non_agent,
                transitions: planned,
                backup_path: None,
                report_path: None,
            });
        }

        // Backup before any mutation, then prune older backups.
        let backup_path = self.write_backup(ran_at)?;
        self.prune_backups();

        // Apply transitions.
        let mut map_mut = map;
        let mut applied: Vec<Transition> = Vec::with_capacity(planned.len());
        for mut t in planned {
            if let Some(rec) = map_mut.get_mut(&t.ward) {
                rec.state = t.to;
                if t.to == WardState::Archived {
                    rec.archived_at = Some(ran_at);
                    let from = self.wards_dir.join(&t.ward);
                    let archive_root = self.wards_dir.join("_archive");
                    if let Err(e) = std::fs::create_dir_all(&archive_root) {
                        tracing::warn!(error = %e, "create _archive dir failed");
                    }
                    let to = archive_root.join(&t.ward);
                    match std::fs::rename(&from, &to) {
                        Ok(()) => {
                            t.archive_path = Some(to);
                        }
                        Err(e) => {
                            // The sidecar state change is kept — the user
                            // can re-archive from the next pass or restore
                            // from the backup if this was unexpected.
                            tracing::warn!(
                                ward = %t.ward,
                                error = %e,
                                "archive rename failed; sidecar state still updated"
                            );
                        }
                    }
                }
            }
            applied.push(t);
        }
        usage.save(&map_mut)?;

        let mut report = CleanupReport {
            ok: true,
            ran_at,
            dry_run: false,
            scanned,
            skipped_pinned,
            skipped_non_agent,
            transitions: applied,
            backup_path: Some(backup_path),
            report_path: None,
        };
        report.report_path = Some(self.write_audit(&report)?);
        Ok(report)
    }

    /// Restore the `wards/` tree from a named backup. The timestamp `backup`
    /// must match a file under `_curator_backups/<backup>.tar.gz`.
    pub fn restore(&self, backup: &str) -> Result<RestoreReport, String> {
        if backup.contains('/') || backup.contains("..") {
            return Err("invalid backup name".to_string());
        }
        let archive = self
            .wards_dir
            .join("_curator_backups")
            .join(format!("{backup}.tar.gz"));
        if !archive.exists() {
            return Err(format!("backup not found: {}", archive.display()));
        }
        let parent = self
            .wards_dir
            .parent()
            .ok_or_else(|| "wards_dir has no parent".to_string())?;
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(parent)
            .status()
            .map_err(|e| format!("tar spawn: {e}"))?;
        if !status.success() {
            return Err(format!("tar exited with status {status}"));
        }
        Ok(RestoreReport {
            ok: true,
            restored_at: Utc::now(),
            backup: backup.to_string(),
        })
    }

    fn write_backup(&self, ts: DateTime<Utc>) -> Result<PathBuf, String> {
        let backup_root = self.wards_dir.join("_curator_backups");
        std::fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
        let stamp = ts.format("%Y%m%dT%H%M%SZ").to_string();
        let dest = backup_root.join(format!("{stamp}.tar.gz"));

        let parent = self
            .wards_dir
            .parent()
            .ok_or_else(|| "wards_dir has no parent".to_string())?;
        let name = self
            .wards_dir
            .file_name()
            .ok_or_else(|| "wards_dir has no name".to_string())?;

        // Exclude the backup dir itself (would recursively grow) and the
        // archive dir (already-archived wards are recoverable from prior
        // snapshots — including them doubles disk use per pass).
        let status = std::process::Command::new("tar")
            .arg("--exclude=_curator_backups")
            .arg("--exclude=_archive")
            .arg("-czf")
            .arg(&dest)
            .arg("-C")
            .arg(parent)
            .arg(name)
            .status()
            .map_err(|e| format!("tar spawn: {e}"))?;
        if !status.success() {
            return Err(format!("tar exited with status {status}"));
        }
        Ok(dest)
    }

    fn prune_backups(&self) {
        let backup_root = self.wards_dir.join("_curator_backups");
        let Ok(entries) = std::fs::read_dir(&backup_root) else {
            return;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.ends_with(".tar.gz"))
            })
            .collect();
        // Timestamps sort lexically; oldest first.
        files.sort();
        let drop_count = files.len().saturating_sub(DEFAULT_BACKUP_KEEP);
        for old in &files[..drop_count] {
            if let Err(e) = std::fs::remove_file(old) {
                tracing::warn!(path = %old.display(), error = %e, "prune backup failed");
            }
        }
    }

    fn write_audit(&self, report: &CleanupReport) -> Result<PathBuf, String> {
        let stamp = report.ran_at.format("%Y%m%dT%H%M%SZ").to_string();
        let dir = self.audit_dir.join(&stamp);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("run.json"), json).map_err(|e| e.to_string())?;
        let md = render_report_md(report);
        let md_path = dir.join("REPORT.md");
        std::fs::write(&md_path, md).map_err(|e| e.to_string())?;
        Ok(md_path)
    }
}

fn max_anchor(rec: &WardRecord) -> DateTime<Utc> {
    let mut anchor = rec.created_at;
    if let Some(t) = rec.last_used_at {
        if t > anchor {
            anchor = t;
        }
    }
    if let Some(t) = rec.last_patched_at {
        if t > anchor {
            anchor = t;
        }
    }
    anchor
}

fn compute_transition(
    state: WardState,
    age_days: i64,
    stale_days: i64,
    archive_days: i64,
) -> WardState {
    if age_days > archive_days {
        WardState::Archived
    } else if age_days > stale_days && state == WardState::Active {
        WardState::Stale
    } else if age_days <= stale_days && state == WardState::Stale {
        WardState::Active
    } else {
        state
    }
}

fn render_report_md(report: &CleanupReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Ward curator run — {}\n\n",
        report.ran_at.to_rfc3339()
    ));
    out.push_str(&format!("- dry_run: {}\n", report.dry_run));
    out.push_str(&format!("- scanned: {}\n", report.scanned));
    out.push_str(&format!("- skipped_pinned: {}\n", report.skipped_pinned));
    out.push_str(&format!(
        "- skipped_non_agent: {}\n",
        report.skipped_non_agent
    ));
    if let Some(p) = &report.backup_path {
        out.push_str(&format!("- backup_path: `{}`\n", p.display()));
    }
    out.push_str("\n## Transitions\n\n");
    if report.transitions.is_empty() {
        out.push_str("_None._\n");
    } else {
        out.push_str("| ward | from | to | age (days) | archive_path |\n");
        out.push_str("|---|---|---|---|---|\n");
        for t in &report.transitions {
            let from = format!("{:?}", t.from).to_lowercase();
            let to = format!("{:?}", t.to).to_lowercase();
            let ap = t
                .archive_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                t.ward, from, to, t.age_days, ap
            ));
        }
    }
    out
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ward_usage::WardUsage;
    use chrono::Duration;

    fn write_record(
        usage: &WardUsage,
        ward: &str,
        provenance: WardProvenance,
        state: WardState,
        last_used: Option<DateTime<Utc>>,
        pinned: bool,
    ) {
        usage.mark_created(ward, provenance).unwrap();
        usage.set_state(ward, state).unwrap();
        if pinned {
            usage.set_pinned(ward, true).unwrap();
        }
        // Back-date both `created_at` and `last_used_at` so the anchor
        // (max of the three) matches the simulated age — without this,
        // `mark_created`'s `created_at = now` dominates and ages out to 0.
        let mut map = usage.load();
        if let Some(rec) = map.get_mut(ward) {
            rec.last_used_at = last_used;
            if let Some(t) = last_used {
                rec.created_at = t;
            }
        }
        usage.save(&map).unwrap();
    }

    #[test]
    fn compute_transition_table() {
        // Active + within stale window → stay active.
        assert_eq!(
            compute_transition(WardState::Active, 10, 30, 90),
            WardState::Active
        );
        // Active + past stale, within archive → stale.
        assert_eq!(
            compute_transition(WardState::Active, 31, 30, 90),
            WardState::Stale
        );
        // Active + past archive → archived (skips stale).
        assert_eq!(
            compute_transition(WardState::Active, 91, 30, 90),
            WardState::Archived
        );
        // Stale + past archive → archived.
        assert_eq!(
            compute_transition(WardState::Stale, 91, 30, 90),
            WardState::Archived
        );
        // Stale + back within stale window → reactivate.
        assert_eq!(
            compute_transition(WardState::Stale, 5, 30, 90),
            WardState::Active
        );
        // Archived → stays archived.
        assert_eq!(
            compute_transition(WardState::Archived, 5, 30, 90),
            WardState::Archived
        );
    }

    #[test]
    fn dry_run_returns_plan_without_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let wards_dir = dir.path().join("wards");
        std::fs::create_dir_all(&wards_dir).unwrap();
        let usage = WardUsage::new(&wards_dir);
        write_record(
            &usage,
            "old-agent",
            WardProvenance::Agent,
            WardState::Active,
            Some(Utc::now() - Duration::days(120)),
            false,
        );
        let curator = WardCurator::new(&wards_dir, dir.path().join("data"));
        let report = curator
            .cleanup(&CleanupRequest {
                dry_run: true,
                ..Default::default()
            })
            .unwrap();
        assert!(report.dry_run);
        assert_eq!(report.transitions.len(), 1);
        assert_eq!(report.transitions[0].to, WardState::Archived);
        // Sidecar unchanged.
        assert_eq!(
            usage.get("old-agent").unwrap().state,
            WardState::Active,
            "dry-run must not mutate"
        );
        // No backup written.
        assert!(report.backup_path.is_none());
        assert!(!wards_dir.join("_curator_backups").exists());
    }

    #[test]
    fn live_run_archives_old_agent_ward() {
        let dir = tempfile::tempdir().unwrap();
        let wards_dir = dir.path().join("wards");
        std::fs::create_dir_all(wards_dir.join("old-agent")).unwrap();
        std::fs::write(wards_dir.join("old-agent/AGENTS.md"), "# old-agent").unwrap();
        let usage = WardUsage::new(&wards_dir);
        write_record(
            &usage,
            "old-agent",
            WardProvenance::Agent,
            WardState::Active,
            Some(Utc::now() - Duration::days(200)),
            false,
        );
        let curator = WardCurator::new(&wards_dir, dir.path().join("data"));
        let report = curator.cleanup(&CleanupRequest::default()).unwrap();
        assert_eq!(report.transitions.len(), 1);
        let t = &report.transitions[0];
        assert_eq!(t.from, WardState::Active);
        assert_eq!(t.to, WardState::Archived);
        // Directory moved.
        assert!(!wards_dir.join("old-agent").exists());
        assert!(wards_dir.join("_archive/old-agent/AGENTS.md").exists());
        // Sidecar updated.
        let rec = usage.get("old-agent").unwrap();
        assert_eq!(rec.state, WardState::Archived);
        assert!(rec.archived_at.is_some());
        // Backup + audit log written.
        assert!(report.backup_path.as_ref().unwrap().exists());
        assert!(report.report_path.as_ref().unwrap().exists());
    }

    #[test]
    fn live_run_marks_stale_without_moving_dir() {
        let dir = tempfile::tempdir().unwrap();
        let wards_dir = dir.path().join("wards");
        std::fs::create_dir_all(wards_dir.join("getting-stale")).unwrap();
        let usage = WardUsage::new(&wards_dir);
        write_record(
            &usage,
            "getting-stale",
            WardProvenance::Agent,
            WardState::Active,
            Some(Utc::now() - Duration::days(45)),
            false,
        );
        let curator = WardCurator::new(&wards_dir, dir.path().join("data"));
        let report = curator.cleanup(&CleanupRequest::default()).unwrap();
        assert_eq!(report.transitions.len(), 1);
        assert_eq!(report.transitions[0].to, WardState::Stale);
        // Dir not moved.
        assert!(wards_dir.join("getting-stale").exists());
        assert!(usage.get("getting-stale").unwrap().archived_at.is_none());
    }

    #[test]
    fn skips_pinned_and_non_agent_wards() {
        let dir = tempfile::tempdir().unwrap();
        let wards_dir = dir.path().join("wards");
        std::fs::create_dir_all(&wards_dir).unwrap();
        let usage = WardUsage::new(&wards_dir);
        // Bundled — never touched.
        write_record(
            &usage,
            "wiki",
            WardProvenance::Bundled,
            WardState::Active,
            Some(Utc::now() - Duration::days(500)),
            false,
        );
        // User-authored — never touched.
        write_record(
            &usage,
            "my-notes",
            WardProvenance::User,
            WardState::Active,
            Some(Utc::now() - Duration::days(500)),
            false,
        );
        // Pinned agent ward — never touched even though it's eligible.
        write_record(
            &usage,
            "pinned-agent",
            WardProvenance::Agent,
            WardState::Active,
            Some(Utc::now() - Duration::days(500)),
            true,
        );
        let curator = WardCurator::new(&wards_dir, dir.path().join("data"));
        let report = curator
            .cleanup(&CleanupRequest {
                dry_run: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(report.transitions.len(), 0);
        assert_eq!(report.scanned, 3);
        assert_eq!(report.skipped_pinned, 1);
        assert_eq!(report.skipped_non_agent, 2);
    }

    #[test]
    fn reactivates_stale_after_recent_use() {
        let dir = tempfile::tempdir().unwrap();
        let wards_dir = dir.path().join("wards");
        std::fs::create_dir_all(&wards_dir).unwrap();
        let usage = WardUsage::new(&wards_dir);
        write_record(
            &usage,
            "back-from-dead",
            WardProvenance::Agent,
            WardState::Stale,
            Some(Utc::now() - Duration::days(2)),
            false,
        );
        let curator = WardCurator::new(&wards_dir, dir.path().join("data"));
        let report = curator.cleanup(&CleanupRequest::default()).unwrap();
        assert_eq!(report.transitions.len(), 1);
        assert_eq!(report.transitions[0].from, WardState::Stale);
        assert_eq!(report.transitions[0].to, WardState::Active);
    }

    #[test]
    fn no_op_run_writes_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let wards_dir = dir.path().join("wards");
        std::fs::create_dir_all(&wards_dir).unwrap();
        let usage = WardUsage::new(&wards_dir);
        write_record(
            &usage,
            "fresh",
            WardProvenance::Agent,
            WardState::Active,
            Some(Utc::now()),
            false,
        );
        let curator = WardCurator::new(&wards_dir, dir.path().join("data"));
        let report = curator.cleanup(&CleanupRequest::default()).unwrap();
        assert!(report.transitions.is_empty());
        assert!(report.backup_path.is_none());
        assert!(!wards_dir.join("_curator_backups").exists());
    }
}
