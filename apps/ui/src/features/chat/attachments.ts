// =============================================================================
// Attachment helpers — compose a markdown block that hands uploaded file
// metadata (including the absolute server-side path) to the agent.
//
// All composer surfaces (chat-v2, research-v2, mission-control hero) upload
// files to /api/upload, which writes them under the vault temp directory and
// returns absolute paths. The agent only learns those paths if we splice them
// into the user prompt — there is no separate "attachments" channel on
// executeAgent today. Keeping the format identical across composers means the
// agent sees one shape regardless of where the message came from.
// =============================================================================

import type { UploadedFile } from "./ChatInput";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Append a `**Attached files:**` markdown table to `text` listing each
 * upload's name, MIME type, size, and absolute path. Returns the original
 * text unchanged when `attachments` is empty.
 */
export function composeMessageWithAttachments(
  text: string,
  attachments: readonly UploadedFile[],
): string {
  const trimmed = text.trim();
  if (attachments.length === 0) return trimmed;
  const header = "| File | Type | Size | Path |";
  const sep = "|------|------|------|------|";
  const rows = attachments
    .map((a) => `| ${a.name} | ${a.mimeType} | ${formatSize(a.size)} | ${a.path} |`)
    .join("\n");
  return `${trimmed}\n\n**Attached files:**\n${header}\n${sep}\n${rows}`;
}
