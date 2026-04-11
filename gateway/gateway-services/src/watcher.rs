//! # File Watcher
//!
//! Utility for watching directory changes with debouncing.
//!
//! Used for hot-reloading skills and agents when files are modified.

use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::mpsc;

/// Configuration for a file watcher.
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Path to watch
    pub path: PathBuf,
    /// Debounce delay in milliseconds
    pub debounce_ms: u64,
    /// Name for logging purposes
    pub name: String,
}

/// Callback type for file change events.
pub type OnChangeCallback = Box<dyn Fn(PathBuf) + Send + 'static>;

/// File watcher with debouncing support.
///
/// Watches a directory for changes and invokes a callback when files are
/// created, modified, or removed. Uses debouncing to batch rapid changes.
pub struct FileWatcher {
    config: WatchConfig,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl FileWatcher {
    /// Create a new file watcher with the given configuration.
    pub fn new(config: WatchConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }

    /// Start watching for file changes.
    ///
    /// The callback will be invoked on a separate thread when changes are detected.
    /// Only reacts to create, modify, and remove events.
    pub fn start<F>(&mut self, on_change: F)
    where
        F: Fn(PathBuf) + Send + 'static,
    {
        if self.running.load(Ordering::SeqCst) {
            tracing::warn!("File watcher for {} is already running", self.config.name);
            return;
        }

        // Ensure the directory exists
        if !self.config.path.exists() {
            tracing::warn!(
                "Watch path does not exist, creating: {:?}",
                self.config.path
            );
            if let Err(e) = std::fs::create_dir_all(&self.config.path) {
                tracing::error!("Failed to create watch directory: {}", e);
                return;
            }
        }

        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let path = self.config.path.clone();
        let name = self.config.name.clone();
        let debounce_ms = self.config.debounce_ms;

        // Channel for debounced events
        let (tx, mut rx) = mpsc::channel::<PathBuf>(64);

        // Spawn the watcher thread
        let thread_running = running.clone();
        let thread_handle = std::thread::spawn(move || {
            // Create a debounced watcher
            let mut debouncer = match new_debouncer(
                Duration::from_millis(debounce_ms),
                None,
                move |result: DebounceEventResult| {
                    if let Ok(events) = result {
                        for event in events {
                            // Only react to create, modify, remove events
                            match event.kind {
                                EventKind::Create(_)
                                | EventKind::Modify(_)
                                | EventKind::Remove(_) => {
                                    if !event.paths.is_empty() {
                                        let path = event.paths[0].clone();
                                        // Send to async context
                                        let _ = tx.blocking_send(path);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                },
            ) {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to create debouncer for {}: {}", name, e);
                    thread_running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // Start watching (0.4+ API)
            if let Err(e) = debouncer.watch(&path, RecursiveMode::Recursive) {
                tracing::error!("Failed to start watcher for {}: {}", name, e);
                thread_running.store(false, Ordering::SeqCst);
                return;
            }

            tracing::info!("Started watching {} at {:?}", name, path);

            // Keep the thread alive while running
            while thread_running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }

            // Stop watching
            if let Err(e) = debouncer.unwatch(&path) {
                tracing::debug!("Error unwatching {}: {}", name, e);
            }

            tracing::info!("Stopped watching {}", name);
        });

        self.thread_handle = Some(thread_handle);

        // Spawn async task to handle callbacks
        let async_running = running.clone();
        let callback_name = self.config.name.clone();
        tokio::spawn(async move {
            while async_running.load(Ordering::SeqCst) {
                match rx.recv().await {
                    Some(changed_path) => {
                        tracing::debug!(
                            "{} changed: {:?}, invalidating cache",
                            callback_name,
                            changed_path
                        );
                        on_change(changed_path);
                    }
                    None => break,
                }
            }
        });
    }

    /// Stop watching for file changes.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(false, Ordering::SeqCst);

        // Wait for the thread to finish
        if let Some(handle) = self.thread_handle.take() {
            // Give it a moment to clean up
            std::thread::sleep(Duration::from_millis(100));
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                tracing::debug!("Watcher thread did not finish in time, continuing shutdown");
            }
        }
    }

    /// Check if the watcher is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatchConfig {
            path: temp_dir.path().to_path_buf(),
            debounce_ms: 100,
            name: "test".to_string(),
        };
        let watcher = FileWatcher::new(config);
        assert!(!watcher.is_running());
    }

    #[tokio::test]
    async fn test_watcher_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatchConfig {
            path: temp_dir.path().to_path_buf(),
            debounce_ms: 100,
            name: "test".to_string(),
        };

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let mut watcher = FileWatcher::new(config);
        watcher.start(move |_path| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert!(watcher.is_running());

        // Give it time to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        watcher.stop();
        assert!(!watcher.is_running());
    }
}
