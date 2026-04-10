# Artifact Logging — Design Spec

## Problem

Agents generate files (reports, code, documents, images, data) but there's no centralized way to see what was produced. Users must manually browse ward directories to find outputs. No association between session and generated artifacts.

## Solution

Extend the `respond` tool to accept an `artifacts` array. When an agent responds, it declares the files it produced. Backend persists artifact metadata to a new `artifacts` table tied to session and ward. UI shows an artifacts panel with inline rendering per file type.

## Design

### Respond Tool Extension

Add optional `artifacts` parameter to `respond`:

```json
{
  "message": "Auth system implementation complete",
  "format": "markdown",
  "artifacts": [
    { "path": "src/auth.rs", "label": "Auth middleware" },
    { "path": "docs/auth-api.md", "label": "API documentation" },
    { "path": "reports/test-results.html", "label": "Test results" }
  ]
}
```

Each artifact has:
- `path` (required) — relative to ward directory, or absolute
- `label` (optional) — human-readable description. Falls back to filename.

Backend resolves:
- `file_name` — extracted from path
- `file_type` — detected from extension (pdf, docx, csv, html, md, png, mp4, etc.)
- `file_size` — read from filesystem at persist time
- Absolute path — resolved relative to current ward

### Database Schema

New table in `conversations.db`:

```sql
CREATE TABLE IF NOT EXISTS artifacts (
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
);

CREATE INDEX idx_artifacts_session ON artifacts(session_id);
CREATE INDEX idx_artifacts_ward ON artifacts(ward_id);
```

Fields:
- `id` — `art-{uuid}`
- `session_id` — the root session (for parent queries)
- `ward_id` — ward where file lives (nullable for non-ward files)
- `execution_id` — which execution created it (root or subagent)
- `agent_id` — agent that created the artifact
- `file_path` — absolute path on filesystem
- `file_name` — display name (basename)
- `file_type` — extension-based: pdf, docx, xlsx, csv, json, html, md, png, jpg, mp4, mp3, rs, py, js, ts, txt, etc.
- `file_size` — bytes, read at persist time
- `label` — agent's description
- `created_at` — ISO 8601 timestamp

### API Endpoints

**List artifacts for a session:**
```
GET /api/sessions/{session_id}/artifacts
```
Returns all artifacts for the session, including from child sessions (subagents). Ordered by `created_at`.

Response:
```json
[
  {
    "id": "art-abc123",
    "session_id": "sess-xyz",
    "ward_id": "stock-tracker",
    "execution_id": "exec-456",
    "agent_id": "code-agent",
    "file_path": "/home/user/Documents/zbot/wards/stock-tracker/src/auth.rs",
    "file_name": "auth.rs",
    "file_type": "rs",
    "file_size": 2148,
    "label": "Auth middleware",
    "created_at": "2026-04-09T10:30:00Z"
  }
]
```

**Serve artifact content:**
```
GET /api/artifacts/{artifact_id}/content
```
Serves the file with correct MIME type. For text files, returns UTF-8 content. For binary files (images, pdf, docx), returns binary with appropriate Content-Type header.

MIME type mapping:
- `md` → `text/markdown`
- `html` → `text/html`
- `csv` → `text/csv`
- `json` → `application/json`
- `pdf` → `application/pdf`
- `docx` → `application/vnd.openxmlformats-officedocument.wordprocessingml.document`
- `pptx` → `application/vnd.openxmlformats-officedocument.presentationml.presentation`
- `xlsx` → `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`
- `png/jpg/gif/svg` → `image/*`
- `mp4/webm` → `video/*`
- `mp3/wav` → `audio/*`
- Code files (rs, py, js, ts) → `text/plain`
- Default → `application/octet-stream`

### Subagent Artifacts

When a subagent calls `respond` with artifacts:
1. Artifacts are stored with the subagent's `execution_id` and `agent_id`
2. `session_id` is set to the **root** session (not the child session) — this way querying by root session_id returns all artifacts including from subagents
3. The delegation callback includes artifact IDs so the root agent knows what was produced

### Respond Action Extension

Extend `RespondAction` in `framework/zero-core/src/event.rs`:

```rust
pub struct RespondAction {
    pub message: String,
    pub format: Option<String>,
    pub conversation_id: Option<String>,
    pub session_id: Option<String>,
    pub artifacts: Vec<ArtifactDeclaration>,  // NEW
}

pub struct ArtifactDeclaration {
    pub path: String,
    pub label: Option<String>,
}
```

### Artifact Processing Flow

1. Agent calls `respond({ message, artifacts: [...] })`
2. Respond tool sets `RespondAction` with artifact declarations
3. Executor processes `RespondAction` — for each artifact declaration:
   - Resolve absolute path (ward-relative → absolute)
   - Check file exists
   - Read file size
   - Detect file type from extension
   - Generate `art-{uuid}` ID
   - Insert into `artifacts` table
4. Emit `StreamEvent::ArtifactCreated` for each artifact (so UI can update in real-time)
5. Include artifact metadata in the `AgentCompleted` event or response payload

### UI: Artifacts Panel

Show below the agent's response in the chat view:

```
📎 3 artifacts
  📄 Auth middleware          src/auth.rs          2.1 KB
  📝 API documentation        docs/auth-api.md     1.4 KB
  🌐 Test results             reports/test.html    8.7 KB
```

Icons by file type:
- 📄 Code files (rs, py, js, ts, etc.)
- 📝 Documents (md, txt, docx)
- 📊 Data (csv, json, xlsx)
- 🌐 Web (html)
- 📑 PDF
- 🖼️ Images (png, jpg, gif, svg)
- 🎬 Video (mp4, webm)
- 🔊 Audio (mp3, wav)

### UI: Inline Rendering (on click)

Opens a modal or slide-out panel. Renderer selected by file type:

| File Type | Renderer |
|-----------|----------|
| md | Rendered markdown (reuse existing markdown renderer) |
| html | Sandboxed iframe |
| csv | HTML table with sortable columns |
| json | Formatted/collapsible JSON tree |
| pdf | Embedded PDF viewer (`<embed>` or pdf.js) |
| docx | Rendered via mammoth.js (JS library, ~100KB) |
| pptx | Rendered via pptx2html or slide thumbnails via soffice conversion |
| xlsx | Rendered via SheetJS (first sheet as HTML table) |
| png, jpg, gif, svg | `<img>` tag |
| mp4, webm | Native `<video>` player |
| mp3, wav | Native `<audio>` player |
| code files | Syntax-highlighted code block |
| other | Download link |

For large files (>5MB), show download link instead of inline rendering.

### Agent Prompt Guidance

Add to agent instructions:

```
When you complete a task that produces output files, include them in your respond call:

respond({
  message: "Task complete. Created the auth system.",
  artifacts: [
    { path: "src/auth.rs", label: "Auth middleware implementation" },
    { path: "docs/api.md", label: "API documentation" }
  ]
})

Include any file the user would want to see or download: reports, code, documents, data exports, visualizations.
```

## Scope

### In Scope
- Extend `respond` tool with `artifacts` array
- `RespondAction` struct extension
- `artifacts` table in conversations.db (schema + migration)
- Artifact processing in executor (resolve path, read size, detect type, persist)
- `GET /api/sessions/{id}/artifacts` endpoint
- `GET /api/artifacts/{id}/content` endpoint (file serving with MIME types)
- UI artifacts panel in chat view
- Inline rendering for md, html, csv, json, images, video, audio, code
- Subagent artifact collection
- Agent prompt guidance

### Out of Scope
- Automatic artifact detection from shell commands
- Advanced DOCX/XLSX/PPTX rendering fallbacks (basic rendering in v1, polished renderers later)
- Artifact versioning or diffing
- Artifact search across sessions
- Artifact deletion/cleanup
- `register_artifact` separate tool

## Files to Modify/Create

### Backend (Rust)
| File | Change |
|------|--------|
| `framework/zero-core/src/event.rs` | Add `ArtifactDeclaration` struct, extend `RespondAction` |
| `runtime/agent-runtime/src/tools/respond.rs` | Parse `artifacts` from tool args into `RespondAction` |
| `runtime/agent-runtime/src/types/events.rs` | Add `StreamEvent::ArtifactCreated` |
| `runtime/agent-runtime/src/executor.rs` | Process artifacts from RespondAction, emit events |
| `gateway/gateway-database/src/schema.rs` | Add `artifacts` table, bump schema version |
| `services/execution-state/src/repository.rs` | CRUD for artifacts table |
| `services/execution-state/src/service.rs` | Service methods for artifact operations |
| `gateway/src/http/` | New artifact API endpoints |
| `gateway/gateway-execution/src/invoke/stream.rs` | Handle ArtifactCreated in stream processing |

### Frontend (TypeScript/React)
| File | Change |
|------|--------|
| `apps/ui/src/services/transport/types.ts` | Add `Artifact` interface |
| `apps/ui/src/services/transport/interface.ts` | Add artifact API methods |
| `apps/ui/src/services/transport/http.ts` | Implement artifact API calls |
| `apps/ui/src/features/chat/ArtifactsPanel.tsx` | New: collapsible artifact list |
| `apps/ui/src/features/chat/ArtifactViewer.tsx` | New: inline renderer modal |
| `apps/ui/src/features/chat/` | Integrate artifacts panel into chat response |
