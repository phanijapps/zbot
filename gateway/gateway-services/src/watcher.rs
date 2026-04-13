//! # File Watcher
//!
//! Consolidated file watcher for hot-reloading skills, agents, and other
//! vault resources. One `FileWatcher` instance can watch multiple paths —
//! each registered path has its own callback, routed by path prefix.
//!
//! ## Transports
//!
//! - **inotify-backed (preferred):** one kernel inotify instance shared
//!   across all registered paths. Sub-second latency for file changes.
//! - **polling fallback:** if `inotify_init` fails (e.g. `EMFILE` from the
//!   `fs.inotify.max_user_instances` kernel limit), the watcher transparently
//!   falls back to a background thread that scans registered paths on an
//!   interval and compares file mtimes. Slower (5s default) but works in
//!   constrained environments (containers, systems with many IDE watchers).
//!
//! The caller doesn't choose — `start()` tries inotify first and falls back
//! silently on failure. A single warning is logged when polling kicks in.

use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

/// Callback invoked when a watched path changes. Must be `Sync` because the
/// polling fallback invokes it from its loop thread without re-wrapping.
pub type OnChangeCallback = Arc<dyn Fn(PathBuf) + Send + Sync + 'static>;

/// One registered watch entry: a root path, a human-readable name, and a
/// callback invoked on any file change under the path.
#[derive(Clone)]
pub struct WatchEntry {
    pub path: PathBuf,
    pub name: String,
    pub callback: OnChangeCallback,
}

/// Configuration for `FileWatcher`.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Debounce delay in milliseconds for the inotify path. Rapid bursts of
    /// events on the same path are coalesced within this window.
    pub debounce_ms: u64,
    /// Interval for the polling fallback. Ignored when inotify succeeds.
    pub poll_interval: Duration,
    /// If `true`, skip inotify and go straight to polling. Useful for tests
    /// and environments that explicitly want polling (e.g. networked FS).
    pub force_polling: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            poll_interval: Duration::from_secs(5),
            force_polling: false,
        }
    }
}

/// Consolidated file watcher.
///
/// Use `add_watch()` to register one or more paths with callbacks, then
/// `start()` once. All paths share a single inotify instance (or polling
/// thread). Call `stop()` or drop the watcher to release resources.
pub struct FileWatcher {
    config: WatchConfig,
    entries: Vec<WatchEntry>,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl FileWatcher {
    /// Create a new watcher with the given config. No paths are registered
    /// yet — use `add_watch()` before `start()`.
    pub fn new(config: WatchConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// Register a path to watch with a callback. May be called multiple
    /// times; each call adds another entry. Must be called before `start()`.
    pub fn add_watch<P, N, F>(&mut self, path: P, name: N, callback: F)
    where
        P: Into<PathBuf>,
        N: Into<String>,
        F: Fn(PathBuf) + Send + Sync + 'static,
    {
        self.entries.push(WatchEntry {
            path: path.into(),
            name: name.into(),
            callback: Arc::new(callback),
        });
    }

    /// Start watching. Tries inotify first; falls back to polling on failure.
    /// Idempotent — calling twice is a no-op after the first.
    pub fn start(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            tracing::warn!("FileWatcher already running");
            return;
        }
        if self.entries.is_empty() {
            tracing::warn!("FileWatcher::start called with no registered watches");
            return;
        }

        // Ensure every watched directory exists before registering.
        for entry in &self.entries {
            if !entry.path.exists() {
                tracing::warn!(
                    "Watch path does not exist, creating: {:?} ({})",
                    entry.path,
                    entry.name
                );
                if let Err(e) = std::fs::create_dir_all(&entry.path) {
                    tracing::error!("Failed to create watch directory {:?}: {}", entry.path, e);
                    // Continue with other entries; don't abort the whole watcher.
                }
            }
        }

        self.running.store(true, Ordering::SeqCst);

        let entries = self.entries.clone();
        let running = self.running.clone();
        let debounce_ms = self.config.debounce_ms;
        let poll_interval = self.config.poll_interval;
        let force_polling = self.config.force_polling;

        self.thread_handle = Some(std::thread::spawn(move || {
            if force_polling {
                tracing::info!("FileWatcher: force_polling=true, using polling transport");
                run_polling(entries, poll_interval, running);
                return;
            }

            match try_inotify(&entries, debounce_ms, running.clone()) {
                Ok(()) => {} // inotify ran until stopped
                Err(e) => {
                    tracing::warn!(
                        "FileWatcher: inotify unavailable ({}), falling back to polling every {:?}",
                        e,
                        poll_interval
                    );
                    run_polling(entries, poll_interval, running);
                }
            }
        }));
    }

    /// Signal the watcher thread to stop. Returns quickly; full teardown may
    /// take up to one poll interval if polling is active.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.thread_handle.take() {
            std::thread::sleep(Duration::from_millis(100));
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                tracing::debug!("FileWatcher thread did not finish in time; leaving to detach");
            }
        }
    }

    /// Whether the watcher is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Number of registered watch entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// inotify transport
// ============================================================================

fn try_inotify(
    entries: &[WatchEntry],
    debounce_ms: u64,
    running: Arc<AtomicBool>,
) -> Result<(), String> {
    // Shared sink for debouncer callbacks. We pass (path, entry_index) so the
    // receiver can dispatch to the right callback without a path-prefix scan.
    let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();

    let mut debouncer = new_debouncer(
        Duration::from_millis(debounce_ms),
        None,
        move |result: DebounceEventResult| {
            let Ok(events) = result else {
                return;
            };
            for event in events {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in &event.paths {
                            let _ = tx.send(path.clone());
                        }
                    }
                    _ => {}
                }
            }
        },
    )
    .map_err(|e| format!("debouncer init failed: {e}"))?;

    // Register each path; collect failures but don't abort — one bad path
    // shouldn't take out the whole watcher.
    for entry in entries {
        if let Err(e) = debouncer.watch(&entry.path, RecursiveMode::Recursive) {
            tracing::error!("Failed to watch {:?} ({}): {}", entry.path, entry.name, e);
        } else {
            tracing::info!("FileWatcher: watching {} at {:?}", entry.name, entry.path);
        }
    }

    // Receive events until stopped. Dispatch each path to the matching entry.
    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(changed_path) => {
                dispatch_event(entries, &changed_path);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Unwatch everything on shutdown.
    for entry in entries {
        let _ = debouncer.unwatch(&entry.path);
    }
    tracing::info!("FileWatcher: inotify transport stopped");
    Ok(())
}

fn dispatch_event(entries: &[WatchEntry], changed_path: &Path) {
    for entry in entries {
        if changed_path.starts_with(&entry.path) {
            tracing::debug!("FileWatcher: {} changed ({:?})", entry.name, changed_path);
            (entry.callback)(changed_path.to_path_buf());
            return;
        }
    }
}

// ============================================================================
// polling transport (fallback)
// ============================================================================

fn run_polling(entries: Vec<WatchEntry>, interval: Duration, running: Arc<AtomicBool>) {
    // Per-entry snapshot of file → mtime. We scan each entry's subtree on
    // each tick and diff against the prior snapshot.
    let mut snapshots: Vec<HashMap<PathBuf, SystemTime>> =
        entries.iter().map(|e| scan_tree(&e.path)).collect();

    for entry in &entries {
        tracing::info!(
            "FileWatcher: polling {} at {:?} every {:?}",
            entry.name,
            entry.path,
            interval
        );
    }

    while running.load(Ordering::SeqCst) {
        std::thread::sleep(interval);
        if !running.load(Ordering::SeqCst) {
            break;
        }

        for (i, entry) in entries.iter().enumerate() {
            let current = scan_tree(&entry.path);
            let changes = diff_snapshots(&snapshots[i], &current);
            snapshots[i] = current;

            for changed_path in changes {
                tracing::debug!(
                    "FileWatcher: {} polling-detected change ({:?})",
                    entry.name,
                    changed_path
                );
                (entry.callback)(changed_path);
            }
        }
    }
    tracing::info!("FileWatcher: polling transport stopped");
}

fn scan_tree(root: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut out = HashMap::new();
    scan_recursive(root, &mut out);
    out
}

fn scan_recursive(dir: &Path, out: &mut HashMap<PathBuf, SystemTime>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            scan_recursive(&path, out);
        } else if ft.is_file() {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    out.insert(path, mtime);
                }
            }
        }
    }
}

fn diff_snapshots(
    prev: &HashMap<PathBuf, SystemTime>,
    current: &HashMap<PathBuf, SystemTime>,
) -> Vec<PathBuf> {
    let mut changes = Vec::new();
    // Additions + modifications
    for (path, mtime) in current {
        match prev.get(path) {
            Some(old) if old == mtime => {}
            _ => changes.push(path.clone()),
        }
    }
    // Deletions
    for path in prev.keys() {
        if !current.contains_key(path) {
            changes.push(path.clone());
        }
    }
    changes
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let watcher = FileWatcher::new(WatchConfig::default());
        assert!(!watcher.is_running());
        assert_eq!(watcher.entry_count(), 0);
    }

    #[test]
    fn test_add_watch_increments_entry_count() {
        let tmp = TempDir::new().unwrap();
        let mut w = FileWatcher::new(WatchConfig::default());
        w.add_watch(tmp.path(), "one", |_| {});
        w.add_watch(tmp.path(), "two", |_| {});
        assert_eq!(w.entry_count(), 2);
    }

    #[tokio::test]
    async fn test_watcher_start_stop() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let mut w = FileWatcher::new(WatchConfig {
            debounce_ms: 100,
            ..Default::default()
        });
        w.add_watch(tmp.path(), "test", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        w.start();
        assert!(w.is_running());

        tokio::time::sleep(Duration::from_millis(200)).await;

        w.stop();
        assert!(!w.is_running());
    }

    #[tokio::test]
    async fn test_polling_fallback_detects_file_creation() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Force polling so the test is deterministic regardless of kernel
        // inotify availability.
        let mut w = FileWatcher::new(WatchConfig {
            debounce_ms: 100,
            poll_interval: Duration::from_millis(200),
            force_polling: true,
        });
        w.add_watch(tmp.path(), "poll-test", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        w.start();

        // Let polling thread take its baseline snapshot.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Create a file; next poll tick should detect it.
        std::fs::write(tmp.path().join("new.txt"), "hello").unwrap();

        // Wait for at least one poll interval + a small grace.
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            counter.load(Ordering::SeqCst) >= 1,
            "polling watcher did not detect file creation"
        );

        w.stop();
    }

    #[tokio::test]
    async fn test_polling_fallback_detects_modification() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("existing.txt");
        std::fs::write(&file_path, "v1").unwrap();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let mut w = FileWatcher::new(WatchConfig {
            debounce_ms: 100,
            poll_interval: Duration::from_millis(200),
            force_polling: true,
        });
        w.add_watch(tmp.path(), "mod-test", move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        w.start();

        // Baseline.
        tokio::time::sleep(Duration::from_millis(300)).await;
        // Ensure mtime advances on slow filesystems.
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(&file_path, "v2-with-more-content").unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            counter.load(Ordering::SeqCst) >= 1,
            "polling watcher did not detect modification"
        );

        w.stop();
    }

    #[tokio::test]
    async fn test_multi_watch_dispatches_to_correct_callback() {
        let tmp_a = TempDir::new().unwrap();
        let tmp_b = TempDir::new().unwrap();
        let a_count = Arc::new(AtomicUsize::new(0));
        let b_count = Arc::new(AtomicUsize::new(0));
        let a_c = a_count.clone();
        let b_c = b_count.clone();

        let mut w = FileWatcher::new(WatchConfig {
            debounce_ms: 100,
            poll_interval: Duration::from_millis(200),
            force_polling: true,
        });
        w.add_watch(tmp_a.path(), "a", move |_| {
            a_c.fetch_add(1, Ordering::SeqCst);
        });
        w.add_watch(tmp_b.path(), "b", move |_| {
            b_c.fetch_add(1, Ordering::SeqCst);
        });
        w.start();

        tokio::time::sleep(Duration::from_millis(300)).await;
        std::fs::write(tmp_a.path().join("x"), "1").unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;

        assert!(a_count.load(Ordering::SeqCst) >= 1);
        assert_eq!(b_count.load(Ordering::SeqCst), 0);

        w.stop();
    }

    #[test]
    fn test_diff_detects_new_file() {
        let mut prev = HashMap::new();
        let mut current = HashMap::new();
        current.insert(PathBuf::from("/a"), SystemTime::UNIX_EPOCH);
        let changes = diff_snapshots(&prev, &current);
        assert_eq!(changes, vec![PathBuf::from("/a")]);

        // Same file, same mtime → no change.
        prev.insert(PathBuf::from("/a"), SystemTime::UNIX_EPOCH);
        let changes = diff_snapshots(&prev, &current);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_detects_deletion() {
        let mut prev = HashMap::new();
        prev.insert(PathBuf::from("/gone"), SystemTime::UNIX_EPOCH);
        let current = HashMap::new();
        let changes = diff_snapshots(&prev, &current);
        assert_eq!(changes, vec![PathBuf::from("/gone")]);
    }
}
