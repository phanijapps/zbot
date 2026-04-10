# Artifact Logging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agents declare output artifacts (files, reports, documents) in their `respond` call. Backend persists metadata to a new `artifacts` table. UI shows an artifacts panel with inline rendering per file type.

**Architecture:** Extend `RespondAction` with `artifacts: Vec<ArtifactDeclaration>`. Executor processes declarations (resolve path, read size, detect type) and persists to SQLite. Two new API endpoints serve artifact lists and file content. UI renders an artifacts panel below agent responses with type-specific inline viewers.

**Tech Stack:** Rust (SQLite, Axum), TypeScript/React, mammoth.js (docx), SheetJS (xlsx)

---

### Task 1: Extend RespondAction and Respond Tool (Rust)

**Files:**
- Modify: `framework/zero-core/src/event.rs:141-155`
- Modify: `runtime/agent-runtime/src/tools/respond.rs:53-123`

- [ ] **Step 1: Add ArtifactDeclaration struct to event.rs**

In `framework/zero-core/src/event.rs`, add before `RespondAction`:

```rust
/// A file artifact declared by an agent in its response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDeclaration {
    /// File path (relative to ward or absolute)
    pub path: String,
    /// Human-readable label
    pub label: Option<String>,
}
```

- [ ] **Step 2: Add artifacts field to RespondAction**

In the same file, add to `RespondAction` struct after `session_id`:

```rust
    /// Artifacts produced by this execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactDeclaration>,
```

- [ ] **Step 3: Update respond tool parameter schema**

In `runtime/agent-runtime/src/tools/respond.rs`, update `parameters_schema()` to add artifacts:

```rust
fn parameters_schema(&self) -> Option<Value> {
    Some(json!({
        "type": "object",
        "properties": {
            "message": {
                "type": "string",
                "description": "The response message to send to the user"
            },
            "format": {
                "type": "string",
                "enum": ["text", "markdown", "html"],
                "default": "text",
                "description": "Format of the message"
            },
            "artifacts": {
                "type": "array",
                "description": "Files produced by this execution. Include any outputs the user would want to see or download.",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path relative to the current ward"
                        },
                        "label": {
                            "type": "string",
                            "description": "Human-readable label for this artifact"
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        "required": ["message"]
    }))
}
```

- [ ] **Step 4: Parse artifacts in respond tool execute()**

In the `execute()` method, after parsing `format`, add artifact parsing:

```rust
    let artifacts: Vec<zero_core::event::ArtifactDeclaration> = args
        .get("artifacts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
```

Then include in the RespondAction construction:

```rust
    actions.respond = Some(zero_core::event::RespondAction {
        message: message.to_string(),
        format: format.to_string(),
        conversation_id: conversation_id.clone(),
        session_id: session_id.clone(),
        artifacts,
    });
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p zero-core -p agent-runtime`
Expected: compiles (warnings about unused `artifacts` field are fine — executor will use it in Task 3)

- [ ] **Step 6: Commit**

```bash
git add framework/zero-core/src/event.rs runtime/agent-runtime/src/tools/respond.rs
git commit -m "feat: extend respond tool with artifacts declaration"
```

---

### Task 2: Add Artifacts Table and DB Operations (Rust)

**Files:**
- Modify: `gateway/gateway-database/src/schema.rs:9,201-207`
- Modify: `services/execution-state/src/repository.rs`
- Modify: `services/execution-state/src/service.rs`
- Modify: `services/execution-state/src/types.rs`

- [ ] **Step 1: Add Artifact struct to types.rs**

In `services/execution-state/src/types.rs`, add:

```rust
/// A file artifact produced by an agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub session_id: String,
    pub ward_id: Option<String>,
    pub execution_id: Option<String>,
    pub agent_id: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub file_type: Option<String>,
    pub file_size: Option<i64>,
    pub label: Option<String>,
    pub created_at: String,
}

impl Artifact {
    pub fn new(
        session_id: impl Into<String>,
        file_path: impl Into<String>,
        file_name: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("art-{}", uuid::Uuid::new_v4()),
            session_id: session_id.into(),
            ward_id: None,
            execution_id: None,
            agent_id: None,
            file_path: file_path.into(),
            file_name: file_name.into(),
            file_type: None,
            file_size: None,
            label: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}
```

- [ ] **Step 2: Add schema migration**

In `gateway/gateway-database/src/schema.rs`:

Bump `SCHEMA_VERSION` from 15 to 16.

Add migration after the v15 block:

```rust
// v15 → v16: Add artifacts table for tracking agent-generated files
if version < 16 {
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS artifacts (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            ward_id TEXT,
            execution_id TEXT,
            agent_id TEXT,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            file_type TEXT,
            file_size INTEGER,
            label TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_artifacts_session ON artifacts(session_id)",
        [],
    );
}
```

Also add the CREATE TABLE to the main `create_tables()` function (so fresh DBs get it without migration).

- [ ] **Step 3: Add repository methods**

In `services/execution-state/src/repository.rs`, add:

```rust
pub fn create_artifact(&self, artifact: &crate::types::Artifact) -> Result<(), String> {
    self.db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO artifacts (id, session_id, ward_id, execution_id, agent_id,
                file_path, file_name, file_type, file_size, label, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                artifact.id,
                artifact.session_id,
                artifact.ward_id,
                artifact.execution_id,
                artifact.agent_id,
                artifact.file_path,
                artifact.file_name,
                artifact.file_type,
                artifact.file_size,
                artifact.label,
                artifact.created_at,
            ],
        )?;
        Ok(())
    })
}

pub fn list_artifacts_by_session(&self, session_id: &str) -> Result<Vec<crate::types::Artifact>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, ward_id, execution_id, agent_id,
                    file_path, file_name, file_type, file_size, label, created_at
             FROM artifacts
             WHERE session_id = ?
             ORDER BY created_at ASC",
        )?;
        let artifacts = stmt
            .query_map(params![session_id], |row| {
                Ok(crate::types::Artifact {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    ward_id: row.get(2)?,
                    execution_id: row.get(3)?,
                    agent_id: row.get(4)?,
                    file_path: row.get(5)?,
                    file_name: row.get(6)?,
                    file_type: row.get(7)?,
                    file_size: row.get(8)?,
                    label: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(artifacts)
    })
}

pub fn get_artifact(&self, artifact_id: &str) -> Result<Option<crate::types::Artifact>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, ward_id, execution_id, agent_id,
                    file_path, file_name, file_type, file_size, label, created_at
             FROM artifacts WHERE id = ?",
        )?;
        let artifact = stmt
            .query_row(params![artifact_id], |row| {
                Ok(crate::types::Artifact {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    ward_id: row.get(2)?,
                    execution_id: row.get(3)?,
                    agent_id: row.get(4)?,
                    file_path: row.get(5)?,
                    file_name: row.get(6)?,
                    file_type: row.get(7)?,
                    file_size: row.get(8)?,
                    label: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })
            .optional()?;
        Ok(artifact)
    })
}
```

- [ ] **Step 4: Add service methods**

In `services/execution-state/src/service.rs`, add:

```rust
pub fn create_artifact(&self, artifact: &crate::types::Artifact) -> Result<(), String> {
    self.repo.create_artifact(artifact)
}

pub fn list_artifacts_by_session(&self, session_id: &str) -> Result<Vec<crate::types::Artifact>, String> {
    self.repo.list_artifacts_by_session(session_id)
}

pub fn get_artifact(&self, artifact_id: &str) -> Result<Option<crate::types::Artifact>, String> {
    self.repo.get_artifact(artifact_id)
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p execution-state -p gateway-database`

- [ ] **Step 6: Commit**

```bash
git add services/execution-state/src/types.rs services/execution-state/src/repository.rs services/execution-state/src/service.rs gateway/gateway-database/src/schema.rs
git commit -m "feat: add artifacts table and DB operations"
```

---

### Task 3: Process Artifacts in Executor and Stream (Rust)

**Files:**
- Modify: `runtime/agent-runtime/src/types/events.rs`
- Modify: `gateway/gateway-execution/src/invoke/stream.rs`
- Modify: `gateway/gateway-events/src/lib.rs`

This task wires artifact processing: when a respond action contains artifacts, the gateway resolves file metadata and persists to DB.

- [ ] **Step 1: Add ArtifactCreated to GatewayEvent**

In `gateway/gateway-events/src/lib.rs`, add a new variant:

```rust
ArtifactCreated {
    session_id: String,
    execution_id: String,
    artifact_id: String,
    file_name: String,
    file_type: Option<String>,
    file_size: Option<i64>,
    label: Option<String>,
},
```

- [ ] **Step 2: Process artifacts in stream handler**

In `gateway/gateway-execution/src/invoke/stream.rs`, find where `GatewayEvent::Respond` is handled (or where the respond action completes). After the response is accumulated, process artifact declarations.

The artifact processing should happen in the completion handler (`handle_execution_success` in `delegation/spawn.rs` for subagents, or the main executor completion for root). The key integration point is where `RespondAction` is available along with session context.

Add a function in `gateway/gateway-execution/src/invoke/stream.rs` or a new file `gateway/gateway-execution/src/artifacts.rs`:

```rust
pub fn process_artifact_declarations(
    declarations: &[zero_core::event::ArtifactDeclaration],
    session_id: &str,
    execution_id: &str,
    agent_id: &str,
    ward_id: Option<&str>,
    ward_dir: Option<&std::path::Path>,
    state_service: &execution_state::StateService<gateway_database::DatabaseManager>,
    event_bus: &gateway_events::EventBus,
) {
    for decl in declarations {
        // Resolve absolute path
        let abs_path = if std::path::Path::new(&decl.path).is_absolute() {
            std::path::PathBuf::from(&decl.path)
        } else if let Some(ward) = ward_dir {
            ward.join(&decl.path)
        } else {
            std::path::PathBuf::from(&decl.path)
        };

        // Check file exists and read metadata
        let (file_size, file_exists) = match std::fs::metadata(&abs_path) {
            Ok(meta) => (Some(meta.len() as i64), true),
            Err(_) => (None, false),
        };

        if !file_exists {
            tracing::warn!(
                path = %abs_path.display(),
                "Artifact declared but file not found"
            );
            continue;
        }

        let file_name = abs_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| decl.path.clone());

        let file_type = abs_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase());

        let mut artifact = execution_state::Artifact::new(
            session_id,
            abs_path.to_string_lossy().to_string(),
            &file_name,
        );
        artifact.ward_id = ward_id.map(|s| s.to_string());
        artifact.execution_id = Some(execution_id.to_string());
        artifact.agent_id = Some(agent_id.to_string());
        artifact.file_type = file_type.clone();
        artifact.file_size = file_size;
        artifact.label = decl.label.clone();

        if let Err(e) = state_service.create_artifact(&artifact) {
            tracing::warn!(artifact_id = %artifact.id, "Failed to persist artifact: {}", e);
            continue;
        }

        // Emit event for real-time UI updates
        let _ = event_bus.try_publish(gateway_events::GatewayEvent::ArtifactCreated {
            session_id: session_id.to_string(),
            execution_id: execution_id.to_string(),
            artifact_id: artifact.id.clone(),
            file_name: file_name.clone(),
            file_type,
            file_size,
            label: decl.label.clone(),
        });

        tracing::info!(
            artifact_id = %artifact.id,
            path = %abs_path.display(),
            file_type = ?artifact.file_type,
            "Artifact persisted"
        );
    }
}
```

- [ ] **Step 3: Call artifact processing from executor completion**

Find where the respond action's artifacts need to be processed. The best place is in the stream event handler where `GatewayEvent::Respond` is created — or in the completion flow where the `RespondAction` is available.

The `RespondAction.artifacts` are set by the tool, passed through `StreamEvent::ActionRespond`. The gateway's `convert_stream_event` function converts this to `GatewayEvent::Respond`. At this point, call `process_artifact_declarations()`.

Alternatively, call it from the `StreamContext` when processing the respond event, since `StreamContext` has access to `state_service`, `event_bus`, `session_id`, `execution_id`, and `agent_id`.

**Note to implementer:** Read the `StreamEvent::ActionRespond` handler in `stream.rs` and the executor's respond handling in `executor.rs:966-978`. The artifacts need to flow from `RespondAction` through the stream event to the gateway where `state_service` is available. The executor doesn't have direct DB access — the gateway does.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p gateway-execution -p gateway-events`

- [ ] **Step 5: Commit**

```bash
git add gateway/gateway-events/src/lib.rs gateway/gateway-execution/src/
git commit -m "feat: process artifact declarations and persist to DB"
```

---

### Task 4: Add Artifact API Endpoints (Rust)

**Files:**
- Create: `gateway/src/http/artifacts.rs`
- Modify: `gateway/src/http/mod.rs` (add route)

- [ ] **Step 1: Create artifacts HTTP handler**

Create `gateway/src/http/artifacts.rs`:

```rust
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use crate::state::AppState;

#[derive(Serialize)]
struct ArtifactResponse {
    id: String,
    session_id: String,
    ward_id: Option<String>,
    execution_id: Option<String>,
    agent_id: Option<String>,
    file_path: String,
    file_name: String,
    file_type: Option<String>,
    file_size: Option<i64>,
    label: Option<String>,
    created_at: String,
}

impl From<execution_state::Artifact> for ArtifactResponse {
    fn from(a: execution_state::Artifact) -> Self {
        Self {
            id: a.id,
            session_id: a.session_id,
            ward_id: a.ward_id,
            execution_id: a.execution_id,
            agent_id: a.agent_id,
            file_path: a.file_path,
            file_name: a.file_name,
            file_type: a.file_type,
            file_size: a.file_size,
            label: a.label,
            created_at: a.created_at,
        }
    }
}

/// GET /api/sessions/:session_id/artifacts
pub async fn list_session_artifacts(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<ArtifactResponse>>, (StatusCode, String)> {
    let artifacts = state
        .state_service
        .list_artifacts_by_session(&session_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Also include artifacts from child sessions
    let child_ids: Vec<String> = state
        .state_service
        .get_child_session_ids(&session_id)
        .unwrap_or_default();

    let mut all_artifacts: Vec<ArtifactResponse> = artifacts.into_iter().map(ArtifactResponse::from).collect();
    for child_id in child_ids {
        if let Ok(child_artifacts) = state.state_service.list_artifacts_by_session(&child_id) {
            all_artifacts.extend(child_artifacts.into_iter().map(ArtifactResponse::from));
        }
    }

    // Sort by created_at
    all_artifacts.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    Ok(Json(all_artifacts))
}

/// GET /api/artifacts/:artifact_id/content
pub async fn serve_artifact_content(
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let artifact = state
        .state_service
        .get_artifact(&artifact_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Artifact not found".to_string()))?;

    let content = std::fs::read(&artifact.file_path)
        .map_err(|e| (StatusCode::NOT_FOUND, format!("File not found: {}", e)))?;

    let mime = match artifact.file_type.as_deref() {
        Some("md") => "text/markdown",
        Some("html") | Some("htm") => "text/html",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    };

    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (
                header::CONTENT_DISPOSITION,
                &format!("inline; filename=\"{}\"", artifact.file_name),
            ),
        ],
        content,
    ))
}
```

- [ ] **Step 2: Register routes**

In `gateway/src/http/mod.rs`, add the routes. Find where other API routes are registered and add:

```rust
.route("/api/sessions/:session_id/artifacts", get(artifacts::list_session_artifacts))
.route("/api/artifacts/:artifact_id/content", get(artifacts::serve_artifact_content))
```

Add `mod artifacts;` at the top.

- [ ] **Step 3: Add `get_child_session_ids` if it doesn't exist**

Check if `StateService` has a method to get child session IDs. If not, add to repository and service:

```rust
// repository.rs
pub fn get_child_session_ids(&self, parent_session_id: &str) -> Result<Vec<String>, String> {
    self.db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id FROM sessions WHERE parent_session_id = ?",
        )?;
        let ids = stmt
            .query_map(params![parent_session_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    })
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p gateway`

- [ ] **Step 5: Commit**

```bash
git add gateway/src/http/artifacts.rs gateway/src/http/mod.rs services/execution-state/src/
git commit -m "feat: add artifact list and content serving API endpoints"
```

---

### Task 5: Frontend — Transport Types and API Methods (TypeScript)

**Files:**
- Modify: `apps/ui/src/services/transport/types.ts`
- Modify: `apps/ui/src/services/transport/interface.ts`
- Modify: `apps/ui/src/services/transport/http.ts`

- [ ] **Step 1: Add Artifact interface**

In `apps/ui/src/services/transport/types.ts`, add:

```typescript
export interface Artifact {
  id: string;
  session_id: string;
  ward_id?: string;
  execution_id?: string;
  agent_id?: string;
  file_path: string;
  file_name: string;
  file_type?: string;
  file_size?: number;
  label?: string;
  created_at: string;
}
```

- [ ] **Step 2: Add transport interface methods**

In `apps/ui/src/services/transport/interface.ts`, add:

```typescript
listSessionArtifacts(sessionId: string): Promise<TransportResult<Artifact[]>>;
getArtifactContentUrl(artifactId: string): string;
```

- [ ] **Step 3: Implement in HTTP transport**

In `apps/ui/src/services/transport/http.ts`, add:

```typescript
async listSessionArtifacts(sessionId: string): Promise<TransportResult<Artifact[]>> {
    return this.get<Artifact[]>(`/api/sessions/${encodeURIComponent(sessionId)}/artifacts`);
}

getArtifactContentUrl(artifactId: string): string {
    return `${this.baseUrl}/api/artifacts/${encodeURIComponent(artifactId)}/content`;
}
```

- [ ] **Step 4: Verify build**

Run: `cd apps/ui && npx tsc --noEmit`

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/services/transport/
git commit -m "feat: add artifact transport types and API methods"
```

---

### Task 6: Frontend — ArtifactsPanel Component (TypeScript/React)

**Files:**
- Create: `apps/ui/src/features/chat/ArtifactsPanel.tsx`

- [ ] **Step 1: Create the artifacts panel**

```tsx
import { useState, useEffect } from "react";
import {
  FileText, FileCode, Table, Globe, Image, Film, Music,
  Presentation, FileSpreadsheet, File, Download, ChevronDown, ChevronRight, Paperclip
} from "lucide-react";
import { getTransport } from "@/services/transport";
import type { Artifact } from "@/services/transport/types";

interface ArtifactsPanelProps {
  sessionId: string;
}

function getArtifactIcon(fileType?: string) {
  const size = 14;
  switch (fileType) {
    case "md": case "txt": case "docx": return <FileText size={size} />;
    case "rs": case "py": case "js": case "ts": case "tsx": case "jsx": return <FileCode size={size} />;
    case "csv": case "json": case "xlsx": return <Table size={size} />;
    case "html": case "htm": return <Globe size={size} />;
    case "png": case "jpg": case "jpeg": case "gif": case "svg": return <Image size={size} />;
    case "mp4": case "webm": return <Film size={size} />;
    case "mp3": case "wav": return <Music size={size} />;
    case "pptx": return <Presentation size={size} />;
    case "pdf": return <FileText size={size} />;
    default: return <File size={size} />;
  }
}

function formatFileSize(bytes?: number): string {
  if (!bytes) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function ArtifactsPanel({ sessionId }: ArtifactsPanelProps) {
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [expanded, setExpanded] = useState(true);
  const [viewingId, setViewingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      const transport = await getTransport();
      const result = await transport.listSessionArtifacts(sessionId);
      if (!cancelled && result.success && result.data) {
        setArtifacts(result.data);
      }
    }
    load();
    return () => { cancelled = true; };
  }, [sessionId]);

  if (artifacts.length === 0) return null;

  return (
    <div className="artifacts-panel">
      <div
        className="artifacts-panel__header"
        onClick={() => setExpanded(!expanded)}
      >
        <Paperclip size={14} />
        <span>{artifacts.length} artifact{artifacts.length !== 1 ? "s" : ""}</span>
        {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      </div>
      {expanded && (
        <div className="artifacts-panel__list">
          {artifacts.map((art) => (
            <ArtifactRow
              key={art.id}
              artifact={art}
              isViewing={viewingId === art.id}
              onToggleView={() => setViewingId(viewingId === art.id ? null : art.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function ArtifactRow({
  artifact,
  isViewing,
  onToggleView,
}: {
  artifact: Artifact;
  isViewing: boolean;
  onToggleView: () => void;
}) {
  return (
    <div>
      <div className="artifacts-panel__row" onClick={onToggleView}>
        <span className="artifacts-panel__icon">{getArtifactIcon(artifact.file_type)}</span>
        <span className="artifacts-panel__label">{artifact.label || artifact.file_name}</span>
        <span className="artifacts-panel__path">{artifact.file_name}</span>
        <span className="artifacts-panel__size">{formatFileSize(artifact.file_size)}</span>
      </div>
      {isViewing && <ArtifactViewer artifact={artifact} />}
    </div>
  );
}

function ArtifactViewer({ artifact }: { artifact: Artifact }) {
  const [content, setContent] = useState<string | null>(null);
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      setLoading(true);
      const transport = await getTransport();
      const url = transport.getArtifactContentUrl(artifact.id);

      try {
        const resp = await fetch(url);
        if (!resp.ok) throw new Error("Failed to fetch");

        const isText = ["md", "txt", "html", "htm", "csv", "json", "rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml", "xml", "sql", "sh", "bash", "css"].includes(artifact.file_type || "");

        if (isText) {
          const text = await resp.text();
          if (!cancelled) setContent(text);
        } else {
          const blob = await resp.blob();
          if (!cancelled) setBlobUrl(URL.createObjectURL(blob));
        }
      } catch (e) {
        console.error("Failed to load artifact:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    load();
    return () => {
      cancelled = true;
      if (blobUrl) URL.revokeObjectURL(blobUrl);
    };
  }, [artifact.id]);

  if (loading) {
    return <div className="artifacts-panel__viewer"><span className="loading-spinner" /></div>;
  }

  const ft = artifact.file_type || "";
  const transport_sync = { getArtifactContentUrl: (id: string) => `/api/artifacts/${id}/content` };
  const contentUrl = transport_sync.getArtifactContentUrl(artifact.id);

  // Text-based renderers
  if (ft === "md" || ft === "txt") {
    return <div className="artifacts-panel__viewer"><pre>{content}</pre></div>;
  }
  if (ft === "html" || ft === "htm") {
    return <div className="artifacts-panel__viewer"><iframe srcDoc={content || ""} style={{ width: "100%", height: 400, border: "none" }} sandbox="allow-scripts" /></div>;
  }
  if (ft === "csv") {
    return <div className="artifacts-panel__viewer"><CsvTable content={content || ""} /></div>;
  }
  if (ft === "json") {
    return <div className="artifacts-panel__viewer"><pre>{formatJson(content || "")}</pre></div>;
  }
  // Code files
  if (["rs", "py", "js", "ts", "tsx", "jsx", "toml", "yaml", "yml", "xml", "sql", "sh", "css"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><pre><code>{content}</code></pre></div>;
  }
  // Images
  if (["png", "jpg", "jpeg", "gif", "svg"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><img src={blobUrl || contentUrl} alt={artifact.file_name} style={{ maxWidth: "100%", maxHeight: 500 }} /></div>;
  }
  // Video
  if (["mp4", "webm"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><video src={blobUrl || contentUrl} controls style={{ maxWidth: "100%" }} /></div>;
  }
  // Audio
  if (["mp3", "wav"].includes(ft)) {
    return <div className="artifacts-panel__viewer"><audio src={blobUrl || contentUrl} controls /></div>;
  }
  // PDF
  if (ft === "pdf") {
    return <div className="artifacts-panel__viewer"><embed src={contentUrl} type="application/pdf" width="100%" height="500px" /></div>;
  }
  // Fallback: download link
  return (
    <div className="artifacts-panel__viewer">
      <a href={contentUrl} download={artifact.file_name} className="btn btn--outline btn--sm">
        <Download size={14} /> Download {artifact.file_name}
      </a>
    </div>
  );
}

function CsvTable({ content }: { content: string }) {
  const rows = content.split("\n").filter(Boolean).map((row) => row.split(","));
  if (rows.length === 0) return <pre>{content}</pre>;
  const header = rows[0];
  const body = rows.slice(1);
  return (
    <table style={{ width: "100%", fontSize: "12px", borderCollapse: "collapse" }}>
      <thead>
        <tr>{header.map((h, i) => <th key={i} style={{ textAlign: "left", padding: "4px 8px", borderBottom: "1px solid var(--border)", color: "var(--foreground)" }}>{h.trim()}</th>)}</tr>
      </thead>
      <tbody>
        {body.slice(0, 50).map((row, i) => (
          <tr key={i}>{row.map((cell, j) => <td key={j} style={{ padding: "4px 8px", borderBottom: "1px solid var(--border)", color: "var(--muted-foreground)" }}>{cell.trim()}</td>)}</tr>
        ))}
      </tbody>
    </table>
  );
}

function formatJson(content: string): string {
  try {
    return JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    return content;
  }
}
```

- [ ] **Step 2: Add CSS styles**

Append to `apps/ui/src/styles/components.css`:

```css
/* Artifacts Panel */
.artifacts-panel {
  margin-top: var(--spacing-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--card);
  overflow: hidden;
}

.artifacts-panel__header {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--muted-foreground);
  cursor: pointer;
  user-select: none;
}

.artifacts-panel__header:hover { background: var(--muted); }

.artifacts-panel__list { border-top: 1px solid var(--border); }

.artifacts-panel__row {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  font-size: var(--text-xs);
  cursor: pointer;
  border-bottom: 1px solid var(--border);
}

.artifacts-panel__row:last-child { border-bottom: none; }
.artifacts-panel__row:hover { background: var(--muted); }

.artifacts-panel__icon { color: var(--muted-foreground); flex-shrink: 0; }
.artifacts-panel__label { font-weight: 500; color: var(--foreground); }
.artifacts-panel__path { color: var(--muted-foreground); font-family: var(--font-mono); font-size: 11px; }
.artifacts-panel__size { margin-left: auto; color: var(--muted-foreground); font-size: 11px; white-space: nowrap; }

.artifacts-panel__viewer {
  padding: var(--spacing-3);
  border-top: 1px solid var(--border);
  background: var(--muted);
  max-height: 500px;
  overflow-y: auto;
  font-size: 12px;
}

.artifacts-panel__viewer pre {
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--foreground);
}
```

- [ ] **Step 3: Integrate into AgentResponse**

In `apps/ui/src/features/chat/AgentResponse.tsx`, import and render the panel after the markdown content. The exact integration depends on how session_id is available in that component — check if it's passed as a prop or available via context. If not available, pass it down from `ExecutionNarrative`.

```tsx
import { ArtifactsPanel } from "./ArtifactsPanel";

// Inside AgentResponse render, after ReactMarkdown:
{sessionId && <ArtifactsPanel sessionId={sessionId} />}
```

- [ ] **Step 4: Build and verify**

Run: `cd apps/ui && npm run build`

- [ ] **Step 5: Commit**

```bash
git add apps/ui/src/features/chat/ArtifactsPanel.tsx apps/ui/src/features/chat/AgentResponse.tsx apps/ui/src/styles/components.css
git commit -m "feat: add artifacts panel with inline rendering in chat view"
```

---

### Task 7: Integration Test and Final Verification

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test --workspace -- --skip test_get_unknown_model`
Expected: all tests pass

- [ ] **Step 2: Run UI build**

Run: `cd apps/ui && npm run build`
Expected: builds successfully

- [ ] **Step 3: Manual testing checklist**

1. Start daemon, open web UI
2. Ask agent to create a file (e.g., "write a Python script that generates a CSV report")
3. Agent should call `respond` with `artifacts: [{ path: "report.csv", label: "Generated report" }]`
4. Below the response, artifacts panel should appear
5. Click the artifact → inline CSV table renders
6. Test with other file types: HTML (iframe), markdown (rendered), JSON (formatted), images, code files
7. Check `/api/sessions/{id}/artifacts` returns the artifact list
8. Check `/api/artifacts/{id}/content` serves the file with correct MIME type

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: address integration testing issues"
```
