# Customization Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Customization" tab in Settings that lets users edit the markdown files under `<vault>/config/` (root markdowns + `shards/*.md`) with a file-watcher integration that auto-refreshes external changes.

**Architecture:** Three new HTTP endpoints under `/api/customization/*` (list/get/put), one new `GatewayEvent::CustomizationFileChanged` variant broadcast by the existing `FileWatcher`, and four new React components in `apps/ui/src/features/settings/customization/` wired into `WebSettingsPanel.tsx`. Reuses existing CSS classes; no new global styles.

**Tech Stack:** Rust 2021 (`axum`, existing `gateway-events`, existing `notify_debouncer_full` via `FileWatcher`), React + TypeScript (existing test infra: vitest + Testing Library + MSW pattern). No new dependencies.

**Spec:** `memory-bank/future-state/2026-05-02-customization-tab-design.md`

**Quality bar:** `cargo clippy -p gateway --all-targets -- -D warnings` clean. `cargo fmt` clean for touched files. UI `npm run build` + `npm run lint` clean. All new tests pass. Edit-only — no create/delete UI.

**Branching:** Create a fresh branch off `origin/develop` named `feature/customization-tab` AFTER this plan branch (`docs/customization-tab-spec`) is opened as a PR. The implementation branch stacks on the docs branch so the spec/plan files are visible to subagents during execution; once the docs PR merges, the implementation PR's diff cleans up to just the implementation files.

---

## File structure

### New files

```
gateway/src/http/customization.rs                                      ← endpoints + path validator
apps/ui/src/features/settings/customization/CustomizationTab.tsx       ← top-level container
apps/ui/src/features/settings/customization/FileList.tsx               ← left-pane file list
apps/ui/src/features/settings/customization/FileEditor.tsx             ← right-pane textarea + save/discard
apps/ui/src/features/settings/customization/ConflictBanner.tsx         ← 409-conflict banner
apps/ui/src/features/settings/customization/CustomizationTab.test.tsx  ← end-to-end mocked test
apps/ui/src/features/settings/customization/FileList.test.tsx          ← file list rendering
apps/ui/src/features/settings/customization/FileEditor.test.tsx        ← editor + save flow
apps/ui/src/features/settings/customization/ConflictBanner.test.tsx    ← banner choices
```

### Modified files

```
gateway/src/http/mod.rs                                                ← register 3 new routes
gateway/gateway-events/src/lib.rs                                      ← add CustomizationFileChanged variant
gateway/src/server.rs                                                  ← add config-dir watch in start_file_watchers
apps/ui/src/features/settings/WebSettingsPanel.tsx                     ← add tab + render CustomizationTab
apps/ui/src/services/transport/<websocket-or-event-router>.ts          ← dispatch new event type
gateway/gateway-services/src/paths.rs                                  ← (verify) public config_dir() exists
```

---

## Tasks

Tasks land in dependency order: backend foundation first, then backend routes, then watcher event, then frontend, then verify + PR.

---

### Task 1: Scaffold `customization.rs` with path validator + unit tests

**Files:**
- Create: `gateway/src/http/customization.rs`

- [ ] **Step 1: Write the failing tests**

Create `gateway/src/http/customization.rs` with:

```rust
//! GET / PUT endpoints for editing markdown files under `<vault>/config/`.
//! Used by the Settings → Customization UI tab.

use std::path::PathBuf;

/// Validate a relative path supplied by the UI.
///
/// Allowed shapes (server only ever reads/writes files matching these):
/// - `<file>.md`               → root-level config markdown
/// - `shards/<file>.md`        → markdown shard
///
/// Rejected:
/// - empty
/// - absolute path (`/...` or `\...`)
/// - parent traversal (`..`)
/// - non-`.md` files
/// - any nested path beyond `shards/<file>.md`
pub(crate) fn validate_customization_path(p: &str) -> Result<PathBuf, &'static str> {
    if p.is_empty() || p.starts_with('/') || p.starts_with('\\') {
        return Err("invalid path");
    }
    if p.contains("..") {
        return Err("invalid path");
    }
    if !p.ends_with(".md") {
        return Err("only markdown files allowed");
    }
    let parts: Vec<&str> = p.split('/').collect();
    match parts.as_slice() {
        [_file] => Ok(PathBuf::from(p)),
        ["shards", _file] => Ok(PathBuf::from(p)),
        _ => Err("invalid path"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert_eq!(validate_customization_path(""), Err("invalid path"));
    }

    #[test]
    fn rejects_absolute_unix() {
        assert_eq!(validate_customization_path("/etc/passwd"), Err("invalid path"));
    }

    #[test]
    fn rejects_absolute_windows() {
        assert_eq!(validate_customization_path("\\Windows\\System32"), Err("invalid path"));
    }

    #[test]
    fn rejects_parent_traversal() {
        assert_eq!(validate_customization_path("../../etc/passwd"), Err("invalid path"));
        assert_eq!(validate_customization_path("shards/../../etc/passwd"), Err("invalid path"));
    }

    #[test]
    fn rejects_non_md() {
        assert_eq!(validate_customization_path("settings.json"), Err("only markdown files allowed"));
        assert_eq!(validate_customization_path("shards/foo.txt"), Err("only markdown files allowed"));
    }

    #[test]
    fn rejects_nested_subdirs() {
        assert_eq!(validate_customization_path("wards/foo/bar.md"), Err("invalid path"));
        assert_eq!(validate_customization_path("shards/foo/bar.md"), Err("invalid path"));
    }

    #[test]
    fn accepts_root_md() {
        assert_eq!(validate_customization_path("SOUL.md"), Ok(PathBuf::from("SOUL.md")));
        assert_eq!(validate_customization_path("INSTRUCTIONS.md"), Ok(PathBuf::from("INSTRUCTIONS.md")));
    }

    #[test]
    fn accepts_shard_md() {
        assert_eq!(
            validate_customization_path("shards/memory_learning.md"),
            Ok(PathBuf::from("shards/memory_learning.md"))
        );
    }
}
```

- [ ] **Step 2: Verify the file compiles**

Run: `cargo check -p gateway`
Expected: clean. (Routes aren't registered yet; the module just exists with the validator.)

The `customization` module isn't yet declared in `gateway/src/http/mod.rs` — that comes in Task 4. For now, add `pub mod customization;` next to the other `mod` declarations in `gateway/src/http/mod.rs`:

```rust
mod customization;
```

(Just the declaration. Route registration is in Task 4.)

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p gateway --lib customization::tests`
Expected: 7 passed.

- [ ] **Step 4: Commit**

```bash
git add gateway/src/http/customization.rs gateway/src/http/mod.rs
git commit -m "feat(customization): add path validator and module scaffolding"
```

---

### Task 2: `GET /api/customization/files` handler + struct shapes

**Files:**
- Modify: `gateway/src/http/customization.rs`

- [ ] **Step 1: Write the test**

Append to `gateway/src/http/customization.rs` (inside the existing `#[cfg(test)] mod tests` block):

```rust
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn list_files_finds_root_and_shards() {
        let tmp = tempdir().unwrap();
        let config = tmp.path();
        fs::write(config.join("SOUL.md"), "soul").unwrap();
        fs::write(config.join("INSTRUCTIONS.md"), "instr").unwrap();
        fs::write(config.join("OS.md"), "os").unwrap();
        fs::write(config.join("settings.json"), "{}").unwrap();
        fs::create_dir_all(config.join("shards")).unwrap();
        fs::write(config.join("shards").join("first_turn_protocol.md"), "shard").unwrap();
        fs::write(config.join("shards").join("ignored.txt"), "no").unwrap();

        let entries = enumerate_customization_files(config).expect("enumerate ok");
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"SOUL.md"));
        assert!(paths.contains(&"INSTRUCTIONS.md"));
        assert!(paths.contains(&"OS.md"));
        assert!(paths.contains(&"shards/first_turn_protocol.md"));
        assert!(!paths.contains(&"settings.json"));
        assert!(!paths.contains(&"shards/ignored.txt"));

        let os_entry = entries.iter().find(|e| e.path == "OS.md").unwrap();
        assert!(os_entry.auto_generated);

        let soul_entry = entries.iter().find(|e| e.path == "SOUL.md").unwrap();
        assert!(!soul_entry.auto_generated);
    }
```

You'll need `tempfile` as a dev-dep — verify `gateway/Cargo.toml` already lists it under `[dev-dependencies]` (it does — used elsewhere in the gateway tests). If not, add it.

- [ ] **Step 2: Add the response types and enumeration function**

Append to `gateway/src/http/customization.rs` (above the test module):

```rust
use crate::state::AppState;
use axum::{extract::{Query, State}, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_at: String,
    pub auto_generated: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Root,
    Shard,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const AUTO_GENERATED_NAMES: &[&str] = &["OS.md"];

/// Walk `config_dir/` and `config_dir/shards/` and return entries for every `*.md`.
pub(crate) fn enumerate_customization_files(config_dir: &Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    push_md_files(config_dir, FileKind::Root, "", &mut entries)?;
    let shards = config_dir.join("shards");
    if shards.is_dir() {
        push_md_files(&shards, FileKind::Shard, "shards/", &mut entries)?;
    }
    entries.sort_by(|a, b| a.kind_order().cmp(&b.kind_order()).then_with(|| a.path.cmp(&b.path)));
    Ok(entries)
}

fn push_md_files(
    dir: &Path,
    kind: FileKind,
    prefix: &str,
    out: &mut Vec<FileEntry>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.ends_with(".md") => n.to_string(),
            _ => continue,
        };
        let metadata = entry.metadata()?;
        let modified = metadata.modified()?.into();
        let modified_at: DateTime<Utc> = modified;
        let auto_generated = AUTO_GENERATED_NAMES.contains(&name.as_str());
        out.push(FileEntry {
            path: format!("{}{}", prefix, name),
            kind: match kind {
                FileKind::Root => FileKind::Root,
                FileKind::Shard => FileKind::Shard,
            },
            size: metadata.len(),
            modified_at: modified_at.to_rfc3339(),
            auto_generated,
        });
    }
    Ok(())
}

impl FileEntry {
    fn kind_order(&self) -> u8 {
        match self.kind {
            FileKind::Root => 0,
            FileKind::Shard => 1,
        }
    }
}

/// `GET /api/customization/files` — list editable markdowns.
pub async fn list_files(State(state): State<AppState>) -> (StatusCode, Json<ListResponse>) {
    let config_dir = state.paths.config_dir();
    match enumerate_customization_files(&config_dir) {
        Ok(files) => (StatusCode::OK, Json(ListResponse { success: true, files: Some(files), error: None })),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ListResponse { success: false, files: None, error: Some(e.to_string()) }),
        ),
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p gateway --lib customization::tests`
Expected: 8 passed (7 from Task 1 + 1 new `list_files_finds_root_and_shards`).

- [ ] **Step 4: Verify clippy + fmt**

```
cargo clippy -p gateway --lib -- -D warnings
cargo fmt -p gateway --check
```
Both clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/customization.rs
git commit -m "feat(customization): GET /api/customization/files handler + enumerator"
```

---

### Task 3: `GET` and `PUT /api/customization/file` handlers

**Files:**
- Modify: `gateway/src/http/customization.rs`

- [ ] **Step 1: Write the tests**

Append inside the test module:

```rust
    use std::time::{Duration, SystemTime};

    fn touch(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
    }

    #[test]
    fn read_file_content_resolves_relative_to_config_dir() {
        let tmp = tempdir().unwrap();
        let config = tmp.path();
        touch(&config.join("SOUL.md"), "soul body");

        let resolved = resolve_path(config, "SOUL.md").expect("ok");
        assert_eq!(resolved, config.join("SOUL.md"));

        let body = fs::read_to_string(&resolved).unwrap();
        assert_eq!(body, "soul body");
    }

    #[test]
    fn read_file_rejects_invalid_path() {
        let tmp = tempdir().unwrap();
        assert!(resolve_path(tmp.path(), "../escape.md").is_err());
        assert!(resolve_path(tmp.path(), "/abs.md").is_err());
        assert!(resolve_path(tmp.path(), "wards/x.md").is_err());
    }

    #[test]
    fn save_file_succeeds_when_version_matches() {
        let tmp = tempdir().unwrap();
        let config = tmp.path();
        let file = config.join("SOUL.md");
        touch(&file, "v1");

        let initial_version = file_version(&file).unwrap();
        // Sleep so mtime differs measurably between writes (not strictly required on most filesystems but defensive).
        std::thread::sleep(Duration::from_millis(20));

        let result = save_file_with_check(config, "SOUL.md", "v2", &initial_version);
        assert!(matches!(result, SaveOutcome::Ok(_)));
        assert_eq!(fs::read_to_string(&file).unwrap(), "v2");
    }

    #[test]
    fn save_file_returns_conflict_when_disk_changed() {
        let tmp = tempdir().unwrap();
        let config = tmp.path();
        let file = config.join("SOUL.md");
        touch(&file, "v1");

        let stale_version = file_version(&file).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        // Someone else updates the file:
        touch(&file, "v_external");

        let result = save_file_with_check(config, "SOUL.md", "v_ours", &stale_version);
        match result {
            SaveOutcome::Conflict { current_content, current_version } => {
                assert_eq!(current_content, "v_external");
                assert_ne!(current_version, stale_version);
            }
            other => panic!("expected Conflict, got {:?}", other),
        }
    }
```

- [ ] **Step 2: Implement the handlers**

Append to `gateway/src/http/customization.rs` (above the test module):

```rust
const MAX_CONTENT_BYTES: usize = 1_000_000; // 1 MB cap

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct FileResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_generated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Populated only on 409 conflict
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SaveRequest {
    pub path: String,
    pub content: String,
    pub expected_version: String,
}

#[derive(Debug)]
pub(crate) enum SaveOutcome {
    Ok(String /* new version */),
    Conflict { current_content: String, current_version: String },
    NotFound,
    Io(String),
}

pub(crate) fn resolve_path(config_dir: &Path, p: &str) -> Result<std::path::PathBuf, &'static str> {
    let validated = validate_customization_path(p)?;
    Ok(config_dir.join(validated))
}

pub(crate) fn file_version(path: &Path) -> std::io::Result<String> {
    let mtime: DateTime<Utc> = std::fs::metadata(path)?.modified()?.into();
    Ok(mtime.to_rfc3339())
}

pub(crate) fn save_file_with_check(
    config_dir: &Path,
    rel_path: &str,
    new_content: &str,
    expected_version: &str,
) -> SaveOutcome {
    let resolved = match resolve_path(config_dir, rel_path) {
        Ok(p) => p,
        Err(e) => return SaveOutcome::Io(format!("invalid path: {}", e)),
    };
    if !resolved.exists() {
        return SaveOutcome::NotFound;
    }
    let current_version = match file_version(&resolved) {
        Ok(v) => v,
        Err(e) => return SaveOutcome::Io(e.to_string()),
    };
    if current_version != expected_version {
        let current_content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(e) => return SaveOutcome::Io(e.to_string()),
        };
        return SaveOutcome::Conflict { current_content, current_version };
    }
    if let Err(e) = std::fs::write(&resolved, new_content) {
        return SaveOutcome::Io(e.to_string());
    }
    match file_version(&resolved) {
        Ok(v) => SaveOutcome::Ok(v),
        Err(e) => SaveOutcome::Io(e.to_string()),
    }
}

/// `GET /api/customization/file?path=<relative>`
pub async fn get_file(
    State(state): State<AppState>,
    Query(q): Query<PathQuery>,
) -> (StatusCode, Json<FileResponse>) {
    let config_dir = state.paths.config_dir();
    let resolved = match resolve_path(&config_dir, &q.path) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FileResponse {
                    success: false,
                    error: Some(e.to_string()),
                    ..Default::default()
                }),
            );
        }
    };
    if !resolved.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(FileResponse {
                success: false,
                error: Some("file not found".to_string()),
                ..Default::default()
            }),
        );
    }
    let content = match std::fs::read_to_string(&resolved) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FileResponse {
                    success: false,
                    error: Some(e.to_string()),
                    ..Default::default()
                }),
            );
        }
    };
    let version = file_version(&resolved).unwrap_or_default();
    let name = std::path::Path::new(&q.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let auto_generated = AUTO_GENERATED_NAMES.contains(&name);
    (
        StatusCode::OK,
        Json(FileResponse {
            success: true,
            path: Some(q.path.clone()),
            content: Some(content),
            version: Some(version),
            auto_generated: Some(auto_generated),
            ..Default::default()
        }),
    )
}

/// `PUT /api/customization/file`
pub async fn put_file(
    State(state): State<AppState>,
    Json(req): Json<SaveRequest>,
) -> (StatusCode, Json<FileResponse>) {
    if req.content.len() > MAX_CONTENT_BYTES {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(FileResponse {
                success: false,
                error: Some(format!(
                    "content too large ({} bytes > {} limit)",
                    req.content.len(),
                    MAX_CONTENT_BYTES
                )),
                ..Default::default()
            }),
        );
    }
    let config_dir = state.paths.config_dir();
    match save_file_with_check(&config_dir, &req.path, &req.content, &req.expected_version) {
        SaveOutcome::Ok(version) => (
            StatusCode::OK,
            Json(FileResponse {
                success: true,
                path: Some(req.path.clone()),
                version: Some(version),
                ..Default::default()
            }),
        ),
        SaveOutcome::Conflict { current_content, current_version } => (
            StatusCode::CONFLICT,
            Json(FileResponse {
                success: false,
                error: Some("version mismatch".to_string()),
                current_content: Some(current_content),
                current_version: Some(current_version),
                ..Default::default()
            }),
        ),
        SaveOutcome::NotFound => (
            StatusCode::NOT_FOUND,
            Json(FileResponse {
                success: false,
                error: Some("file not found".to_string()),
                ..Default::default()
            }),
        ),
        SaveOutcome::Io(e) => (
            StatusCode::BAD_REQUEST,
            Json(FileResponse {
                success: false,
                error: Some(e),
                ..Default::default()
            }),
        ),
    }
}

impl Default for FileResponse {
    fn default() -> Self {
        Self {
            success: false,
            path: None,
            content: None,
            version: None,
            auto_generated: None,
            error: None,
            current_content: None,
            current_version: None,
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p gateway --lib customization::tests`
Expected: 12 passed (8 prior + 4 new).

- [ ] **Step 4: clippy + fmt**

```
cargo clippy -p gateway --lib -- -D warnings
cargo fmt -p gateway --check
```
Both clean.

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/customization.rs
git commit -m "feat(customization): GET/PUT file handlers with optimistic concurrency"
```

---

### Task 4: Register routes in `gateway/src/http/mod.rs`

**Files:**
- Modify: `gateway/src/http/mod.rs`

- [ ] **Step 1: Find the existing route block**

Read `gateway/src/http/mod.rs` and locate the section near the existing `/api/settings/...` routes (around line 178+ in the existing file).

- [ ] **Step 2: Add the three new routes**

Insert these three lines next to the existing settings routes:

```rust
        .route("/api/customization/files", get(customization::list_files))
        .route("/api/customization/file", get(customization::get_file))
        .route("/api/customization/file", put(customization::put_file))
```

- [ ] **Step 3: Build and run an end-to-end smoke**

Run: `cargo build -p gateway`
Expected: clean.

If a daemon is running: `curl -s http://localhost:18791/api/customization/files | jq '.success, (.files | length)'`
Expected: `true` and a number ≥ 6.

(Skip the curl if no daemon is running.)

- [ ] **Step 4: Commit**

```bash
git add gateway/src/http/mod.rs
git commit -m "feat(customization): register list/get/put routes under /api/customization"
```

---

### Task 5: Add `CustomizationFileChanged` event variant

**Files:**
- Modify: `gateway/gateway-events/src/lib.rs`

- [ ] **Step 1: Add the new variant to `GatewayEvent`**

Insert this variant inside the `pub enum GatewayEvent { … }` block, near the bottom (just before `Heartbeat` or `Error` — anywhere within the enum body works):

```rust
    /// A markdown file in <vault>/config/ was created, modified, or deleted.
    /// Only emitted for files matching the customization allow-list (root
    /// `*.md` and `shards/*.md`). Used by the Settings → Customization tab
    /// to refresh its file list and detect external edits while a file is
    /// being edited in the UI.
    CustomizationFileChanged {
        /// Relative to <vault>/config/ (e.g., "SOUL.md" or "shards/foo.md")
        path: String,
        /// New mtime as RFC3339, or empty string if the file was deleted.
        modified_at: String,
    },
```

- [ ] **Step 2: Build to confirm the enum still compiles for all consumers**

Run: `cargo check --workspace`
Expected: clean.

If any exhaustive `match GatewayEvent { … }` somewhere in the codebase doesn't have a wildcard arm, you'll get a `non_exhaustive_pattern` error. Add `GatewayEvent::CustomizationFileChanged { .. } => { /* no-op */ }` to such matches. Most consumers use `if let` patterns or have wildcard arms.

- [ ] **Step 3: Commit**

```bash
git add gateway/gateway-events/src/lib.rs
git commit -m "feat(customization): add GatewayEvent::CustomizationFileChanged variant"
```

If you had to add wildcard arms to other consumers in Step 2, include those files in the commit and amend the message: `feat(customization): add CustomizationFileChanged event variant + match arm updates`.

---

### Task 6: Wire `FileWatcher` to broadcast the new event

**Files:**
- Modify: `gateway/src/server.rs`

- [ ] **Step 1: Find `start_file_watchers`**

Read `gateway/src/server.rs` around line 390. The existing `start_file_watchers` adds two watches (skills, agents). Add a third for the config dir.

- [ ] **Step 2: Insert the config-dir watch**

After the existing `agents` watch (line ~409), before `watcher.start()`:

```rust
        let event_bus = self.state.event_bus.clone();
        let config_dir = self.state.paths.config_dir();
        let config_dir_for_filter = config_dir.clone();
        watcher.add_watch(config_dir.clone(), "customization", move |path| {
            // Compute relative path; reject anything outside the customization allow-list.
            let rel = match path.strip_prefix(&config_dir_for_filter) {
                Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
                Err(_) => return,
            };
            if !rel.ends_with(".md") {
                return;
            }
            let parts: Vec<&str> = rel.split('/').collect();
            let valid = matches!(parts.as_slice(), [_] | ["shards", _]);
            if !valid {
                return;
            }
            let modified_at = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_default();
            let event = gateway_events::GatewayEvent::CustomizationFileChanged {
                path: rel,
                modified_at,
            };
            let event_bus = event_bus.clone();
            tokio::spawn(async move {
                event_bus.broadcast(event);
            });
        });
```

(If `event_bus.broadcast` is a sync call, drop the `tokio::spawn` wrapper. The existing watchers don't broadcast — they invalidate caches. This is the first one to broadcast events, so verify the EventBus signature; if it's `fn broadcast(&self, event: GatewayEvent)` synchronously, just call it directly.)

- [ ] **Step 3: Verify the build**

Run: `cargo build -p gateway`
Expected: clean.

If the closure capture has Send/Sync issues (likely when `event_bus` is wrapped in `Arc`), adjust to clone the `Arc` before the closure rather than inside. The plan-shape captures `event_bus` once; if the watcher's `add_watch` requires `'static + Fn`, you may need to wrap fields in `Arc`s manually.

- [ ] **Step 4: Smoke-test the watcher**

Start the daemon. From another terminal: `touch ~/Documents/zbot/config/SOUL.md`. From the daemon's logs, you should see no error. From a UI WebSocket subscriber (or `wscat`-equivalent), you should see a `customization_file_changed` event within ~5s (debounced).

If you can't easily do this end-to-end, defer to manual smoke at the PR stage.

- [ ] **Step 5: Commit**

```bash
git add gateway/src/server.rs
git commit -m "feat(customization): emit CustomizationFileChanged events from FileWatcher"
```

---

### Task 7: Frontend — scaffold `CustomizationTab` and wire into `WebSettingsPanel`

**Files:**
- Create: `apps/ui/src/features/settings/customization/CustomizationTab.tsx`
- Modify: `apps/ui/src/features/settings/WebSettingsPanel.tsx`

- [ ] **Step 1: Create the empty tab container**

Create `apps/ui/src/features/settings/customization/CustomizationTab.tsx`:

```tsx
import { useEffect, useState } from "react";

type FileEntry = {
  path: string;
  kind: "root" | "shard";
  size: number;
  modifiedAt: string;
  autoGenerated: boolean;
};

type ApiList = { success: boolean; files?: FileEntry[]; error?: string };

export function CustomizationTab() {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch("/api/customization/files");
        const body = (await res.json()) as ApiList;
        if (!cancelled) {
          if (body.success && body.files) {
            setFiles(body.files);
          } else {
            setError(body.error ?? "Failed to load files");
          }
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) return <div className="settings-card">Loading customization files…</div>;
  if (error) return <div className="settings-card error">{error}</div>;

  return (
    <section className="settings-card">
      <header>
        <h3>Customization</h3>
      </header>
      <p className="muted small">
        Edit the markdown files that shape your agent's behavior. Changes save back to{" "}
        <code>~/Documents/zbot/config/</code>.
      </p>
      <p className="muted small">{files.length} file(s) found.</p>
    </section>
  );
}
```

(Bare scaffold — file list and editor follow in later tasks.)

- [ ] **Step 2: Wire the tab into `WebSettingsPanel.tsx`**

In `apps/ui/src/features/settings/WebSettingsPanel.tsx`:

1. Add the import near the other tab-component imports:
   ```tsx
   import { CustomizationTab } from "./customization/CustomizationTab";
   ```

2. Find the `tabs` array (around line 285) and add an entry:
   ```tsx
   { id: "customization", label: "Customization" },
   ```
   Place it after `{ id: "advanced", label: "Advanced" }`.

3. Find the `<TabPanel>` block for the existing tabs and add a new one for customization, modeled on the `advanced` panel. The exact JSX shape will depend on the existing structure — read it and follow the pattern. At minimum:
   ```tsx
   <TabPanel id="customization" activeTab={activeTab}>
     <CustomizationTab />
   </TabPanel>
   ```

- [ ] **Step 3: Build the UI**

```bash
cd apps/ui && npm run build
```
Expected: clean. The new tab renders (empty list of files, but loads).

- [ ] **Step 4: Manual smoke**

Start the daemon + dev server: visit `/settings`, click "Customization" — should show "N file(s) found" with N matching the number of markdowns in your config dir.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/settings/customization/CustomizationTab.tsx apps/ui/src/features/settings/WebSettingsPanel.tsx
git commit -m "feat(ui): scaffold Customization tab in Settings"
```

---

### Task 8: Frontend — `FileList` component + tests

**Files:**
- Create: `apps/ui/src/features/settings/customization/FileList.tsx`
- Create: `apps/ui/src/features/settings/customization/FileList.test.tsx`
- Modify: `apps/ui/src/features/settings/customization/CustomizationTab.tsx`

- [ ] **Step 1: Write the test**

Create `apps/ui/src/features/settings/customization/FileList.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { FileList } from "./FileList";

const sampleFiles = [
  { path: "SOUL.md", kind: "root" as const, size: 42, modifiedAt: "2026-05-02T12:00:00Z", autoGenerated: false },
  { path: "OS.md", kind: "root" as const, size: 1152, modifiedAt: "2026-05-02T12:00:00Z", autoGenerated: true },
  { path: "shards/memory_learning.md", kind: "shard" as const, size: 800, modifiedAt: "2026-05-02T12:00:00Z", autoGenerated: false },
];

describe("FileList", () => {
  it("renders root and shards sections", () => {
    render(<FileList files={sampleFiles} selectedPath={null} onSelect={() => {}} />);
    expect(screen.getByText("SOUL.md")).toBeInTheDocument();
    expect(screen.getByText("OS.md")).toBeInTheDocument();
    expect(screen.getByText("memory_learning.md")).toBeInTheDocument();
  });

  it("marks OS.md as auto-generated", () => {
    render(<FileList files={sampleFiles} selectedPath={null} onSelect={() => {}} />);
    expect(screen.getByText(/auto-regenerated on restart/i)).toBeInTheDocument();
  });

  it("invokes onSelect with the clicked path", () => {
    const onSelect = vi.fn();
    render(<FileList files={sampleFiles} selectedPath={null} onSelect={onSelect} />);
    fireEvent.click(screen.getByText("SOUL.md"));
    expect(onSelect).toHaveBeenCalledWith("SOUL.md");
  });

  it("highlights the selected file", () => {
    render(<FileList files={sampleFiles} selectedPath="SOUL.md" onSelect={() => {}} />);
    const row = screen.getByText("SOUL.md").closest("button");
    expect(row?.getAttribute("aria-selected")).toBe("true");
  });
});
```

- [ ] **Step 2: Implement the component**

Create `apps/ui/src/features/settings/customization/FileList.tsx`:

```tsx
type FileEntry = {
  path: string;
  kind: "root" | "shard";
  size: number;
  modifiedAt: string;
  autoGenerated: boolean;
};

type Props = {
  files: FileEntry[];
  selectedPath: string | null;
  onSelect: (path: string) => void;
};

export function FileList({ files, selectedPath, onSelect }: Props) {
  const root = files.filter((f) => f.kind === "root");
  const shards = files.filter((f) => f.kind === "shard");

  const renderRow = (f: FileEntry) => {
    const displayName = f.kind === "shard" ? f.path.replace(/^shards\//, "") : f.path;
    const isSelected = selectedPath === f.path;
    return (
      <button
        key={f.path}
        type="button"
        onClick={() => onSelect(f.path)}
        aria-selected={isSelected}
        className={`btn btn--outline btn--sm${isSelected ? " btn--primary" : ""}`}
        style={{ display: "block", width: "100%", textAlign: "left", margin: "2px 0" }}
      >
        <div>
          <code>{displayName}</code>
          {f.autoGenerated && (
            <span className="muted small" style={{ marginLeft: "var(--spacing-2)" }}>
              (auto-regenerated on restart)
            </span>
          )}
        </div>
      </button>
    );
  };

  return (
    <div className="customization-file-list">
      {root.length > 0 && (
        <>
          <h4 className="muted small" style={{ margin: "var(--spacing-2) 0" }}>
            Root
          </h4>
          {root.map(renderRow)}
        </>
      )}
      {shards.length > 0 && (
        <>
          <h4 className="muted small" style={{ margin: "var(--spacing-3) 0 var(--spacing-2)" }}>
            shards/
          </h4>
          {shards.map(renderRow)}
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Wire into `CustomizationTab.tsx`**

Replace the body of `CustomizationTab.tsx` (after the imports + state) with:

```tsx
import { FileList } from "./FileList";

// … existing FileEntry type, useEffect, loading/error states …

  const [selectedPath, setSelectedPath] = useState<string | null>(null);

  // (after the loading/error early returns)

  return (
    <section className="settings-card">
      <header>
        <h3>Customization</h3>
      </header>
      <p className="muted small">
        Edit the markdown files that shape your agent's behavior.
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "260px 1fr", gap: "var(--spacing-4)" }}>
        <FileList files={files} selectedPath={selectedPath} onSelect={setSelectedPath} />
        <div className="muted">{selectedPath ? `Selected: ${selectedPath}` : "Select a file to edit."}</div>
      </div>
    </section>
  );
```

- [ ] **Step 4: Run tests**

```bash
cd apps/ui && npm test -- --run FileList
```
Expected: 4 passed.

- [ ] **Step 5: Lint + build**

```bash
cd apps/ui && npm run lint
cd apps/ui && npm run build
```
Both clean.

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/settings/customization/
git commit -m "feat(ui): FileList component for Customization tab"
```

---

### Task 9: Frontend — `FileEditor` component + save flow + tests

**Files:**
- Create: `apps/ui/src/features/settings/customization/FileEditor.tsx`
- Create: `apps/ui/src/features/settings/customization/FileEditor.test.tsx`
- Modify: `apps/ui/src/features/settings/customization/CustomizationTab.tsx`

- [ ] **Step 1: Write the test**

Create `apps/ui/src/features/settings/customization/FileEditor.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { FileEditor } from "./FileEditor";

beforeEach(() => {
  globalThis.fetch = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
    const u = url.toString();
    if (u.includes("/api/customization/file?path=")) {
      return new Response(
        JSON.stringify({
          success: true,
          path: "SOUL.md",
          content: "soul body",
          version: "v1",
          autoGenerated: false,
        }),
        { status: 200 },
      );
    }
    if (u.endsWith("/api/customization/file") && init?.method === "PUT") {
      const body = JSON.parse(init.body as string);
      if (body.expectedVersion === "v1") {
        return new Response(
          JSON.stringify({ success: true, path: body.path, version: "v2" }),
          { status: 200 },
        );
      }
      return new Response(
        JSON.stringify({
          success: false,
          error: "version mismatch",
          currentContent: "external content",
          currentVersion: "v_external",
        }),
        { status: 409 },
      );
    }
    return new Response("not mocked", { status: 404 });
  }) as unknown as typeof fetch;
});

describe("FileEditor", () => {
  it("loads file content on mount", async () => {
    render(<FileEditor path="SOUL.md" />);
    await waitFor(() => {
      expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("soul body");
    });
  });

  it("shows dirty badge when content changes", async () => {
    render(<FileEditor path="SOUL.md" />);
    await waitFor(() => screen.getByRole("textbox"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "edited" } });
    expect(screen.getByText(/Modified/i)).toBeInTheDocument();
  });

  it("saves changes via PUT and clears dirty badge", async () => {
    render(<FileEditor path="SOUL.md" />);
    await waitFor(() => screen.getByRole("textbox"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "edited" } });
    fireEvent.click(screen.getByRole("button", { name: /Save/i }));
    await waitFor(() => {
      expect(screen.queryByText(/Modified/i)).not.toBeInTheDocument();
    });
  });

  it("Discard reverts to disk content", async () => {
    render(<FileEditor path="SOUL.md" />);
    await waitFor(() => screen.getByRole("textbox"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "edited" } });
    fireEvent.click(screen.getByRole("button", { name: /Discard/i }));
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("soul body");
  });

  it("shows conflict banner when save returns 409", async () => {
    render(<FileEditor path="SOUL.md" />);
    await waitFor(() => screen.getByRole("textbox"));
    // Force a stale version by editing, then mock fetch to return 409 on PUT
    globalThis.fetch = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
      const u = url.toString();
      if (u.endsWith("/api/customization/file") && init?.method === "PUT") {
        return new Response(
          JSON.stringify({
            success: false,
            error: "version mismatch",
            currentContent: "external content",
            currentVersion: "v_external",
          }),
          { status: 409 },
        );
      }
      return new Response("nope", { status: 404 });
    }) as unknown as typeof fetch;
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "edited" } });
    fireEvent.click(screen.getByRole("button", { name: /Save/i }));
    await waitFor(() => {
      expect(screen.getByText(/changed on disk/i)).toBeInTheDocument();
    });
  });
});
```

- [ ] **Step 2: Implement the component**

Create `apps/ui/src/features/settings/customization/FileEditor.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ConflictBanner } from "./ConflictBanner";

type Props = {
  path: string;
};

type Conflict = { currentContent: string; currentVersion: string };

export function FileEditor({ path }: Props) {
  const [diskContent, setDiskContent] = useState("");
  const [editorContent, setEditorContent] = useState("");
  const [version, setVersion] = useState<string>("");
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch(`/api/customization/file?path=${encodeURIComponent(path)}`);
        const body = await res.json();
        if (!cancelled && body.success) {
          setDiskContent(body.content ?? "");
          setEditorContent(body.content ?? "");
          setVersion(body.version ?? "");
          setError(null);
          setConflict(null);
        } else if (!cancelled) {
          setError(body.error ?? "Failed to load file");
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, [path]);

  const isDirty = editorContent !== diskContent;

  const onSave = async () => {
    setIsSaving(true);
    try {
      const res = await fetch("/api/customization/file", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          path,
          content: editorContent,
          expectedVersion: version,
        }),
      });
      const body = await res.json();
      if (res.status === 409) {
        setConflict({
          currentContent: body.currentContent ?? "",
          currentVersion: body.currentVersion ?? "",
        });
      } else if (body.success) {
        setDiskContent(editorContent);
        setVersion(body.version ?? version);
      } else {
        setError(body.error ?? "Save failed");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSaving(false);
    }
  };

  const onDiscard = () => {
    setEditorContent(diskContent);
  };

  const onAcceptDisk = () => {
    if (!conflict) return;
    setDiskContent(conflict.currentContent);
    setEditorContent(conflict.currentContent);
    setVersion(conflict.currentVersion);
    setConflict(null);
  };

  const onKeepEditing = () => {
    if (!conflict) return;
    setVersion(conflict.currentVersion);
    setConflict(null);
  };

  return (
    <div className="customization-editor">
      <header className="row" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)" }}>
        <h4 style={{ margin: 0 }}>
          <code>{path}</code>
        </h4>
        {isDirty && <span className="muted small">• Modified</span>}
      </header>
      {conflict && (
        <ConflictBanner onAcceptDisk={onAcceptDisk} onKeepEditing={onKeepEditing} />
      )}
      {error && (
        <div className="warning" role="status">
          {error}
        </div>
      )}
      <textarea
        value={editorContent}
        onChange={(e) => setEditorContent(e.target.value)}
        style={{
          width: "100%",
          minHeight: 400,
          fontFamily: "monospace",
          fontSize: "var(--font-size-sm)",
          padding: "var(--spacing-3)",
        }}
        spellCheck={false}
      />
      <div className="row" style={{ display: "flex", gap: "var(--spacing-2)", marginTop: "var(--spacing-2)" }}>
        <button
          type="button"
          className="btn btn--outline btn--sm"
          onClick={onDiscard}
          disabled={!isDirty || isSaving}
        >
          Discard
        </button>
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={onSave}
          disabled={!isDirty || isSaving}
        >
          {isSaving ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Wire into `CustomizationTab.tsx`**

Replace the placeholder right pane in `CustomizationTab.tsx`:

```tsx
import { FileEditor } from "./FileEditor";

// inside the JSX, replace the right-pane <div className="muted">…</div> with:
{selectedPath ? (
  <FileEditor key={selectedPath} path={selectedPath} />
) : (
  <div className="muted">Select a file to edit.</div>
)}
```

The `key={selectedPath}` forces React to remount the editor when the user picks a different file, which resets all internal state (avoids stale dirty content from a previously-edited file leaking).

- [ ] **Step 4: Tests**

```bash
cd apps/ui && npm test -- --run FileEditor
```
Expected: 5 passed (this task adds 5 tests).

You'll also need a stub `ConflictBanner` so the component compiles. Either inline it temporarily or do Task 10 first, then come back. Easiest: do Task 10 first.

**If `ConflictBanner` does not yet exist**, create a minimal stub:

```tsx
// apps/ui/src/features/settings/customization/ConflictBanner.tsx
type Props = { onAcceptDisk: () => void; onKeepEditing: () => void };
export function ConflictBanner({ onAcceptDisk, onKeepEditing }: Props) {
  return (
    <div role="status">
      changed on disk —
      <button type="button" onClick={onAcceptDisk}>View disk version</button>
      <button type="button" onClick={onKeepEditing}>Keep editing</button>
    </div>
  );
}
```

(Task 10 fleshes it out.)

- [ ] **Step 5: Lint + build**

```bash
cd apps/ui && npm run lint
cd apps/ui && npm run build
```

- [ ] **Step 6: Commit**

```bash
git add apps/ui/src/features/settings/customization/
git commit -m "feat(ui): FileEditor with optimistic-concurrency save flow"
```

---

### Task 10: Frontend — `ConflictBanner` component + tests

**Files:**
- Modify: `apps/ui/src/features/settings/customization/ConflictBanner.tsx`
- Create: `apps/ui/src/features/settings/customization/ConflictBanner.test.tsx`

- [ ] **Step 1: Write the test**

Create `apps/ui/src/features/settings/customization/ConflictBanner.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ConflictBanner } from "./ConflictBanner";

describe("ConflictBanner", () => {
  it("renders the conflict message", () => {
    render(<ConflictBanner onAcceptDisk={() => {}} onKeepEditing={() => {}} />);
    expect(screen.getByText(/changed on disk while you were editing/i)).toBeInTheDocument();
  });

  it("invokes onAcceptDisk when 'View disk version' clicked", () => {
    const onAcceptDisk = vi.fn();
    render(<ConflictBanner onAcceptDisk={onAcceptDisk} onKeepEditing={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /View disk version/i }));
    expect(onAcceptDisk).toHaveBeenCalled();
  });

  it("invokes onKeepEditing when 'Keep editing' clicked", () => {
    const onKeepEditing = vi.fn();
    render(<ConflictBanner onAcceptDisk={() => {}} onKeepEditing={onKeepEditing} />);
    fireEvent.click(screen.getByRole("button", { name: /Keep editing/i }));
    expect(onKeepEditing).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Replace the stub with the full component**

Overwrite `apps/ui/src/features/settings/customization/ConflictBanner.tsx`:

```tsx
type Props = {
  onAcceptDisk: () => void;
  onKeepEditing: () => void;
};

export function ConflictBanner({ onAcceptDisk, onKeepEditing }: Props) {
  return (
    <div className="warning" role="status" style={{ display: "flex", alignItems: "center", gap: "var(--spacing-2)" }}>
      <span>This file changed on disk while you were editing.</span>
      <button type="button" className="btn btn--outline btn--sm" onClick={onAcceptDisk}>
        View disk version
      </button>
      <button type="button" className="btn btn--outline btn--sm" onClick={onKeepEditing}>
        Keep editing
      </button>
    </div>
  );
}
```

- [ ] **Step 3: Tests**

```bash
cd apps/ui && npm test -- --run ConflictBanner
```
Expected: 3 passed.

- [ ] **Step 4: Lint + build**

Both clean.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/settings/customization/
git commit -m "feat(ui): ConflictBanner with View-disk / Keep-editing actions"
```

---

### Task 11: Frontend — WebSocket handler for `customization_file_changed`

**Files:**
- Modify: `apps/ui/src/services/transport/<websocket-or-event-router>.ts`
- Modify: `apps/ui/src/features/settings/customization/CustomizationTab.tsx`

- [ ] **Step 1: Find the existing WebSocket event router**

The frontend already handles `GatewayEvent` types over WebSocket. Locate the file that receives WebSocket messages and dispatches by `event.type`. Likely in `apps/ui/src/services/transport/`. Read it.

The event payload from the gateway is:

```ts
{ type: "customization_file_changed"; path: string; modified_at: string; }
```

(Note: backend uses `snake_case` per the existing `#[serde(rename_all = "snake_case")]` on `GatewayEvent`. So field names from the wire are `modified_at`, not `modifiedAt`. Adjust as needed.)

- [ ] **Step 2: Add a handler that calls back to subscribed components**

The exact mechanism depends on how the existing event router works. Two common shapes:

**Shape A (event emitter / pubsub):** the router has something like `transport.on("customization_file_changed", callback)`. Add to the union type and call back on dispatch.

**Shape B (React context with reducer):** the router dispatches into a global context. Add a handler for the new type.

Look at how an existing event (e.g., `agent_started` or `tool_call`) is wired and follow the same pattern.

- [ ] **Step 3: Subscribe in `CustomizationTab.tsx`**

Add to `CustomizationTab.tsx`:

```tsx
useEffect(() => {
  const unsubscribe = transport.subscribe("customization_file_changed", (event: { path: string; modifiedAt: string }) => {
    // 1. Refresh files list (refetch /api/customization/files)
    // 2. If event.path === selectedPath AND editor is not dirty → reload editor (the editor already reloads on `path` change; bumping a refresh key handles this)
    // 3. If event.path === selectedPath AND editor IS dirty → editor's banner handles it (next save → 409 path)
    void refreshFiles();
  });
  return unsubscribe;
}, []);
```

(The exact `transport.subscribe` shape depends on the router pattern. Adapt.)

- [ ] **Step 4: Build and run end-to-end**

```bash
cd apps/ui && npm run build
```

Manual test: with the daemon running, open the Customization tab. From a separate terminal: `touch ~/Documents/zbot/config/SOUL.md`. The file list's `modifiedAt` should refresh within ~5s (notify debounce + WebSocket roundtrip).

If the manual test isn't easy, defer to the implementation PR's smoke checklist.

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/
git commit -m "feat(ui): subscribe to customization_file_changed events"
```

---

### Task 12: Cross-cut verify

**Files:**
- None modified — verification pass.

- [ ] **Step 1: Backend**

```bash
cargo test -p gateway --lib customization
cargo clippy -p gateway --all-targets -- -D warnings
cargo fmt -p gateway --check
cargo check --workspace
```

Expected: all clean. (Pre-existing fmt drift in unrelated files is fine; just make sure the customization changes are clean.)

- [ ] **Step 2: Frontend**

```bash
cd apps/ui
npm test -- --run customization
npm run lint
npm run build
```

Expected: all clean. The `customization` test filter should pick up FileList, FileEditor, ConflictBanner, CustomizationTab tests.

- [ ] **Step 3: Confirm scope of changes**

```bash
git diff --name-only origin/develop...HEAD
```

Expected files (only — flag anything else as a scope creep):
- `gateway/src/http/customization.rs` (new)
- `gateway/src/http/mod.rs` (modified)
- `gateway/gateway-events/src/lib.rs` (modified)
- `gateway/src/server.rs` (modified)
- `apps/ui/src/features/settings/customization/CustomizationTab.tsx` (new)
- `apps/ui/src/features/settings/customization/FileList.tsx` (new)
- `apps/ui/src/features/settings/customization/FileEditor.tsx` (new)
- `apps/ui/src/features/settings/customization/ConflictBanner.tsx` (new)
- `apps/ui/src/features/settings/customization/*.test.tsx` (new)
- `apps/ui/src/features/settings/WebSettingsPanel.tsx` (modified)
- `apps/ui/src/services/transport/<…>.ts` (modified)
- `memory-bank/future-state/2026-05-02-customization-tab-design.md` (will drop out once docs PR merges)
- `memory-bank/plans/2026-05-02-customization-tab-implementation.md` (same)

---

### Task 13: Push + open implementation PR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin feature/customization-tab
```

- [ ] **Step 2: Open the PR**

```bash
gh pr create --base develop --head feature/customization-tab \
  --title "feat: Customization tab — in-app editor for config markdowns" \
  --body "$(cat <<'EOF'
## Summary

Implements [\`memory-bank/future-state/2026-05-02-customization-tab-design.md\`](../blob/develop/memory-bank/future-state/2026-05-02-customization-tab-design.md).

New \"Customization\" tab in Settings. Lists every markdown file under \`<vault>/config/\` (root markdowns + \`shards/*.md\`) and provides an in-app editor with optimistic-concurrency save and live external-edit detection via the existing \`FileWatcher\`.

## Files

**Backend (~3 files):**
- \`gateway/src/http/customization.rs\` (new) — path validator, list/get/put handlers
- \`gateway/src/http/mod.rs\` — register 3 new routes
- \`gateway/gateway-events/src/lib.rs\` — \`CustomizationFileChanged\` variant
- \`gateway/src/server.rs\` — extra \`add_watch\` on the config dir that broadcasts the new event

**Frontend (~5 files):**
- \`apps/ui/src/features/settings/customization/CustomizationTab.tsx\` (new)
- \`apps/ui/src/features/settings/customization/FileList.tsx\` (new)
- \`apps/ui/src/features/settings/customization/FileEditor.tsx\` (new)
- \`apps/ui/src/features/settings/customization/ConflictBanner.tsx\` (new)
- \`apps/ui/src/features/settings/customization/*.test.tsx\` (new tests)
- \`apps/ui/src/features/settings/WebSettingsPanel.tsx\` — add tab entry
- \`apps/ui/src/services/transport/<router>.ts\` — handle new event

No new dependencies. No new global CSS — reuses existing classes (\`settings-card\`, \`btn--primary\`, \`btn--outline\`, \`muted\`, etc.).

## Test plan

- [x] \`cargo test -p gateway --lib customization\` — 12 unit tests pass
- [x] \`cargo clippy -p gateway --all-targets -- -D warnings\` — clean
- [x] \`cd apps/ui && npm test -- --run customization\` — all component tests pass
- [x] \`npm run lint\` + \`npm run build\` — clean
- [ ] Manual: open Settings → Customization, see all markdowns, edit + save round-trip
- [ ] Manual: external edit (\`echo \"x\" >> ~/Documents/zbot/config/SOUL.md\`) → editor shows the conflict banner if dirty, silently refreshes if not

## Behavior preserved

- SOUL.md generation, INSTRUCTIONS.md assembly, etc. — unchanged. The existing prompt-assembly flow still reads from \`<vault>/config/\` files; the editor just provides a UI surface for the same files.
- File watcher infrastructure — unchanged; we just add one more \`add_watch\` call.
- Event bus — gains one new \`GatewayEvent\` variant; existing consumers either use \`if let\` or wildcard arms, so no consumer-side changes were needed (verified during Task 5).

## Out of scope (per spec)

Create / delete operations from the UI, JSON config files, the \`wards/\` subdirectory, live markdown preview, syntax highlighting, diff view, audit log, multi-user concurrent edit reconciliation. All documented in the spec.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

I reviewed the plan against the spec:

**Spec coverage:** Every spec section maps to a task — file scope (Task 2), backend API (Tasks 2, 3, 4), file watcher event (Tasks 5, 6), frontend components (Tasks 7-10), state management / save flow (Task 9), file watcher reaction in UI (Task 11), testing (per-task plus Task 12), file structure (covered across tasks).

**Placeholder scan:** No "TBD"/"TODO"/"add appropriate"/"similar to Task N" patterns. The few "the exact mechanism depends on …" notes (Task 11) reference the existing WebSocket event router whose shape is already in the codebase — the implementer reads it and follows the existing pattern. Not a placeholder.

**Type / name consistency:** `FileEntry`, `FileResponse`, `SaveRequest`, `FileKind`, `GatewayEvent::CustomizationFileChanged`, allow-list rules (`*.md` and `shards/*.md`) used consistently across tasks. Field naming alignment between backend `snake_case` (wire format via serde) and frontend `camelCase` is documented in Task 11. Endpoint paths consistent across Tasks 2, 3, 4 (`/api/customization/files`, `/api/customization/file`).

No fixes needed.
