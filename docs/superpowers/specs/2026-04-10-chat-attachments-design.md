# Chat Attachments — Design Spec

## Problem

Users can't attach files to their chat messages. To share a PDF, image, or data file with the agent, users must manually copy it into a ward directory and reference the path in their message.

## Solution

Add file attachment support to the chat input. Files are staged to `{vault}/temp/attachments/`, paths are tracked in state for cleanup, and a markdown table of attached files is injected into the user's prompt so the agent can access them via existing tools.

## Design

### Upload Flow

1. User attaches file(s) via file picker button or drag-drop on chat input
2. UI uploads each file to `POST /api/attachments`
3. Backend saves to `{vault}/temp/attachments/{uuid}-{filename}`
4. Backend records the file path in `session_attachments` table (tied to session)
5. Backend returns `{ id, path, fileName, mimeType, size }`
6. UI shows attachment previews (pills/thumbnails) below the text input
7. When user sends the message, UI prepends an attachment table to the message text:

```markdown
[Attached files]
| File | Type | Size | Path |
|------|------|------|------|
| report.pdf | application/pdf | 2.1 MB | /home/user/Documents/zbot/temp/attachments/a1b2c3-report.pdf |
| data.csv | text/csv | 340 KB | /home/user/Documents/zbot/temp/attachments/d4e5f6-data.csv |

{user's actual message}
```

8. Agent sees paths in its prompt, uses `read`, `shell`, `multimodal_analyze`, `python` etc. to process them

### Backend

#### Upload Endpoint

```
POST /api/attachments
Content-Type: multipart/form-data

Body: file (binary), session_id (string, optional)
Response: { id, path, fileName, mimeType, size }
```

- File saved to `{vault}/temp/attachments/{uuid}-{original_filename}`
- UUID prefix prevents collisions
- No file type restrictions (agent decides how to handle)
- No size limit enforced by the endpoint (filesystem is the limit)

#### State Tracking Table

```sql
CREATE TABLE IF NOT EXISTS session_attachments (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    mime_type TEXT,
    file_size INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);
```

- `session_id` is nullable — attachment may be uploaded before session is created (first message)
- When session is established, update the attachment's `session_id`
- `ON DELETE CASCADE` — when session is deleted from logs, attachment records are removed (phase 2: also delete the files)

#### Link Attachments to Session

```
POST /api/attachments/{id}/link
Body: { session_id }
```

Called after the session is created (first message response includes session_id). Links orphaned attachments to their session.

Alternatively, pass `session_id` in the upload if the session already exists (follow-up messages).

### Frontend

#### Chat Input Enhancement

- Add paperclip/attach button next to the send button
- Support drag-drop on the chat input area
- Show attached files as removable pills below the text input:

```
┌─────────────────────────────────────────────┐
│ Build an auth system using the attached spec │
├─────────────────────────────────────────────┤
│ 📄 spec.pdf (2.1 MB) ✕  📊 data.csv (340 KB) ✕ │
└─────────────────────────────────────────────┘
                                        [📎] [Send]
```

- Clicking ✕ removes the attachment (pill only — file stays on disk for potential reuse)
- On send: prepend attachment table to message text, then send via existing invoke flow

#### Prompt Injection

The UI builds the table before sending. This means:
- No backend changes to the invoke/message flow
- The agent sees a normal text message with file paths
- Works with both new sessions and follow-up messages
- The table is visible in the chat history (user can see what was attached)

### MIME Type Detection

Use file extension → MIME type mapping:

| Extension | MIME Type |
|-----------|-----------|
| pdf | application/pdf |
| docx | application/vnd.openxmlformats-officedocument.wordprocessingml.document |
| pptx | application/vnd.openxmlformats-officedocument.presentationml.presentation |
| xlsx | application/vnd.openxmlformats-officedocument.spreadsheetml.sheet |
| csv | text/csv |
| json | application/json |
| md, txt | text/plain |
| html | text/html |
| png | image/png |
| jpg, jpeg | image/jpeg |
| gif | image/gif |
| svg | image/svg+xml |
| mp4 | video/mp4 |
| mp3 | audio/mpeg |
| py, rs, js, ts | text/plain |
| * | application/octet-stream |

Backend detects from filename extension. No content sniffing.

### Cleanup (Phase 2 — Backlog)

When a session is deleted from the Logs page:
1. Query `session_attachments` for the session's files
2. Delete files from disk
3. Records cascade-deleted automatically via FK

For now: manual cleanup. Files accumulate in `{vault}/temp/attachments/`. User can delete the directory contents manually.

## Scope

### In Scope
- `POST /api/attachments` upload endpoint
- `session_attachments` table + schema migration
- File picker button + drag-drop on chat input
- Attachment preview pills below text input
- Prompt injection (markdown table prepended to message)
- MIME type detection from extension

### Out of Scope
- File size limits
- Image preview thumbnails (just show filename + size as pills)
- Auto-cleanup on session delete (phase 2)
- Paste from clipboard (images) — future enhancement
- Multimodal auto-invoke for images — agent decides
- Compression or transcoding

## Files to Create/Modify

### Backend (Rust)
| File | Change |
|------|--------|
| `gateway/gateway-database/src/schema.rs` | Add `session_attachments` table, bump version |
| `services/execution-state/src/types.rs` | Add `SessionAttachment` struct |
| `services/execution-state/src/repository.rs` | CRUD for session_attachments |
| `services/execution-state/src/service.rs` | Service wrappers |
| `gateway/src/http/attachments.rs` | New: upload endpoint |
| `gateway/src/http/mod.rs` | Register route |

### Frontend (TypeScript/React)
| File | Change |
|------|--------|
| `apps/ui/src/services/transport/types.ts` | Add `UploadedAttachment` interface |
| `apps/ui/src/services/transport/interface.ts` | Add `uploadAttachment` method |
| `apps/ui/src/services/transport/http.ts` | Implement multipart upload |
| `apps/ui/src/features/chat/ChatInput.tsx` or equivalent | Add file picker, drag-drop, attachment pills, prompt injection |
| `apps/ui/src/styles/components.css` | Attachment pill styles |
